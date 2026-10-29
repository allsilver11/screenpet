//! One client connection (after the WebSocket upgrade): challenge →
//! authentication → control messages and frame forwarding, with per
//! connection rate limits and timeouts.

use crate::hub::{Bucket, Hub, Out};
use crate::proto::{self, ClientMsg, ServerMsg, HEADER, MAX_FRAME, MAX_ID, PROTOCOL_VERSION};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::{Error as WsError, Message, WebSocket};

/// Messages queued to one connection before it counts as stuck.
const OUTBOX: usize = 512;
/// How long a read waits before the loop checks its queue and timers.
const POLL: Duration = Duration::from_millis(15);
/// Most redeem attempts per connection per minute.
const REDEEMS_PER_CONN_PER_MIN: usize = 10;
/// Most "new code" requests per connection.
const NEW_CODES_PER_CONN: usize = 5;

struct Authed {
    key: String,
    conn: u64,
}

fn text(ws: &mut WebSocket<TcpStream>, m: &ServerMsg) -> bool {
    ws.write(Message::text(m.to_json())).is_ok()
}

/// Send `error{reason}` and close.
fn refuse(ws: &mut WebSocket<TcpStream>, reason: &str) {
    let _ = ws.write(Message::text(ServerMsg::Error { reason: reason.into() }.to_json()));
    let _ = ws.close(None);
    let _ = ws.flush();
}

pub(crate) fn serve(hub: Arc<Hub>, mut ws: WebSocket<TcpStream>, ip: String) {
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
    let kill = Arc::new(AtomicBool::new(false));
    let mut authed: Option<Authed> = None;
    let started = Instant::now();
    let mut last_recv = Instant::now();
    let mut last_code_check = Instant::now();
    let mut msgs = Bucket::new(lim.conn_msgs_per_sec, lim.conn_msgs_burst);
    let mut bytes = Bucket::new(lim.conn_bytes_per_sec, lim.conn_bytes_burst);
    let mut redeems: std::collections::VecDeque<Instant> = Default::default();
    let mut new_codes = 0usize;
    'conn: loop {
        if hub.stopping() || kill.load(Ordering::SeqCst) {
            refuse(&mut ws, if kill.load(Ordering::SeqCst) { "slow" } else { "shutdown" });
            break;
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
                                    let conn = hub.conn_id();
                                    match hub.join(&key, &id, &friends, &ip, conn, tx.clone(), kill.clone()) {
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
                                ClientMsg::Friends { keys } => hub.set_friends(&a.key, a.conn, &keys),
                                ClientMsg::Redeem { code } => {
                                    let now = Instant::now();
                                    while redeems.front().is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60)) {
                                        redeems.pop_front();
                                    }
                                    if redeems.len() >= REDEEMS_PER_CONN_PER_MIN {
                                        Some(ServerMsg::RedeemFailed { code, reason: "rate".into() })
                                    } else {
                                        redeems.push_back(now);
                                        hub.redeem(&a.key, a.conn, &ip, &code)
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
                    hub.forward(&a.key, a.conn, b.to_vec());
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
}

/// Check `auth`: the MAC proves the client holds the private key for `key`.
/// Returns the canonical key, the key bytes and the shared secret.
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
