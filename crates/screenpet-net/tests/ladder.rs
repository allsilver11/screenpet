//! A ladder game over real links: one host and two players on localhost
//! (spec 2026-11-04 §3, §6). Discovery is unicast: both players announce to
//! the host's port, so the host ("aaaa", the smallest id) dials them both.

use screenpet_net::ladder::{CancelReason, GuestGame, HostGame, LockedResult, Phase, Seat};
use screenpet_net::{Friend, LadderMsg, Net, NetConfig, NetEvent, Timing};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

fn fast() -> Timing {
    Timing {
        announce_interval: Duration::from_millis(150),
        peer_timeout: Duration::from_millis(1500),
        ping_interval: Duration::from_millis(200),
        conn_timeout: Duration::from_millis(1500),
        invite_timeout: Duration::from_millis(3000),
        relay_retry: Duration::from_millis(300),
    }
}

fn free_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn cfg(id: &str, key: [u8; 32], port: u16, to: u16, friends: Vec<Friend>) -> NetConfig {
    let mut c = NetConfig::new(id, id.to_uppercase(), key, friends);
    c.discovery_port = port;
    c.broadcast_to = SocketAddr::from(([127, 0, 0, 1], to));
    c.timing = fast();
    c
}

fn friend(id: &str, key: &[u8; 32]) -> Friend {
    Friend { id: id.into(), public_key: Net::public_key(key) }
}

struct Three {
    host: (Net, Receiver<NetEvent>),
    b: (Net, Receiver<NetEvent>),
    c: (Net, Receiver<NetEvent>),
}

fn three() -> Three {
    let (pa, pb, pc) = (free_udp_port(), free_udp_port(), free_udp_port());
    let (ka, kb, kc) = (Net::generate_static_key(), Net::generate_static_key(), Net::generate_static_key());
    let host = Net::start(cfg("aaaa", ka, pa, pb, vec![friend("bbbb", &kb), friend("cccc", &kc)])).unwrap();
    let b = Net::start(cfg("bbbb", kb, pb, pa, vec![friend("aaaa", &ka)])).unwrap();
    let c = Net::start(cfg("cccc", kc, pc, pa, vec![friend("aaaa", &ka)])).unwrap();
    let t = Three { host, b, c };
    let deadline = Instant::now() + Duration::from_secs(8);
    while (t.host.0.peer_version("bbbb").is_none() || t.host.0.peer_version("cccc").is_none()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(t.host.0.send_ladder("bbbb", LadderMsg::LadderStart { game_id: "warmup".into(), mode: "all".into() }), "host ↔ b linked");
    assert!(t.host.0.send_ladder("cccc", LadderMsg::LadderStart { game_id: "warmup".into(), mode: "all".into() }), "host ↔ c linked");
    t
}

fn seat(name: &str) -> Seat {
    Seat { name: name.into(), pet: "cat".into(), style: String::new() }
}

fn draft() -> Vec<LockedResult> {
    ["커피", "통과", "통과"].iter().map(|t| LockedResult { text: t.to_string(), mood: "none".into() }).collect()
}

/// Messages to a friend who already left are dropped (like the app does).
fn send_all(net: &Net, out: Vec<(String, LadderMsg)>) {
    for (to, m) in out {
        net.send_ladder(&to, m);
    }
}

/// One player's loop step: join invites, answer the host.
fn step_player(net: &Net, rx: &Receiver<NetEvent>, game: &mut Option<GuestGame>, name: &str) {
    while let Ok(ev) = rx.try_recv() {
        let now = Instant::now();
        match ev {
            NetEvent::Ladder { from, msg: msg @ LadderMsg::LadderInvite { .. } } => {
                let mut g = GuestGame::on_invite(&from, msg, now).expect("valid invite");
                let join = g.join(seat(name), now).unwrap();
                assert!(net.send_ladder(&from, join));
                *game = Some(g);
            }
            NetEvent::Ladder { from, msg } => {
                if let Some(g) = game.as_mut().filter(|g| g.host_id() == from) {
                    if let Some(reply) = g.on_message(msg, now) {
                        assert!(net.send_ladder(&from, reply));
                    }
                }
            }
            NetEvent::Disconnected { id } => {
                if let Some(g) = game.as_mut().filter(|g| g.host_id() == id) {
                    g.on_host_disconnected();
                }
            }
            _ => {}
        }
    }
}

fn step_host(net: &Net, rx: &Receiver<NetEvent>, game: &mut HostGame) {
    while let Ok(ev) = rx.try_recv() {
        let now = Instant::now();
        match ev {
            NetEvent::Ladder { from, msg } => send_all(net, game.on_message(&from, msg, now)),
            NetEvent::Disconnected { id } => send_all(net, game.on_disconnected(&id, now)),
            _ => {}
        }
    }
    send_all(net, game.tick(Instant::now()));
}

#[test]
fn three_installs_agree_on_the_seed_and_print() {
    let t = three();
    let now = Instant::now();
    let (mut host, out) = HostGame::new(seat("방장"), &draft(), &["bbbb".into(), "cccc".into()], now).unwrap();
    send_all(&t.host.0, out);
    let (mut gb, mut gc) = (None, None);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut started = false;
    while Instant::now() < deadline {
        step_host(&t.host.0, &t.host.1, &mut host);
        step_player(&t.b.0, &t.b.1, &mut gb, "비");
        step_player(&t.c.0, &t.c.1, &mut gc, "씨");
        if !started && host.all_answered() {
            send_all(&t.host.0, host.start(&draft(), Instant::now()).expect("lockable"));
            started = true;
        }
        let all_done = [gb.as_ref().map(GuestGame::phase), gc.as_ref().map(GuestGame::phase), Some(host.phase())]
            .iter()
            .all(|p| matches!(p, Some(Phase::Done(_))));
        if all_done {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Phase::Done(h) = host.phase() else { panic!("host: {:?}", host.phase()) };
    for g in [&gb, &gc] {
        let Some(Phase::Done(o)) = g.as_ref().map(GuestGame::phase) else { panic!("player not done") };
        assert_eq!((o.seed, o.print, &o.players), (h.seed, h.print, &h.players));
    }
    let names: Vec<&str> = h.players.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"방장") && names.contains(&"비") && names.contains(&"씨"), "{names:?}");
}

#[test]
fn a_player_dropping_after_the_lock_cancels_with_its_name() {
    let t = three();
    let now = Instant::now();
    let (mut host, out) = HostGame::new(seat("방장"), &draft(), &["bbbb".into(), "cccc".into()], now).unwrap();
    send_all(&t.host.0, out);
    let (mut gb, mut gc) = (None, None);
    let mut c = Some(t.c);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut started = false;
    while Instant::now() < deadline {
        step_host(&t.host.0, &t.host.1, &mut host);
        // b는 잠근 뒤로 읽지 않는다 (공개가 늦는다). c는 잠그기를 받은 것을 확인하고 공개 전에 꺼진다 (아래)
        if !started {
            step_player(&t.b.0, &t.b.1, &mut gb, "비");
        }
        if let Some(c) = &c {
            step_player(&c.0, &c.1, &mut gc, "씨");
        }
        if !started && host.all_answered() {
            send_all(&t.host.0, host.start(&draft(), Instant::now()).expect("lockable"));
            started = true;
        }
        if started && c.is_some() && matches!(gc.as_ref().map(GuestGame::phase), Some(Phase::Locked)) {
            c.take().unwrap().0.shutdown();
        }
        if host.phase().is_over() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(host.phase(), Phase::Cancelled { reason: CancelReason::Left, who: Some(n) } if n == "씨"), "{:?}", host.phase());
    // b에게도 취소가 간다
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && !gb.as_ref().unwrap().phase().is_over() {
        step_player(&t.b.0, &t.b.1, &mut gb, "비");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(gb.as_ref().unwrap().phase(), Phase::Cancelled { reason: CancelReason::Left, who: Some(n) } if n == "씨"));
}

#[test]
fn net_and_core_agree_on_the_limits() {
    use screenpet_core::ladder as core;
    use screenpet_net::ladder as net;
    assert_eq!((net::MIN_PLAYERS, net::MAX_PLAYERS, net::RESULT_MAX_CHARS), (core::MIN_PLAYERS, core::MAX_PLAYERS, core::RESULT_MAX_CHARS));
}
