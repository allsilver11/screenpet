//! 클라이언트 연결 하나임 (WebSocket 업그레이드 뒤): 챌린지 → 인증 → 제어 메시지와 프레임 넘기기.
//! 연결별 속도 한도와 시간 제한을 둠. `http::handle`이 `serve`를 부르고, 일은 `Hub` 메서드로 넘김.

use crate::hub::{bump, Admission, Bucket, Hub, Link, Out};
use crate::proto::{self, ClientMsg, ServerMsg, HEADER, MAX_FRAME, MAX_ID, PROTOCOL_VERSION};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{sync_channel, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::{Error as WsError, Message, WebSocket};

/// 연결 하나에 쌓아 두는 메시지 수. 넘으면 막힌 것으로 봄
const OUTBOX: usize = 512;
/// 루프가 큐와 타이머를 보기 전 읽기가 기다리는 시간임. 친구 프레임이 흐르는 동안은 이것,
/// `ACTIVE_FOR` 동안 멈춘 뒤엔 `POLL_IDLE`: ping만 하는 연결이 초당 66번 깨면 안 됨
const POLL: Duration = Duration::from_millis(15);
/// 쉬는 연결의 읽기 대기
const POLL_IDLE: Duration = Duration::from_millis(100);
/// 마지막 프레임 뒤 이만큼은 바쁜 연결로 봄
const ACTIVE_FOR: Duration = Duration::from_secs(2);
/// 연결당 분당 코드 사용 시도 수 상한
const REDEEMS_PER_CONN_PER_MIN: usize = 10;
/// 연결당 분당 건의 편지 시도 수 (진짜 하루 한도는 편지함이 지킴)
const LETTERS_PER_CONN_PER_MIN: usize = 3;
/// 연결당 "새 코드" 요청 수 상한
const NEW_CODES_PER_CONN: usize = 5;
/// 연결당 `friends` 갱신 속도임: 앱은 목록이 바뀔 때 하나 보내고 저장 데이터를 다시 씀.
/// 넘으면 끊음: 다음 `auth`에 목록이 실리므로 잃는 것 없음
const FRIENDS_PER_SEC: f64 = 0.5;
/// `friends` 갱신 한 번에 몰아 받는 수
const FRIENDS_BURST: f64 = 30.0;
/// ☀️ 연결당 `weather_want` 속도: 10초에 하나, 한 번에 몇 개
const WEATHER_WANTS_PER_SEC: f64 = 0.1;
/// `weather_want` 한 번에 몰아 받는 수
const WEATHER_WANTS_BURST: f64 = 5.0;

/// 인증을 마친 연결의 신원임
struct Authed {
    /// 표준 설치 키 (hex)
    key: String,
    /// 허브의 연결 번호
    conn: u64,
}

/// 제어 메시지 하나를 텍스트 프레임으로 씀. 실패면 false임
fn text(ws: &mut WebSocket<TcpStream>, m: &ServerMsg) -> bool {
    ws.write(Message::text(m.to_json())).is_ok()
}

/// `error{reason}`을 보내고 닫음
fn refuse(ws: &mut WebSocket<TcpStream>, reason: &str) {
    let _ = ws.write(Message::text(ServerMsg::Error { reason: reason.into() }.to_json()));
    let _ = ws.close(None);
    let _ = ws.flush();
}

/// 연결 하나를 끝날 때까지 섬김: 챌린지·인증 뒤 메시지 루프를 돌리며 큐를 비우고 한도를 지킴
pub(crate) fn serve(hub: Arc<Hub>, mut ws: WebSocket<TcpStream>, ip: String, admission: &Admission) {
    let lim = hub.limits.clone();
    let _ = ws.get_mut().set_read_timeout(Some(POLL));
    let _ = ws.get_mut().set_write_timeout(Some(Duration::from_secs(5)));
    let _ = ws.get_mut().set_nodelay(true);
    let nonce = proto::random32();
    let challenge =
        ServerMsg::Challenge { relay: proto::to_hex(&hub.public_key), nonce: proto::to_hex(&nonce), v: PROTOCOL_VERSION };
    if ws.send(Message::text(challenge.to_json())).is_err() {
        return;
    }
    let (tx, rx) = sync_channel::<Out>(OUTBOX);
    let link = Link::new(tx, admission);
    let mut poll = POLL;
    let mut last_frame = Instant::now();
    let mut authed: Option<Authed> = None;
    let started = Instant::now();
    let mut last_recv = Instant::now();
    let mut last_code_check = Instant::now();
    let mut msgs = Bucket::new(lim.conn_msgs_per_sec, lim.conn_msgs_burst);
    let mut bytes = Bucket::new(lim.conn_bytes_per_sec, lim.conn_bytes_burst);
    let mut redeems: std::collections::VecDeque<Instant> = Default::default();
    let mut letters_sent: std::collections::VecDeque<Instant> = Default::default();
    let mut new_codes = 0usize;
    let mut friend_updates = Bucket::new(FRIENDS_PER_SEC, FRIENDS_BURST);
    let mut weather_wants = Bucket::new(WEATHER_WANTS_PER_SEC, WEATHER_WANTS_BURST);
    'conn: loop {
        let ending = if hub.stopping() {
            Some("shutdown")
        } else if hub.relay_is_off() {
            Some("relay_off")
        } else {
            link.kill.get().copied()
        };
        if let Some(reason) = ending {
            if reason == "slow" {
                bump(&hub.counters.kicked_slow);
            }
            refuse(&mut ws, reason);
            break;
        }
        let want = poll_for(last_frame.elapsed());
        if want != poll {
            poll = want;
            let _ = ws.get_mut().set_read_timeout(Some(poll));
        }
        // Queue → socket.
        loop {
            match rx.try_recv() {
                Ok(Out::Text(s)) => {
                    if ws.write(Message::text(s)).is_err() {
                        break 'conn;
                    }
                }
                Ok(Out::Binary(b)) => {
                    hub.dequeued(&link, b.len());
                    last_frame = Instant::now();
                    if ws.write(Message::binary(b)).is_err() {
                        break 'conn;
                    }
                }
                Ok(Out::Kick(reason)) => {
                    refuse(&mut ws, &reason);
                    break 'conn;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        match ws.flush() {
            Ok(()) => {}
            Err(WsError::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
        // Socket → hub.
        let msg = match ws.read() {
            Ok(m) => Some(m),
            Err(WsError::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => None,
            Err(WsError::Capacity(_)) => {
                refuse(&mut ws, "too_large");
                break;
            }
            Err(_) => break,
        };
        if let Some(msg) = msg {
            last_recv = Instant::now();
            let len = match &msg {
                Message::Text(t) => t.len(),
                Message::Binary(b) => b.len(),
                _ => 0,
            };
            if !msgs.take(1.0) || !bytes.take(len as f64) {
                bump(&hub.counters.kicked_rate);
                refuse(&mut ws, "rate");
                break;
            }
            match msg {
                Message::Text(t) => {
                    if t.len() > lim.max_text {
                        refuse(&mut ws, "too_large");
                        break;
                    }
                    let Ok(cm) = serde_json::from_str::<ClientMsg>(t.as_str()) else {
                        refuse(&mut ws, "bad_message");
                        break;
                    };
                    match (&authed, cm) {
                        (None, ClientMsg::Auth { id, key, mac, friends, cnonce }) => {
                            match authenticate(&hub, &nonce, &id, &key, &mac) {
                                Some((key, client_pub, shared)) => {
                                    let conn = admission.conn;
                                    match hub.join(&key, &id, &friends, &ip, conn, link.clone()) {
                                        Ok(mut welcome) => {
                                            if let (ServerMsg::Welcome { proof, .. }, Some(cn)) =
                                                (&mut welcome, cnonce.as_deref().and_then(proto::hex32))
                                            {
                                                *proof = Some(proto::to_hex(&proto::welcome_proof(&shared, &cn, &nonce, &client_pub)));
                                            }
                                            if !text(&mut ws, &welcome) {
                                                hub.leave(&key, conn);
                                                break;
                                            }
                                            authed = Some(Authed { key, conn });
                                        }
                                        Err(reason) => {
                                            refuse(&mut ws, reason);
                                            break;
                                        }
                                    }
                                }
                                None => {
                                    bump(&hub.counters.auth_failed);
                                    refuse(&mut ws, "auth");
                                    break;
                                }
                            }
                        }
                        (None, _) | (Some(_), ClientMsg::Auth { .. }) => {
                            refuse(&mut ws, "unexpected");
                            break;
                        }
                        (Some(a), cm) => {
                            let reply = match cm {
                                ClientMsg::Friends { keys } => {
                                    if !friend_updates.take(1.0) {
                                        bump(&hub.counters.kicked_rate);
                                        refuse(&mut ws, "rate");
                                        break;
                                    }
                                    hub.set_friends(&a.key, a.conn, &keys)
                                }
                                ClientMsg::Redeem { code } => {
                                    let now = Instant::now();
                                    while redeems.front().is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60)) {
                                        redeems.pop_front();
                                    }
                                    if redeems.len() >= REDEEMS_PER_CONN_PER_MIN {
                                        Some(ServerMsg::RedeemFailed { code, reason: "rate".into() })
                                    } else {
                                        redeems.push_back(now);
                                        let reply = hub.redeem(&a.key, a.conn, &ip, &code);
                                        let ok = matches!(reply, Some(ServerMsg::Redeemed { .. }));
                                        bump(if ok { &hub.counters.redeems_ok } else { &hub.counters.redeems_failed });
                                        reply
                                    }
                                }
                                ClientMsg::Open { req, to } => hub.open(&a.key, a.conn, req, &to),
                                ClientMsg::Close { ch } => {
                                    hub.close(&a.key, a.conn, ch);
                                    None
                                }
                                ClientMsg::NewCode => {
                                    new_codes += 1;
                                    if new_codes > NEW_CODES_PER_CONN {
                                        None
                                    } else {
                                        hub.new_code(&a.key, a.conn)
                                    }
                                }
                                ClientMsg::Ping => Some(ServerMsg::Pong),
                                ClientMsg::Letter { r#box } => {
                                    // A few tries a minute per connection; the real limits are in the mailbox
                                    let now = Instant::now();
                                    while letters_sent.front().is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60)) {
                                        letters_sent.pop_front();
                                    }
                                    if letters_sent.len() >= LETTERS_PER_CONN_PER_MIN {
                                        Some(ServerMsg::LetterRefused { reason: "daily".into() })
                                    } else {
                                        letters_sent.push_back(now);
                                        hub.letter(&a.key, a.conn, &ip, &r#box)
                                    }
                                }
                                ClientMsg::Letters => hub.letters(&a.key, a.conn),
                                ClientMsg::LetterRead { id } => {
                                    hub.letter_read(&a.key, a.conn, id);
                                    None
                                }
                                ClientMsg::LetterDone { id, done } => {
                                    hub.letter_done(&a.key, a.conn, id, done);
                                    None
                                }
                                ClientMsg::LetterDelete { id } => {
                                    hub.letter_delete(&a.key, a.conn, id);
                                    None
                                }
                                ClientMsg::LetterBlock { from } => {
                                    hub.letter_block(&a.key, a.conn, &from);
                                    None
                                }
                                ClientMsg::LetterReject { id, rejected } => {
                                    hub.letter_reject(&a.key, a.conn, id, rejected);
                                    None
                                }
                                ClientMsg::LetterPrivate { id, private } => {
                                    hub.letter_private(&a.key, a.conn, id, private);
                                    None
                                }
                                ClientMsg::BoardPut { items } => {
                                    hub.board_put(&a.key, a.conn, &items);
                                    None
                                }
                                ClientMsg::BoardList { page } => hub.board_list(&a.key, a.conn, page),
                                ClientMsg::WeatherWant { place } => {
                                    // A place changes rarely: past a few a minute, just ignored
                                    if weather_wants.take(1.0) {
                                        hub.weather_want(&a.key, a.conn, &place)
                                    } else {
                                        None
                                    }
                                }
                                ClientMsg::Auth { .. } => unreachable!("matched above"),
                            };
                            if let Some(r) = reply {
                                if !text(&mut ws, &r) {
                                    break;
                                }
                            }
                        }
                    }
                }
                Message::Binary(b) => {
                    let Some(a) = &authed else {
                        refuse(&mut ws, "unexpected");
                        break;
                    };
                    if b.len() <= HEADER || b.len() > HEADER + MAX_FRAME {
                        refuse(&mut ws, "bad_frame");
                        break;
                    }
                    last_frame = Instant::now();
                    hub.forward(&a.key, a.conn, &ip, b.to_vec());
                }
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
        let now = Instant::now();
        if authed.is_none() && now.duration_since(started) > lim.auth_timeout {
            refuse(&mut ws, "timeout");
            break;
        }
        if now.duration_since(last_recv) > lim.idle_timeout {
            refuse(&mut ws, "idle");
            break;
        }
        if let Some(a) = &authed {
            if now.duration_since(last_code_check) >= Duration::from_secs(5) {
                last_code_check = now;
                if let Some(code) = hub.rotated_code(&a.key, a.conn) {
                    if !text(&mut ws, &code) {
                        break;
                    }
                }
            }
        }
    }
    if let Some(a) = authed {
        hub.leave(&a.key, a.conn);
    }
    // Nothing can be queued any more: this peer has left the hub.
    hub.dequeued(&link, link.queued.load(Ordering::Relaxed));
}

/// 마지막 친구 프레임 뒤 지난 시간에 맞는 읽기 대기를 고름
fn poll_for(since_last_frame: Duration) -> Duration {
    if since_last_frame < ACTIVE_FOR {
        POLL
    } else {
        POLL_IDLE
    }
}

/// `auth`를 확인함: MAC이 클라이언트가 `key`의 개인키를 가졌음을 증명함. 표준 키, 키 바이트, 공유 비밀을 돌려줌
fn authenticate(hub: &Hub, nonce: &[u8; 32], id: &str, key: &str, mac: &str) -> Option<(String, [u8; 32], [u8; 32])> {
    if id.is_empty() || id.len() > MAX_ID || id.chars().any(char::is_control) {
        return None;
    }
    let client_pub = proto::hex32(key)?;
    let mac = proto::hex32(mac)?;
    let shared = proto::dh(&hub.private_key, &client_pub)?;
    let expected = proto::auth_mac(&shared, nonce, &hub.public_key, &client_pub, id);
    proto::ct_eq(&mac, &expected).then(|| (proto::to_hex(&client_pub), client_pub, shared))
}

/// 폴링 주기 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_connections_poll_less_often() {
        assert_eq!(poll_for(Duration::ZERO), POLL);
        assert_eq!(poll_for(ACTIVE_FOR), POLL_IDLE);
    }
}
