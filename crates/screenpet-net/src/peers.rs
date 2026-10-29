//! Per-link session driver, transport-independent (frames in, frames out).
//!
//! 1. Noise XX handshake (dialer = initiator) → peer's static public key.
//! 2. Encrypted `hello{id, name}`. The link is a *friend* link iff a friend
//!    with that id is pinned to exactly that key (an id with a different key,
//!    or a pinned key under a different id, is rejected). On an outgoing link
//!    the id must equal the id we dialed.
//! 3. Otherwise it is a *stranger* link, which may only pair, and only on the
//!    link the inviter dialed: `invite{commit}` → `pair_random` →
//!    `pair_reveal` → both sides show the same 6-emoji SAS → both users
//!    confirm with `invite_reply{accepted}` → both pin each other's key.

use crate::discovery::pick_candidate;
use crate::noise::{self, Established};
use crate::protocol::{clamp_name, Control, Message, MAX_FRAME, MAX_ID, PROTOCOL_VERSION};
use crate::secure::SecureLink;
use crate::transport::{self, FrameLink, FrameReader, Recv};
use crate::{
    Conn, Kind, NetEvent, PairStage, Pairing, SessionGuard, Shared, State, INVITED_WINDOW, MAX_INVITED_PER_WINDOW,
    MAX_INVITES_IN, MAX_PAIR_ATTEMPTS_PER_ADDR,
};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn ctl(c: Control) -> Message {
    Message::Control(c)
}

/// Is `token` the session currently registered for `id`?
fn current(st: &State, id: &str, token: u64) -> bool {
    st.conns.get(id).is_some_and(|c| c.token == token)
}

/// Per-session facts established by the handshake.
struct Ctx {
    token: u64,
    link: Arc<SecureLink>,
    dialed: Option<String>,
    remote_key: [u8; 32],
    hs_hash: [u8; 32],
    peer_addr: String,
    unverified: Arc<AtomicBool>,
}

/// Drive one link until it closes, times out, misbehaves or we shut down.
/// `dialed` is the peer id we connected to (outgoing), `None` if accepted.
/// Returns whether the link was registered (for an outgoing link: the dialed
/// identity was proven, or the stranger answered with the dialed id).
pub(crate) fn run_session(
    shared: Arc<Shared>,
    frames: Arc<dyn FrameLink>,
    mut reader: Box<dyn FrameReader>,
    dialed: Option<String>,
    guard: SessionGuard,
) -> bool {
    let t = shared.timing.clone();
    let deadline = Instant::now() + t.conn_timeout;
    let est = noise::handshake(&shared.static_key, dialed.is_some(), &*frames, &mut *reader, deadline, &shared.stop);
    let Some(Established { transport, remote_static, hash }) = est else {
        frames.close();
        cleanup(&shared, 0, None, dialed.as_deref());
        return false;
    };
    let transport = Arc::new(Mutex::new(transport));
    let Some(link) = SecureLink::start(&shared, frames.clone(), transport.clone()) else {
        frames.close();
        cleanup(&shared, 0, None, dialed.as_deref());
        return false;
    };
    let ctx = Ctx {
        token: shared.token(),
        link: link.clone(),
        dialed: dialed.clone(),
        remote_key: remote_static,
        hs_hash: hash,
        peer_addr: frames.peer_addr(),
        unverified: guard.unverified.clone(),
    };
    link.send(&ctl(Control::Hello { id: shared.id.clone(), name: shared.name.clone(), v: PROTOCOL_VERSION }));

    let mut registered: Option<(String, String)> = None;
    let mut buf = vec![0u8; MAX_FRAME];
    let hello_deadline = Instant::now() + t.conn_timeout;
    let mut last_recv = Instant::now();
    let mut last_ping = Instant::now();
    let mut closing_since: Option<Instant> = None;
    while !shared.stopping() {
        match reader.recv() {
            Recv::Closed | Recv::Invalid => break,
            Recv::Idle => {}
            Recv::Frame(frame) => {
                last_recv = Instant::now();
                let n = transport.lock().unwrap_or_else(|e| e.into_inner()).read_message(&frame, &mut buf);
                let Ok(n) = n else { break }; // forged, replayed or reordered frame
                let Some(msg) = Message::parse(&buf[..n]) else { break };
                match &registered {
                    None => {
                        let Message::Control(Control::Hello { id, name, .. }) = msg else { break };
                        let name = clamp_name(&name);
                        if !register(&shared, &ctx, &id, &name) {
                            break;
                        }
                        registered = Some((id, name));
                    }
                    Some((id, name)) => {
                        if !handle(&shared, &ctx, id, name, msg) {
                            break;
                        }
                    }
                }
            }
        }
        let now = Instant::now();
        if now.duration_since(last_recv) > t.conn_timeout || (registered.is_none() && now > hello_deadline) {
            break;
        }
        if link.closing() {
            if now.duration_since(*closing_since.get_or_insert(now)) > t.conn_timeout {
                break;
            }
        } else if now.duration_since(last_ping) >= t.ping_interval {
            if !link.send(&ctl(Control::Ping)) {
                break;
            }
            last_ping = now;
        }
        if let Some((id, _)) = &registered {
            if !timers(&shared, &ctx, id) {
                break;
            }
        }
    }
    link.close();
    let reg_id = registered.map(|r| r.0);
    let ok = reg_id.is_some();
    cleanup(&shared, ctx.token, reg_id, dialed.as_deref());
    drop(guard);
    ok
}

/// Classify the link after the peer's hello and record it. `false` = reject.
fn register(shared: &Shared, ctx: &Ctx, id: &str, name: &str) -> bool {
    if id.is_empty() || id.len() > MAX_ID || id == shared.id {
        return false;
    }
    if ctx.dialed.as_deref().is_some_and(|d| d != id) {
        return false; // answered by someone other than who we dialed
    }
    let outgoing = ctx.dialed.is_some();
    let dialer = if outgoing { shared.id.clone() } else { id.to_string() };
    let now = Instant::now();
    let mut replaced = None;
    let mut first_msg = None;
    {
        let mut st = shared.state();
        let friend_link = match st.friend_key(id) {
            Some(k) if k == ctx.remote_key => true,
            Some(_) => return false, // claims a friend's id with another key
            None if st.friends.iter().any(|f| f.public_key == ctx.remote_key) => return false,
            None => false,
        };
        let mut announced = false;
        if let Some(old) = st.conns.get(id) {
            // Only an authenticated friend link may displace an existing one.
            // A newer link from the same dialer replaces a stale one; links
            // dialed by different sides resolve to the one dialed by the
            // smaller id, identically on both peers.
            let min = if shared.id.as_str() < id { shared.id.as_str() } else { id };
            if !friend_link || (old.dialer != dialer && dialer != min) {
                return false;
            }
            announced = old.announced;
            replaced = Some(old.link.clone());
        }
        let kind = if friend_link {
            shared.release_unverified(&ctx.unverified);
            Kind::Friend
        } else if outgoing {
            // We only dial strangers to deliver an invite.
            let Some(pet) = st.invites_out.remove(id) else { return false };
            let nonce_i = noise::random32();
            first_msg = Some(Control::Invite { pet, commit: noise::to_hex(&noise::commit(&nonce_i)) });
            Kind::Stranger(Some(Pairing {
                stage: PairStage::AwaitRandom { nonce_i },
                local_ok: false,
                remote_ok: false,
                since: now,
                failure_reported: false,
            }))
        } else {
            Kind::Stranger(None)
        };
        let connected = friend_link && !announced;
        st.conns.insert(
            id.to_string(),
            Conn {
                token: ctx.token,
                link: ctx.link.clone(),
                name: name.to_string(),
                kind,
                announced: announced || connected,
                outgoing,
                dialer,
                remote_key: ctx.remote_key,
                hs_hash: ctx.hs_hash,
                since: now,
                unverified: ctx.unverified.clone(),
            },
        );
        if outgoing {
            st.dialing.remove(id);
        }
        if connected {
            shared.emit(NetEvent::Connected { id: id.to_string(), name: name.to_string() });
        }
    }
    if let Some(old) = replaced {
        old.close();
    }
    match first_msg {
        Some(m) => ctx.link.send(&ctl(m)),
        None => true,
    }
}

/// Pin the pairing peer `id` as a friend (both users confirmed).
fn pin(shared: &Shared, st: &mut State, id: &str) {
    let Some(conn) = st.conns.get_mut(id) else { return };
    conn.kind = Kind::Friend;
    conn.announced = true;
    shared.release_unverified(&conn.unverified);
    let (key, name) = (conn.remote_key, conn.name.clone());
    st.add_friend(id, key);
    shared.emit(NetEvent::Connected { id: id.to_string(), name });
}

/// Record an incoming pairing attempt; `true` if it is within all limits.
fn admit_invite(st: &mut State, addr: &str, now: Instant) -> bool {
    st.invited_log.retain(|t| now.duration_since(*t) < INVITED_WINDOW);
    if st.pair_attempts.len() >= 256 && !st.pair_attempts.contains_key(addr) {
        st.pair_attempts.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) < INVITED_WINDOW));
    }
    let attempts = st.pair_attempts.entry(addr.to_string()).or_default();
    attempts.retain(|t| now.duration_since(*t) < INVITED_WINDOW);
    let addr_ok = attempts.len() < MAX_PAIR_ATTEMPTS_PER_ADDR;
    if attempts.len() < 2 * MAX_PAIR_ATTEMPTS_PER_ADDR {
        attempts.push_back(now);
    }
    let pairing = st
        .conns
        .values()
        .filter(|c| !c.outgoing && matches!(c.kind, Kind::Stranger(Some(_))))
        .count();
    addr_ok && pairing < MAX_INVITES_IN && st.invited_log.len() < MAX_INVITED_PER_WINDOW
}

/// Handle one message on a registered link. `false` = drop the link.
fn handle(shared: &Shared, ctx: &Ctx, id: &str, name: &str, msg: Message) -> bool {
    let now = Instant::now();
    let mut st = shared.state();
    if !current(&st, id, ctx.token) {
        return false; // replaced by a newer link
    }
    match msg {
        Message::Control(Control::Hello { .. } | Control::Ping) => true,
        Message::Control(Control::Invite { pet, commit }) => {
            let conn = &st.conns[id];
            match conn.kind {
                Kind::Friend => return true,
                Kind::Stranger(Some(_)) => return false, // one pairing per link
                Kind::Stranger(None) if conn.outgoing => return false, // invites only on links the inviter dialed
                Kind::Stranger(None) => {}
            }
            let Some(commit) = noise::hex32(&commit) else { return false };
            if !admit_invite(&mut st, &ctx.peer_addr, now) {
                drop(st);
                ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
                ctx.link.finish();
                return true;
            }
            let r_r = noise::random32();
            let conn = st.conns.get_mut(id).expect("current conn");
            conn.kind = Kind::Stranger(Some(Pairing {
                stage: PairStage::AwaitReveal { commit, r_r, pet: clamp_name(&pet) },
                local_ok: false,
                remote_ok: false,
                since: now,
                failure_reported: false,
            }));
            drop(st);
            ctx.link.send(&ctl(Control::PairRandom { random: noise::to_hex(&r_r) }))
        }
        Message::Control(Control::PairRandom { random }) => {
            let conn = st.conns.get_mut(id).expect("current conn");
            let hs_hash = conn.hs_hash;
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return false };
            let PairStage::AwaitRandom { nonce_i } = p.stage else { return false };
            let Some(r_r) = noise::hex32(&random) else { return false };
            p.stage = PairStage::Confirm;
            shared.emit(NetEvent::InviteSent { to: id.to_string(), sas: noise::sas(&hs_hash, &nonce_i, &r_r) });
            drop(st);
            ctx.link.send(&ctl(Control::PairReveal { nonce: noise::to_hex(&nonce_i) }))
        }
        Message::Control(Control::PairReveal { nonce }) => {
            let conn = st.conns.get_mut(id).expect("current conn");
            let hs_hash = conn.hs_hash;
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return false };
            let PairStage::AwaitReveal { commit, r_r, pet } = &p.stage else { return false };
            let Some(nonce_i) = noise::hex32(&nonce) else { return false };
            if noise::commit(&nonce_i) != *commit {
                return false; // reveal does not match the commitment: abort pairing
            }
            let sas = noise::sas(&hs_hash, &nonce_i, r_r);
            let pet = pet.clone();
            p.stage = PairStage::Confirm;
            st.invited_log.push_back(now);
            shared.emit(NetEvent::Invited { from: id.to_string(), name: name.to_string(), pet, sas });
            true
        }
        Message::Control(Control::InviteReply { accepted }) => {
            let conn = st.conns.get_mut(id).expect("current conn");
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return true };
            if !accepted {
                if !p.failure_reported {
                    p.failure_reported = true;
                    shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: false });
                }
                return false;
            }
            if !matches!(p.stage, PairStage::Confirm) || p.remote_ok {
                return false; // confirmation before the SAS exists
            }
            p.remote_ok = true;
            let both = p.local_ok;
            shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: true });
            if both {
                pin(shared, &mut st, id);
            }
            true
        }
        Message::Control(Control::Unfriend) => {
            let conn = st.conns.get_mut(id).expect("current conn");
            if !matches!(conn.kind, Kind::Friend) {
                return true;
            }
            conn.kind = Kind::Stranger(None);
            st.remove_friend(id);
            false
        }
        Message::Visit(v) => {
            if matches!(st.conns[id].kind, Kind::Friend) && st.is_friend(id) {
                shared.emit(NetEvent::Received { from: id.to_string(), msg: v });
            }
            true
        }
    }
}

/// Pairing deadline and idle-stranger check. `false` = drop the link.
fn timers(shared: &Shared, ctx: &Ctx, id: &str) -> bool {
    let t = &shared.timing;
    let now = Instant::now();
    let mut st = shared.state();
    if !current(&st, id, ctx.token) {
        return false;
    }
    let conn = st.conns.get_mut(id).expect("current conn");
    if conn.link.closing() {
        return true; // graceful close in progress; the session bounds it
    }
    let since = conn.since;
    match &mut conn.kind {
        Kind::Stranger(Some(p)) if now.duration_since(p.since) > t.invite_timeout => {
            if !p.failure_reported {
                p.failure_reported = true;
                shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: false });
            }
            drop(st);
            ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
            ctx.link.finish();
            true
        }
        // A stranger link with no invite has no business staying open.
        Kind::Stranger(None) => now.duration_since(since) <= t.conn_timeout,
        _ => true,
    }
}

/// Session teardown. Dial bookkeeping always uses the dialed id, never the
/// id the peer claimed.
fn cleanup(shared: &Shared, token: u64, registered: Option<String>, dialed: Option<&str>) {
    let mut st = shared.state();
    if let Some(d) = dialed {
        st.dialing.remove(d);
        if registered.as_deref() != Some(d) && st.invites_out.remove(d).is_some() {
            shared.emit(NetEvent::InviteAnswered { from: d.to_string(), accepted: false });
        }
    }
    let Some(id) = registered else { return };
    if !current(&st, &id, token) {
        return; // a newer link owns this peer now
    }
    let conn = st.conns.remove(&id).expect("current conn");
    if let Kind::Stranger(Some(p)) = &conn.kind {
        if !p.failure_reported {
            shared.emit(NetEvent::InviteAnswered { from: id.clone(), accepted: false });
        }
    }
    if conn.announced {
        shared.emit(NetEvent::Disconnected { id });
    }
}

/// A dial to `id` could not even start or connect.
pub(crate) fn dial_failed(shared: &Shared, id: &str) {
    let mut st = shared.state();
    st.dialing.remove(id);
    if st.invites_out.remove(id).is_some() {
        shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: false });
    }
}

pub(crate) fn invite(shared: &Arc<Shared>, peer_id: &str, pet: &str) {
    let mut st = shared.state();
    if peer_id == shared.id || st.invites_out.contains_key(peer_id) {
        return;
    }
    if st.is_friend(peer_id) {
        shared.emit(NetEvent::InviteAnswered { from: peer_id.to_string(), accepted: true });
        return;
    }
    // Always a fresh outgoing link: never reuse an inbound/unauthenticated one.
    let busy = st.conns.contains_key(peer_id) || st.dialing.contains(peer_id);
    let target = if busy { None } else { pick_candidate(&st, peer_id) };
    let Some(target) = target else {
        shared.emit(NetEvent::InviteAnswered { from: peer_id.to_string(), accepted: false });
        return;
    };
    st.invites_out.insert(peer_id.to_string(), clamp_name(pet));
    st.dialing.insert(peer_id.to_string());
    drop(st);
    transport::dial(shared, peer_id.to_string(), target);
}

pub(crate) fn answer_invite(shared: &Shared, peer_id: &str, accept: bool) {
    let mut st = shared.state();
    let Some(conn) = st.conns.get_mut(peer_id) else { return };
    let link = conn.link.clone();
    let Kind::Stranger(Some(p)) = &mut conn.kind else { return };
    if !matches!(p.stage, PairStage::Confirm) || p.local_ok {
        return;
    }
    if !accept {
        p.failure_reported = true;
        drop(st);
        link.send(&ctl(Control::InviteReply { accepted: false }));
        link.finish();
        return;
    }
    p.local_ok = true;
    if p.remote_ok {
        pin(shared, &mut st, peer_id);
    }
    drop(st);
    link.send(&ctl(Control::InviteReply { accepted: true }));
}
