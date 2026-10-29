//! Per-link session driver, transport-independent (frames in, frames out).
//!
//! 1. Noise XX handshake (dialer = initiator) → peer's static public key.
//! 2. Encrypted `hello{id, name, friend}`. The link is a *friend* link iff a
//!    friend with that id is pinned to exactly that key and the peer also has
//!    our key pinned (`friend: true`). An id with a different key, or a
//!    pinned key under a different id, is rejected. On an outgoing link the
//!    id must equal the id we dialed. Handshake + hello share one deadline.
//! 3. Otherwise it is a *stranger* link (several may claim the same id; they
//!    never block each other or a friend), which may only pair, and only on
//!    the link the inviter dialed: `invite{commit}` → `pair_random` →
//!    `pair_reveal` → both sides show the same 6-emoji SAS → both users
//!    confirm with `invite_reply{accepted}` before the deadline → both pin
//!    each other's key. A refused or timed-out pairing is terminal.

use crate::discovery::{note_dial, pick_candidate};
use crate::noise::{self, Established};
use crate::protocol::{clamp_name, Control, Message, MAX_FRAME, MAX_ID, PROTOCOL_VERSION};
use crate::secure::SecureLink;
use crate::transport::{self, FrameLink, FrameReader, Recv};
use crate::{
    Backoff, Conn, Kind, NetEvent, PairStage, Pairing, SessionGuard, Shared, State, INVITED_WINDOW,
    MAX_INVITED_PER_WINDOW, MAX_INVITES_IN, MAX_INVITES_IN_PER_ADDR, MAX_PAIR_ATTEMPTS_PER_ADDR, PRE_AUTH_MAX,
};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// How a dial ended, for candidate bookkeeping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialOutcome {
    /// The dialed identity answered (link kept, or a duplicate of one).
    Proven,
    /// The dialed friend id answered with a different static key.
    WrongKey,
    /// Anything else (no connection, handshake failure, rejected).
    Failed,
}

/// Result of classifying a link after hello.
enum Reg {
    Ok,
    /// A friend link that loses the duplicate tie-break.
    Duplicate,
    WrongKey,
    /// Our pinned friend (right key) does not have us pinned.
    NotMutual,
    Reject,
}

fn ctl(c: Control) -> Message {
    Message::Control(c)
}

fn pairing_active(c: &Conn) -> bool {
    matches!(&c.kind, Kind::Stranger(Some(p)) if !matches!(p.stage, PairStage::Failed))
}

/// Is one of *our own* outgoing pairings to `id` in progress? (Inbound links
/// merely claiming `id` never block an invite we start.)
fn outbound_pairing(st: &State, id: &str) -> bool {
    st.conns.values().any(|c| c.outgoing && c.peer_id == id && pairing_active(c))
}

/// Active inbound pairings from one source address (limited per address,
/// not per claimed id, so a squatter cannot block someone else's id).
fn inbound_pairings_from(st: &State, addr: &str) -> usize {
    st.conns.values().filter(|c| !c.outgoing && c.peer_addr == addr && pairing_active(c)).count()
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
pub(crate) fn run_session(
    shared: Arc<Shared>,
    frames: Arc<dyn FrameLink>,
    mut reader: Box<dyn FrameReader>,
    dialed: Option<String>,
    guard: SessionGuard,
) -> DialOutcome {
    let t = shared.timing.clone();
    // One deadline for the whole pre-auth phase (handshake + hello).
    let pre_auth = Instant::now() + t.conn_timeout.min(PRE_AUTH_MAX);
    let est = noise::handshake(&shared.static_key, dialed.is_some(), &*frames, &mut *reader, pre_auth, &shared.stop);
    let Some(Established { transport, remote_static, hash }) = est else {
        frames.close();
        cleanup(&shared, None, dialed.as_deref());
        return DialOutcome::Failed;
    };
    let transport = Arc::new(Mutex::new(transport));
    let Some(link) = SecureLink::start(&shared, frames.clone(), transport.clone()) else {
        frames.close();
        cleanup(&shared, None, dialed.as_deref());
        return DialOutcome::Failed;
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
    let we_pinned_them = shared.state().friends.iter().any(|f| f.public_key == remote_static);
    link.send(&ctl(Control::Hello {
        id: shared.id.clone(),
        name: shared.name.clone(),
        v: PROTOCOL_VERSION,
        friend: we_pinned_them,
    }));

    let mut outcome = DialOutcome::Failed;
    let mut registered: Option<(String, String)> = None;
    let mut buf = vec![0u8; MAX_FRAME];
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
                        let Message::Control(Control::Hello { id, name, friend, .. }) = msg else { break };
                        let name = clamp_name(&name);
                        match register(&shared, &ctx, &id, &name, friend) {
                            Reg::Ok => {
                                outcome = DialOutcome::Proven;
                                registered = Some((id, name));
                            }
                            Reg::Duplicate | Reg::NotMutual => {
                                outcome = DialOutcome::Proven;
                                break;
                            }
                            Reg::WrongKey => {
                                outcome = DialOutcome::WrongKey;
                                break;
                            }
                            Reg::Reject => break,
                        }
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
        if now.duration_since(last_recv) > t.conn_timeout || (registered.is_none() && now > pre_auth) {
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
        if registered.is_some() && !timers(&shared, &ctx) {
            break;
        }
    }
    link.close();
    cleanup(&shared, Some(ctx.token), dialed.as_deref());
    drop(guard);
    outcome
}

/// Classify the link after the peer's hello and record it.
fn register(shared: &Shared, ctx: &Ctx, id: &str, name: &str, peer_pinned_us: bool) -> Reg {
    if id.is_empty() || id.len() > MAX_ID || id == shared.id {
        return Reg::Reject;
    }
    if ctx.dialed.as_deref().is_some_and(|d| d != id) {
        return Reg::Reject; // answered by someone other than who we dialed
    }
    let outgoing = ctx.dialed.is_some();
    let dialer = if outgoing { shared.id.clone() } else { id.to_string() };
    let now = Instant::now();
    let mut replaced = None;
    let mut first_msg = None;
    {
        let mut st = shared.state();
        let pinned = st.friend_key(id);
        match pinned {
            Some(k) if k != ctx.remote_key => {
                // Unproven: whoever answered is not our friend. Silent; the
                // dialed candidate is backed off by the caller.
                return Reg::WrongKey;
            }
            None if st.friends.iter().any(|f| f.public_key == ctx.remote_key) => return Reg::Reject,
            _ => {}
        }
        let friend_link = pinned.is_some();
        if friend_link && !peer_pinned_us {
            // Authenticated friend that does not have us: back off redials.
            st.back_off(Backoff::Id(id.to_string()), shared.timing.peer_timeout, now);
            shared.emit(NetEvent::FriendForgotUs { id: id.to_string() });
            return Reg::NotMutual;
        }
        if !friend_link && peer_pinned_us {
            return Reg::Reject; // they think we are friends; we are not: free the slot now
        }
        // A link that ended moments ago still owes its Disconnected: this one
        // takes over its Connected state instead.
        let mut announced = friend_link && !st.friend_links.contains_key(id) && st.pending_gone.remove(id).is_some();
        let kind = if friend_link {
            if let Some(&old_t) = st.friend_links.get(id) {
                // One friend link per id. A newer link from the same dialer
                // replaces a stale one; links dialed by different sides
                // resolve to the one dialed by the smaller id on both peers.
                let min = if shared.id.as_str() < id { shared.id.as_str() } else { id };
                let old = st.conns.get_mut(&old_t).expect("friend link indexed");
                if old.dialer != dialer && dialer != min {
                    return Reg::Duplicate;
                }
                announced = old.announced;
                old.announced = false; // the new link inherits the Connected state
                replaced = Some(old.link.clone());
            }
            shared.release_unverified(&ctx.unverified);
            st.trust_ip(id, &ctx.peer_addr, now);
            st.friend_wait.remove(id);
            st.backoff.remove(&Backoff::Id(id.to_string()));
            st.friend_links.insert(id.to_string(), ctx.token);
            Kind::Friend
        } else if outgoing {
            // We only dial strangers to deliver an invite.
            let Some(pet) = st.invites_out.remove(id) else { return Reg::Reject };
            let nonce_i = noise::random32();
            first_msg = Some(Control::Invite { pet, commit: noise::to_hex(&noise::commit(&nonce_i)) });
            Kind::Stranger(Some(Pairing {
                stage: PairStage::AwaitRandom { nonce_i },
                local_ok: false,
                remote_ok: false,
                since: now,
                failure_reported: false,
                surfaced: false,
            }))
        } else {
            Kind::Stranger(None)
        };
        let connected = friend_link && !announced;
        st.conns.insert(
            ctx.token,
            Conn {
                peer_id: id.to_string(),
                link: ctx.link.clone(),
                name: name.to_string(),
                kind,
                announced: announced || connected,
                outgoing,
                dialer,
                remote_key: ctx.remote_key,
                peer_addr: ctx.peer_addr.clone(),
                hs_hash: ctx.hs_hash,
                since: now,
                unverified: ctx.unverified.clone(),
            },
        );
        if connected {
            shared.emit(NetEvent::Connected { id: id.to_string(), name: name.to_string() });
        }
    }
    if let Some(old) = replaced {
        old.close();
    }
    if let Some(m) = first_msg {
        if !ctx.link.send(&ctl(m)) {
            return Reg::Reject;
        }
    }
    Reg::Ok
}

/// Pin the peer of pairing link `token` (both users confirmed in time).
/// Returns other stranger links claiming the same id, to be closed.
fn pin(shared: &Shared, st: &mut State, token: u64) -> Vec<Arc<SecureLink>> {
    let Some(conn) = st.conns.get_mut(&token) else { return Vec::new() };
    conn.kind = Kind::Friend;
    conn.announced = true;
    shared.release_unverified(&conn.unverified);
    let (id, key, name, addr) = (conn.peer_id.clone(), conn.remote_key, conn.name.clone(), conn.peer_addr.clone());
    st.add_friend(&id, key);
    st.trust_ip(&id, &addr, Instant::now());
    st.friend_links.insert(id.clone(), token);
    if st.pending_gone.remove(&id).is_none() {
        shared.emit(NetEvent::Connected { id: id.clone(), name });
    }
    st.conns.iter().filter(|(t, c)| **t != token && c.peer_id == id).map(|(_, c)| c.link.clone()).collect()
}

/// Mark a pairing failed (terminal); report it once unless already reported.
fn fail(shared: &Shared, p: &mut Pairing, id: &str, token: u64) {
    p.stage = PairStage::Failed;
    if !p.failure_reported {
        p.failure_reported = true;
        if p.surfaced {
            shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: false, pairing: token });
        }
    }
}

fn expired(shared: &Shared, p: &Pairing, now: Instant) -> bool {
    now.duration_since(p.since) > shared.timing.invite_timeout
}

/// Record an incoming pairing attempt; `true` if it is within all limits.
fn admit_invite(st: &mut State, addr: &str, now: Instant) -> bool {
    st.invited_log.retain(|t| now.duration_since(*t) < INVITED_WINDOW);
    if st.pair_attempts.len() >= 256 && !st.pair_attempts.contains_key(addr) {
        st.pair_attempts.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) < INVITED_WINDOW));
        if st.pair_attempts.len() >= 256 {
            let oldest = st.pair_attempts.iter().min_by_key(|(_, q)| q.back().copied()).map(|(a, _)| a.clone());
            if let Some(o) = oldest {
                st.pair_attempts.remove(&o);
            }
        }
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
        .filter(|c| !c.outgoing && matches!(&c.kind, Kind::Stranger(Some(p)) if !matches!(p.stage, PairStage::Failed)))
        .count();
    addr_ok && pairing < MAX_INVITES_IN && st.invited_log.len() < MAX_INVITED_PER_WINDOW
}

/// Handle one message on a registered link. `false` = drop the link.
fn handle(shared: &Shared, ctx: &Ctx, id: &str, name: &str, msg: Message) -> bool {
    let now = Instant::now();
    let mut st = shared.state();
    let Some(conn) = st.conns.get(&ctx.token) else { return false };
    let (outgoing, hs_hash) = (conn.outgoing, conn.hs_hash);
    match msg {
        Message::Control(Control::Hello { .. } | Control::Ping) => true,
        Message::Control(Control::Invite { pet, commit }) => {
            match conn.kind {
                Kind::Friend => return true,
                Kind::Stranger(Some(_)) => return false, // one pairing per link
                Kind::Stranger(None) if outgoing => return false, // invites only on links the inviter dialed
                Kind::Stranger(None) => {}
            }
            let Some(commit) = noise::hex32(&commit) else { return false };
            if inbound_pairings_from(&st, &ctx.peer_addr) >= MAX_INVITES_IN_PER_ADDR
                || !admit_invite(&mut st, &ctx.peer_addr, now)
            {
                drop(st);
                ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
                ctx.link.finish();
                return true;
            }
            let r_r = noise::random32();
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            conn.kind = Kind::Stranger(Some(Pairing {
                stage: PairStage::AwaitReveal { commit, r_r, pet: clamp_name(&pet) },
                local_ok: false,
                remote_ok: false,
                since: now,
                failure_reported: false,
                surfaced: false,
            }));
            drop(st);
            ctx.link.send(&ctl(Control::PairRandom { random: noise::to_hex(&r_r) }))
        }
        Message::Control(Control::PairRandom { random }) => {
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return false };
            let PairStage::AwaitRandom { nonce_i } = p.stage else { return false };
            let Some(r_r) = noise::hex32(&random) else { return false };
            p.stage = PairStage::Confirm;
            p.surfaced = true;
            shared.emit(NetEvent::InviteSent {
                to: id.to_string(),
                sas: noise::sas(&hs_hash, &nonce_i, &r_r),
                pairing: ctx.token,
            });
            drop(st);
            ctx.link.send(&ctl(Control::PairReveal { nonce: noise::to_hex(&nonce_i) }))
        }
        Message::Control(Control::PairReveal { nonce }) => {
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return false };
            let PairStage::AwaitReveal { commit, r_r, pet } = &p.stage else { return false };
            let Some(nonce_i) = noise::hex32(&nonce) else { return false };
            if noise::commit(&nonce_i) != *commit {
                return false; // reveal does not match the commitment: abort pairing
            }
            let sas = noise::sas(&hs_hash, &nonce_i, r_r);
            let pet = pet.clone();
            p.stage = PairStage::Confirm;
            p.surfaced = true;
            st.invited_log.push_back(now);
            shared.emit(NetEvent::Invited { from: id.to_string(), name: name.to_string(), pet, sas, pairing: ctx.token });
            true
        }
        Message::Control(Control::InviteReply { accepted }) => {
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return true };
            if matches!(p.stage, PairStage::Failed) {
                return true; // terminal: late replies change nothing
            }
            if !accepted {
                fail(shared, p, id, ctx.token);
                return false;
            }
            if !matches!(p.stage, PairStage::Confirm) || p.remote_ok {
                return false; // confirmation before the SAS exists
            }
            if expired(shared, p, now) {
                fail(shared, p, id, ctx.token);
                drop(st);
                ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
                ctx.link.finish();
                return true;
            }
            p.remote_ok = true;
            let both = p.local_ok;
            shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: true, pairing: ctx.token });
            let to_close = if both { pin(shared, &mut st, ctx.token) } else { Vec::new() };
            drop(st);
            to_close.iter().for_each(|l| l.close());
            true
        }
        Message::Control(Control::Unfriend) => {
            if !matches!(conn.kind, Kind::Friend) {
                return true;
            }
            st.conns.get_mut(&ctx.token).expect("current conn").kind = Kind::Stranger(None);
            st.remove_friend(id);
            // Also drop any other friend link for this id (e.g. one that
            // registered while this message was in flight).
            let other = st.friend_links.remove(id).filter(|t| *t != ctx.token);
            let other = other.and_then(|t| st.conns.get_mut(&t)).map(|c| {
                c.kind = Kind::Stranger(None);
                c.link.clone()
            });
            drop(st);
            if let Some(l) = other {
                l.close();
            }
            false
        }
        Message::Visit(v) => {
            if matches!(conn.kind, Kind::Friend) && st.is_friend(id) {
                shared.emit(NetEvent::Received { from: id.to_string(), msg: v });
            }
            true
        }
    }
}

/// Pairing deadline and idle-stranger check. `false` = drop the link.
fn timers(shared: &Shared, ctx: &Ctx) -> bool {
    let now = Instant::now();
    let mut st = shared.state();
    let Some(conn) = st.conns.get_mut(&ctx.token) else { return false };
    if conn.link.closing() {
        return true; // graceful close in progress; the session bounds it
    }
    let (since, id) = (conn.since, conn.peer_id.clone());
    match &mut conn.kind {
        Kind::Stranger(Some(p)) if !matches!(p.stage, PairStage::Failed) && expired(shared, p, now) => {
            fail(shared, p, &id, ctx.token);
            drop(st);
            ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
            ctx.link.finish();
            true
        }
        // A stranger link with no invite has no business staying open.
        Kind::Stranger(None) => now.duration_since(since) <= shared.timing.conn_timeout,
        _ => true,
    }
}

/// Session teardown. Dial bookkeeping always uses the dialed id, never the
/// id the peer claimed.
fn cleanup(shared: &Shared, token: Option<u64>, dialed: Option<&str>) {
    let mut st = shared.state();
    if let Some(d) = dialed {
        st.dialing.remove(d);
        // An invite that never reached a surfaced pairing has no outcome.
        st.invites_out.remove(d);
    }
    let Some(conn) = token.and_then(|t| st.conns.remove(&t)) else { return };
    if st.friend_links.get(&conn.peer_id) == token.as_ref() {
        st.friend_links.remove(&conn.peer_id);
    }
    if let Kind::Stranger(Some(p)) = &conn.kind {
        if !p.failure_reported && p.surfaced {
            if let Some(t) = token {
                shared.emit(NetEvent::InviteAnswered { from: conn.peer_id.clone(), accepted: false, pairing: t });
            }
        }
    }
    if conn.announced {
        if st.is_friend(&conn.peer_id) && !st.friend_links.contains_key(&conn.peer_id) {
            // Hold the Disconnected for a grace; see `State::pending_gone`.
            let until = Instant::now() + shared.timing.conn_timeout;
            st.pending_gone.insert(conn.peer_id, until);
        } else {
            st.pending_gone.remove(&conn.peer_id);
            shared.emit(NetEvent::Disconnected { id: conn.peer_id });
        }
    }
}

/// Emit the `Disconnected` events whose grace ran out (called every tick).
pub(crate) fn flush_gone(shared: &Shared) {
    let now = Instant::now();
    let mut st = shared.state();
    if st.pending_gone.is_empty() {
        return;
    }
    let due: Vec<String> = st.pending_gone.iter().filter(|(_, t)| now >= **t).map(|(id, _)| id.clone()).collect();
    for id in due {
        st.pending_gone.remove(&id);
        if !st.friend_links.contains_key(&id) {
            shared.emit(NetEvent::Disconnected { id });
        }
    }
}

/// A dial to `id` could not even start or connect.
pub(crate) fn dial_failed(shared: &Shared, id: &str) {
    let mut st = shared.state();
    st.dialing.remove(id);
    st.invites_out.remove(id);
}

pub(crate) fn invite(shared: &Arc<Shared>, peer_id: &str, pet: &str) {
    let mut st = shared.state();
    if peer_id == shared.id || st.invites_out.contains_key(peer_id) {
        return;
    }
    if st.is_friend(peer_id) {
        return;
    }
    // Always a fresh outgoing link; inbound links claiming this id (possibly
    // squatters, even mid-pairing) neither block nor get reused.
    let busy = st.dialing.contains(peer_id) || outbound_pairing(&st, peer_id);
    let target = if busy { None } else { pick_candidate(&st, peer_id) };
    let Some(target) = target else {
        return;
    };
    note_dial(&mut st, peer_id, target, Instant::now(), false);
    drop(st);
    invite_at(shared, peer_id, pet, target);
}

/// Start a pairing with `peer_id` on a fresh outgoing link to `target`
/// (a LAN candidate, or the relay after a friend code was redeemed).
/// `false` if not possible now (us, already a friend, or one in progress).
pub(crate) fn invite_at(shared: &Arc<Shared>, peer_id: &str, pet: &str, target: transport::Candidate) -> bool {
    let mut st = shared.state();
    if peer_id == shared.id
        || st.invites_out.contains_key(peer_id)
        || st.is_friend(peer_id)
        || st.dialing.contains(peer_id)
        || outbound_pairing(&st, peer_id)
    {
        return false;
    }
    st.invites_out.insert(peer_id.to_string(), clamp_name(pet));
    st.dialing.insert(peer_id.to_string());
    drop(st);
    transport::dial(shared, peer_id.to_string(), target);
    true
}

/// Answer exactly the pairing the user saw (`pairing` from its event).
pub(crate) fn answer_invite(shared: &Shared, pairing: u64, accept: bool) {
    let now = Instant::now();
    let mut st = shared.state();
    let token = pairing;
    let Some(conn) = st.conns.get_mut(&token) else { return };
    let link = conn.link.clone();
    let peer_id = conn.peer_id.clone();
    let peer_id = peer_id.as_str();
    let Kind::Stranger(Some(p)) = &mut conn.kind else { return };
    if !matches!(p.stage, PairStage::Confirm) || p.local_ok {
        return;
    }
    if !accept || expired(shared, p, now) {
        if !accept {
            p.failure_reported = true; // our own refusal needs no event
        }
        fail(shared, p, peer_id, token);
        drop(st);
        link.send(&ctl(Control::InviteReply { accepted: false }));
        link.finish();
        return;
    }
    p.local_ok = true;
    let to_close = if p.remote_ok { pin(shared, &mut st, token) } else { Vec::new() };
    drop(st);
    to_close.iter().for_each(|l| l.close());
    link.send(&ctl(Control::InviteReply { accepted: true }));
}
