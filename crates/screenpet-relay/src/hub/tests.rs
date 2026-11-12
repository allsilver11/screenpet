//! 허브 단위 시험: 예전 hub.rs의 시험을 그대로 옮김.

use super::*;
use std::sync::mpsc::{sync_channel, Receiver};

fn hub(limits: Limits) -> Hub {
    Hub::new(limits, Vec::new(), proto::random32(), Store::memory(), Control::default(), None)
}

/// Every text message queued for a peer so far, parsed.
fn texts(rx: &Receiver<Out>) -> Vec<ServerMsg> {
    rx.try_iter().filter_map(|o| if let Out::Text(t) = o { serde_json::from_str(&t).ok() } else { None }).collect()
}

/// A reader (install 1) and a known sender (install 2, seen on 3 days).
fn mailbox_hub() -> (Hub, (String, u64, Receiver<Out>), (String, u64, Receiver<Out>)) {
    let hub = hub(Limits::default());
    let (reader, rconn, _rl, rrx) = peer_at(&hub, "198.51.100.5", 1, &[], 64);
    hub.admin_mailbox_reader("add", &reader).unwrap();
    let (sender, sconn, _sl, srx) = peer_at(&hub, "198.51.100.6", 2, &[], 64);
    hub.state().store().set_seen_days(&sender, 3);
    texts(&rrx);
    (hub, (reader, rconn, rrx), (sender, sconn, srx))
}

#[test]
fn a_letter_is_stored_pushed_to_online_readers_and_listed_only_for_them() {
    let (hub, (reader, rconn, rrx), (sender, sconn, _srx)) = mailbox_hub();
    assert_eq!(hub.letter(&sender, sconn, "198.51.100.6", "abcd"), Some(ServerMsg::LetterOk));
    let pushed = texts(&rrx);
    assert!(matches!(&pushed[..], [ServerMsg::LetterNew { item }] if item.r#box == "abcd" && item.from == sender), "{pushed:?}");
    assert!(matches!(hub.letters(&reader, rconn), Some(ServerMsg::Letters { items, .. }) if items.len() == 1));
    assert_eq!(hub.letters(&sender, sconn), None, "not a reader: ignored");
    for _ in 1..crate::mailbox::PER_KEY {
        assert_eq!(hub.letter(&sender, sconn, "198.51.100.6", "abcd"), Some(ServerMsg::LetterOk));
    }
    texts(&rrx);
    assert_eq!(hub.letter(&sender, sconn, "198.51.100.6", "abcd"), Some(ServerMsg::LetterRefused { reason: "daily".into() }), "the 6th in a day");
    assert_eq!(hub.letter(&sender, sconn, "198.51.100.6", "zz"), Some(ServerMsg::LetterRefused { reason: "size".into() }));
}

#[test]
fn a_brand_new_install_cannot_send() {
    let hub = hub(Limits::default());
    let (k, c, _l, _rx) = peer_at(&hub, "198.51.100.7", 3, &[], 8);
    assert_eq!(hub.letter(&k, c, "198.51.100.7", "abcd"), Some(ServerMsg::LetterRefused { reason: "new".into() }));
}

#[test]
fn readers_mark_read_delete_and_block_and_every_reader_hears_it() {
    let (hub, (reader, rconn, rrx), (sender, sconn, _srx)) = mailbox_hub();
    let (reader2, r2conn, _l2, r2rx) = peer_at(&hub, "198.51.100.8", 4, &[], 64);
    hub.admin_mailbox_reader("add", &reader2).unwrap();
    hub.letter(&sender, sconn, "198.51.100.6", "abcd");
    texts(&rrx);
    texts(&r2rx);
    let Some(ServerMsg::Letters { items, .. }) = hub.letters(&reader, rconn) else { panic!() };
    let id = items[0].id;
    hub.letter_read(&reader, rconn, id);
    assert_eq!(texts(&r2rx), vec![ServerMsg::LettersChanged]);
    assert_eq!(texts(&rrx), vec![ServerMsg::LettersChanged], "the reader who changed it hears it too");
    assert!(matches!(hub.letters(&reader2, r2conn), Some(ServerMsg::Letters { items, .. }) if items[0].read));
    hub.letter_done(&reader2, r2conn, id, true);
    assert_eq!(texts(&rrx), vec![ServerMsg::LettersChanged]);
    assert!(matches!(hub.letters(&reader, rconn), Some(ServerMsg::Letters { items, .. }) if items[0].done));
    hub.letter_done(&sender, sconn, id, false);
    assert!(texts(&rrx).is_empty(), "a non-reader can't tick it");
    hub.letter_block(&reader2, r2conn, &sender);
    assert_eq!(texts(&rrx), vec![ServerMsg::LettersChanged]);
    assert!(matches!(hub.letters(&reader, rconn), Some(ServerMsg::Letters { items, .. }) if items.is_empty()));
    // a non-reader can't delete or block
    hub.letter_block(&sender, sconn, &reader);
    assert!(texts(&rrx).is_empty());
    assert_eq!(hub.admin_mailbox_reader("remove", &reader2).unwrap(), format!("mailbox reader removed: {reader2}"));
    assert_eq!(hub.letters(&reader2, r2conn), None);
    let _ = id;
    hub.letter_delete(&reader, rconn, 12345);
}

fn put(id: u64, name: &str, line: &str) -> proto::BoardIn {
    proto::BoardIn { id, name: name.into(), line: line.into() }
}

fn board_of(m: Option<ServerMsg>) -> (Vec<crate::board::Item>, u32, u32, u32) {
    match m {
        Some(ServerMsg::BoardPage { items, page, pages, total }) => (items, page, pages, total),
        other => panic!("{other:?}"),
    }
}

#[test]
fn only_mailbox_readers_put_on_the_board_and_anyone_can_read_it() {
    let (hub, (reader, rconn, _rrx), (sender, sconn, _srx)) = mailbox_hub();
    hub.letter(&sender, sconn, "198.51.100.6", "abcd");
    let Some(ServerMsg::Letters { items, board }) = hub.letters(&reader, rconn) else { panic!() };
    assert!(board.is_empty());
    let id = items[0].id;
    hub.board_put(&sender, sconn, &[put(id, "*", "가짜")]);
    assert!(board_of(hub.board_list(&sender, sconn, 1)).0.is_empty(), "a sender can't put");
    hub.board_put(&reader, rconn, &[put(id, "김철수", "안 가려진 이름"), put(id + 1, "*", "없는 편지")]);
    std::thread::sleep(Duration::from_millis(510));
    assert!(board_of(hub.board_list(&sender, sconn, 1)).0.is_empty(), "bad masks and unknown letters are refused");
    hub.board_put(&reader, rconn, &[put(id, "김**", "펭귄 더 주세요")]);
    let (items, page, pages, total) = board_of(hub.board_list(&reader, rconn, 1));
    assert_eq!((page, pages, total, items[0].name.as_str(), items[0].line.as_str()), (1, 1, 1, "김**", "펭귄 더 주세요"));
    assert!(matches!(hub.letters(&reader, rconn), Some(ServerMsg::Letters { board, .. }) if board == vec![id]));
    // too many at once: all ignored
    let many: Vec<_> = (0..=crate::board::PUT_MAX as u64).map(|i| put(id, "*", &format!("{i}"))).collect();
    hub.board_put(&reader, rconn, &many);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&reader, rconn, 1)).0[0].line, "펭귄 더 주세요");
    // 반려 / 반영 show on the board
    hub.letter_reject(&reader, rconn, id, true);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&sender, sconn, 1)).0[0].status, crate::board::Status::Rejected);
    hub.letter_reject(&sender, sconn, id, false);
    hub.letter_done(&reader, rconn, id, true);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&sender, sconn, 1)).0[0].status, crate::board::Status::Done, "a non-reader can't clear it; 반영 replaces 반려");
    // deleting the letter takes its entry away
    hub.letter_delete(&reader, rconn, id);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&sender, sconn, 1)), (vec![], 1, 1, 0));
}

#[test]
fn only_readers_make_a_letter_private_and_then_nobody_else_sees_it() {
    let (hub, (reader, rconn, rrx), (sender, sconn, _srx)) = mailbox_hub();
    hub.letter(&sender, sconn, "198.51.100.6", "abcd");
    let Some(ServerMsg::Letters { items, .. }) = hub.letters(&reader, rconn) else { panic!() };
    let id = items[0].id;
    hub.board_put(&reader, rconn, &[put(id, "김**", "비밀일 수도")]);
    texts(&rrx);
    hub.letter_private(&sender, sconn, id, true);
    assert!(texts(&rrx).is_empty(), "a non-reader can't hide it");
    assert_eq!(board_of(hub.board_list(&sender, sconn, 1)).3, 1);
    hub.letter_private(&reader, rconn, id, true);
    assert_eq!(texts(&rrx), vec![ServerMsg::LettersChanged]);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&sender, sconn, 1)), (vec![], 1, 1, 0), "not in items, total or pages");
    hub.board_put(&reader, rconn, &[put(id, "김**", "다시")]);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&reader, rconn, 1)).3, 0, "putting again doesn't unhide it, even for a reader's board");
    assert!(matches!(hub.letters(&reader, rconn), Some(ServerMsg::Letters { items, .. }) if items[0].private));
    hub.letter_private(&sender, sconn, id, false);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&sender, sconn, 1)).3, 0, "a non-reader can't unhide it");
    hub.letter_private(&reader, rconn, id, false);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&sender, sconn, 1)).0[0].line, "다시");
}

#[test]
fn board_pages_are_rate_limited_per_connection() {
    let hub = hub(Limits { board_gap: Duration::from_millis(200), ..Limits::default() });
    let (a, ac, _al, _arx) = peer(&hub, 1, &[], 8);
    let (b, bc, _bl, _brx) = peer(&hub, 2, &[], 8);
    assert!(hub.board_list(&a, ac, 1).is_some());
    assert_eq!(hub.board_list(&a, ac, 1), None, "too soon: ignored");
    assert!(hub.board_list(&b, bc, 1).is_some(), "another connection has its own");
    std::thread::sleep(Duration::from_millis(210));
    assert!(hub.board_list(&a, ac, 2).is_some());
    assert_eq!(hub.board_list(&a, ac + 999, 1), None, "not this connection");
}

#[test]
fn blocking_a_sender_takes_their_board_entries_away() {
    let (hub, (reader, rconn, _rrx), (sender, sconn, _srx)) = mailbox_hub();
    hub.letter(&sender, sconn, "198.51.100.6", "abcd");
    hub.letter(&sender, sconn, "198.51.100.6", "abce");
    let Some(ServerMsg::Letters { items, .. }) = hub.letters(&reader, rconn) else { panic!() };
    assert_eq!(items.len(), 2);
    hub.board_put(&reader, rconn, &items.iter().map(|l| put(l.id, "*", "x")).collect::<Vec<_>>());
    assert_eq!(board_of(hub.board_list(&reader, rconn, 1)).3, 2);
    hub.letter_block(&reader, rconn, &sender);
    std::thread::sleep(Duration::from_millis(510));
    assert_eq!(board_of(hub.board_list(&reader, rconn, 1)).3, 0);
}

/// Join install `n`, friends with `friends`, on a queue nobody drains.
fn peer(hub: &Hub, n: u8, friends: &[u8], cap: usize) -> (String, u64, Link, Receiver<Out>) {
    peer_at(hub, "203.0.113.1", n, friends, cap)
}

fn peer_at(hub: &Hub, ip: &str, n: u8, friends: &[u8], cap: usize) -> (String, u64, Link, Receiver<Out>) {
    let key = proto::to_hex(&[n; 32]);
    let friends: Vec<String> = friends.iter().map(|f| proto::to_hex(&[*f; 32])).collect();
    let (tx, rx) = sync_channel(cap);
    let admission = hub.admit_ip(ip).ok().expect("admitted");
    let link = Link::new(tx, &admission);
    hub.join(&key, "x", &friends, ip, admission.conn, link.clone()).unwrap();
    (key, admission.conn, link, rx)
}

/// Two mutual friends and a channel between them, `b` never reading.
fn stuck_pair(hub: &Hub) -> (String, u64, Link, Link, u32, Receiver<Out>, Receiver<Out>) {
    let (ka, ca, la, ra) = peer(hub, 1, &[2], 512);
    let (kb, _, lb, rb) = peer(hub, 2, &[1], 512);
    let Some(ServerMsg::Opened { ch, .. }) = hub.open(&ka, ca, 1, &kb) else { panic!("no channel") };
    (ka, ca, la, lb, ch, ra, rb)
}

fn frame(ch: u32, len: usize) -> Vec<u8> {
    let mut m = ch.to_be_bytes().to_vec();
    m.resize(4 + len, 0);
    m
}

#[test]
fn a_full_queue_closes_the_channel_and_only_a_stuck_one_disconnects() {
    let limits = Limits { max_outbox_bytes: 200_000, chan_msgs_burst: 1000.0, slow_after: Duration::from_millis(100), ..Limits::default() };
    let h = hub(limits);
    let (ka, ca, _la, lb, ch, _ra, _rb) = stuck_pair(&h);
    // The old limit was 512 messages, so 33 MB of 64 KB frames.
    for _ in 0..3 {
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
    }
    assert_eq!(lb.queued.load(Ordering::Relaxed), 180_012);
    assert!(lb.kill.get().is_none());
    h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
    assert!(lb.kill.get().is_none(), "a sender filling the queue fast can't get the receiver disconnected");
    assert_eq!(lb.queued.load(Ordering::Relaxed), 180_012, "the refused frame is not counted");
    assert!(h.state().channels.is_empty(), "its channel is closed");
    // A queue that drains is fine however long it has been around…
    std::thread::sleep(Duration::from_millis(150));
    h.dequeued(&lb, 4);
    let Some(ServerMsg::Opened { ch, .. }) = h.open(&ka, ca, 2, &proto::to_hex(&[2; 32])) else { panic!("no channel") };
    h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
    assert!(lb.kill.get().is_none());
    // …one that has not moved for `slow_after` is not.
    std::thread::sleep(Duration::from_millis(150));
    let Some(ServerMsg::Opened { ch, .. }) = h.open(&ka, ca, 3, &proto::to_hex(&[2; 32])) else { panic!("no channel") };
    h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
    assert_eq!(lb.kill.get(), Some(&"slow"));
}

#[test]
fn one_network_using_up_the_total_queue_stops_only_itself() {
    let limits = Limits { max_total_outbox_bytes: 64_000, chan_msgs_burst: 1000.0, ..Limits::default() };
    let h = hub(limits);
    // 1 → 2 stuck in one network; 3 → 4 with its receiver in another.
    let (k1, c1, _, _r1) = peer_at(&h, "203.0.113.1", 1, &[2], 512);
    let (k2, _, _, _r2) = peer_at(&h, "203.0.113.2", 2, &[1], 512);
    let (k3, c3, _, _r3) = peer_at(&h, "192.0.2.1", 3, &[4], 512);
    let (k4, _, _, _r4) = peer_at(&h, "198.51.100.1", 4, &[3], 512);
    let Some(ServerMsg::Opened { ch: heavy, .. }) = h.open(&k1, c1, 1, &k2) else { panic!() };
    let Some(ServerMsg::Opened { ch: light, .. }) = h.open(&k3, c3, 1, &k4) else { panic!() };
    h.forward(&k1, c1, "203.0.113.1", frame(heavy, 63_000));
    h.forward(&k1, c1, "203.0.113.1", frame(heavy, 63_000));
    assert!(!h.state().channels.contains_key(&heavy), "over the total and its share");
    h.forward(&k3, c3, "192.0.2.1", frame(light, 500));
    assert!(h.state().channels.contains_key(&light), "within its share");
}

#[test]
fn the_global_byte_budget_keeps_a_share_per_network_up_to_a_ceiling() {
    // Budget 160 KB (ceiling 320 KB), each network's share 10 KB.
    let limits = Limits { global_bytes_per_sec: 1.0, global_bytes_burst: 160_000.0, chan_msgs_burst: 1000.0, ..Limits::default() };
    let h = hub(limits);
    let (ka, ca, _, _ra, ) = peer(&h, 1, &[2], 512);
    let (kb, _, _, _rb) = peer(&h, 2, &[1], 512);
    let open = |req| {
        let Some(ServerMsg::Opened { ch, .. }) = h.open(&ka, ca, req, &kb) else { panic!("no channel") };
        ch
    };
    let alive = |ch| h.state().channels.contains_key(&ch);
    let ch = open(1);
    for _ in 0..16 {
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 9_000));
    }
    // Budget nearly gone: this source, past its share, loses its channel.
    h.forward(&ka, ca, "203.0.113.1", frame(ch, 9_000));
    h.forward(&ka, ca, "203.0.113.1", frame(ch, 9_000));
    assert!(!alive(ch));
    // Another network within its share goes through.
    let ch = open(2);
    h.forward(&ka, ca, "198.51.100.1", frame(ch, 9_000));
    assert!(alive(ch));
    // Enough different networks reach the ceiling.
    let mut n = 0;
    while alive(ch) {
        n += 1;
        assert!(n < 40, "no ceiling");
        h.forward(&ka, ca, &format!("10.{n}.0.1"), frame(ch, 9_000));
    }
}

#[test]
fn byte_buckets_are_kept_until_refilled_in_a_bounded_table() {
    let h = hub(Limits { ip_bytes_per_sec: 1.0, ip_bytes_burst: 1000.0, ..Limits::default() });
    let a = h.admit_ip("203.0.113.1").ok().unwrap();
    assert!(h.bandwidth_allows(&mut h.state(), "203.0.113.1", 900.0));
    h.release_ip("203.0.113.1", a.conn);
    h.maintain();
    assert!(!h.bandwidth_allows(&mut h.state(), "203.0.113.1", 900.0), "reconnecting doesn't refill it");

    let h = hub(Limits { ip_bytes_per_sec: 1e9, ip_bytes_burst: 1000.0, ..Limits::default() });
    assert!(h.bandwidth_allows(&mut h.state(), "203.0.113.1", 900.0));
    std::thread::sleep(Duration::from_millis(5));
    h.maintain();
    assert!(h.state().ip_bytes.is_empty(), "refilled: dropped");

    let mut t = HashMap::new();
    bucket_in(&mut t, "used", 1.0, 100.0, 2).take(90.0);
    bucket_in(&mut t, "fresh", 1.0, 100.0, 2);
    bucket_in(&mut t, "new", 1.0, 100.0, 2);
    assert_eq!(t.len(), 2);
    assert!(t.contains_key("used") && !t.contains_key("fresh"), "the fullest goes");
}

#[test]
fn an_evicted_connection_cannot_join() {
    let h = hub(Limits { max_unauthed_per_ip: 1, ..Limits::default() });
    let first = h.admit_ip("203.0.113.1").ok().unwrap();
    let second = h.admit_ip("203.0.113.1").ok().unwrap();
    assert_eq!(first.kill.get(), Some(&"busy"));
    assert!(second.kill.get().is_none());
    let (tx, _rx) = sync_channel(1);
    let link = Link::new(tx, &first);
    assert_eq!(h.join(&proto::to_hex(&[1; 32]), "x", &[], "203.0.113.1", first.conn, link).err(), Some("busy"));
    // Its slot is counted once when it finally ends.
    h.release_ip("203.0.113.1", first.conn);
    h.release_ip("203.0.113.1", second.conn);
    let st = h.state();
    assert_eq!((st.all_conns.total, st.all_conns.unauthed), (0, 0));
    assert!(st.ip_conns.is_empty() && st.net_conns.is_empty() && st.net_queued.is_empty());
}

#[test]
fn too_many_queued_bytes_in_all_close_the_channel_only() {
    let limits = Limits { max_total_outbox_bytes: 100_000, chan_msgs_burst: 1000.0, ..Limits::default() };
    let h = hub(limits);
    let (ka, ca, _la, lb, ch, _ra, _rb) = stuck_pair(&h);
    h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
    h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
    assert!(lb.kill.get().is_none());
    assert!(h.state().channels.is_empty());
}

#[test]
fn a_replaced_connection_with_a_full_queue_still_goes() {
    let h = hub(Limits::default());
    let (_, _, old, _rx) = peer(&h, 1, &[], 1);
    old.out.try_send(Out::Text("filler".into())).unwrap();
    let (_, _, _new, _rx2) = peer(&h, 1, &[], 8);
    assert_eq!(old.kill.get(), Some(&"replaced"), "it could not be told through the queue");
}

#[test]
fn online_installs_are_kept_fresh_for_after_a_restart() {
    let h = hub(Limits::default());
    let (key, _, _, _rx) = peer(&h, 1, &[], 8);
    let long_ago = unix_now() - 2 * 24 * 3600;
    h.state().store().touch_at_for_test(&key, long_ago);
    h.prune_store();
    assert!(h.state().store_ref().get(&key).unwrap().last_seen > long_ago);
}

#[test]
fn a_full_queue_is_slow() {
    let h = hub(Limits::default());
    let (_, _, a, _rx) = peer(&h, 1, &[2], 1);
    a.out.try_send(Out::Text("filler".into())).unwrap();
    let (_, _, _b, _rx2) = peer(&h, 2, &[1], 8); // presence for 1 doesn't fit
    assert_eq!(a.kill.get(), Some(&"slow"));
}

#[test]
fn a_full_rate_table_evicts_instead_of_refusing_newcomers() {
    let mut t = HashMap::new();
    for i in 0..4 {
        assert!(allow(&mut t, &format!("k{i}"), 1, HOUR, 4));
    }
    // Table full of live keys: a new key still gets its own budget.
    assert!(allow(&mut t, "new", 1, HOUR, 4));
    assert!(t.len() <= 4);
    assert!(!allow(&mut t, "new", 1, HOUR, 4), "and its limit still applies");
}

#[test]
fn one_network_cannot_fill_a_table_with_its_own_keys() {
    let mut t = HashMap::new();
    for i in 0..100u32 {
        let key = format!("2001:db8:1:{i:x}::/64");
        allow(&mut t, &key, 1000, HOUR, 4096);
    }
    assert!(t.len() <= MAX_KEYS_PER_PARENT + 1, "{} keys from one /48", t.len());
    let mut t = HashMap::new();
    for i in 0..100u32 {
        allow(&mut t, &format!("203.0.113.{i}"), 1000, HOUR, 4096);
    }
    assert!(t.len() <= MAX_KEYS_PER_PARENT + 1, "{} keys from one /24", t.len());
}

#[test]
fn the_per_code_table_fails_closed_for_new_targets_only() {
    let mut t = HashMap::new();
    assert!(allow_distinct(&mut t, "t1 c", "x", 5, HOUR, 2));
    assert!(allow_distinct(&mut t, "t2 c", "x", 5, HOUR, 2));
    assert!(!allow_distinct(&mut t, "t3 c", "x", 5, HOUR, 2), "full: no eviction of live caps");
    assert!(allow_distinct(&mut t, "t1 c", "y", 5, HOUR, 2), "known targets keep working");
    assert!(t.contains_key("t1 c") && t.contains_key("t2 c"));
}

#[test]
fn wide_buckets() {
    assert_eq!(wide_bucket("203.0.113.9"), "203.0.0.0/16");
    assert_eq!(wide_bucket("2001:db8:1:2::/64"), "2001:db8::/32");
}

#[test]
fn share_buckets() {
    assert_eq!(share_bucket("203.0.113.9"), "203.0.113.0/24");
    assert_eq!(share_bucket("2001:db8:1:2::/64"), "2001:db8:1::/48");
}

fn weathers(rx: &Receiver<Out>) -> Vec<ServerMsg> {
    texts(rx).into_iter().filter(|m| matches!(m, ServerMsg::Weather { .. })).collect()
}

fn due_ids(hub: &Hub) -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = hub.weather_due().iter().map(|p| p.id).collect();
    ids.sort();
    ids
}

#[test]
fn apps_get_yongin_weather_until_they_ask_for_another_place() {
    let hub = hub(Limits::default());
    let (_a, _, _la, ra) = peer(&hub, 1, &[], 64);
    assert_eq!(due_ids(&hub), vec!["yongin"], "a connected app wants Yongin");
    assert!(weathers(&ra).is_empty(), "nothing known yet");
    hub.weather_fetched("yongin", Some(73));
    let got = weathers(&ra);
    assert!(matches!(&got[..], [ServerMsg::Weather { place, code: 73, at }] if place == "yongin" && *at > 0), "{got:?}");
    // The next app gets the known reading right after its welcome
    let (_b, _, _lb, rb) = peer(&hub, 2, &[], 64);
    assert!(matches!(weathers(&rb)[..], [ServerMsg::Weather { code: 73, .. }]));
    assert!(due_ids(&hub).is_empty(), "fetched half an hour ago");
}

#[test]
fn an_app_can_switch_to_another_place_and_gets_only_that_places_weather() {
    let hub = hub(Limits::default());
    let (a, ca, _la, ra) = peer(&hub, 1, &[], 64);
    let (_b, _, _lb, rb) = peer(&hub, 2, &[], 64);
    assert_eq!(hub.weather_want(&a, ca, "seoul"), None, "not fetched yet");
    assert_eq!(due_ids(&hub), vec!["seoul", "yongin"]);
    hub.weather_fetched("seoul", Some(0));
    hub.weather_fetched("yongin", Some(61));
    assert!(matches!(&weathers(&ra)[..], [ServerMsg::Weather { code: 0, place, .. }] if place == "seoul"));
    assert!(matches!(weathers(&rb)[..], [ServerMsg::Weather { code: 61, .. }]));
    // Back to Yongin: the known reading at once
    assert!(matches!(hub.weather_want(&a, ca, "yongin"), Some(ServerMsg::Weather { code: 61, .. })));
    // A failed fetch keeps the last reading and tells nobody
    hub.weather_fetched("seoul", None);
    assert!(weathers(&ra).is_empty() && weathers(&rb).is_empty());
}

#[test]
fn unknown_places_and_other_connections_change_nothing() {
    let hub = hub(Limits::default());
    let (a, ca, _la, _ra) = peer(&hub, 1, &[], 64);
    for bad in ["", "tokyo", "37.5,127.0", "Seoul", &"z".repeat(5000)] {
        assert_eq!(hub.weather_want(&a, ca, bad), None, "{bad:?}");
    }
    assert_eq!(hub.weather_want(&a, ca + 100, "seoul"), None, "an old connection");
    assert_eq!(due_ids(&hub), vec!["yongin"], "still on Yongin, no new place");
    assert_eq!(hub.state().weathers.len(), 1);
}
