//! Per-peer session driver: `hello` handshake with challenge-response
//! authentication for friends, invites/pairing, visit delivery, ping and
//! timeouts. Transport-independent: it only sees a `Link`/`LinkReader` pair
//! (see `transport`).
//!
//! Handshake: both sides send `hello{id, name, v, nonce}`. If the peer's id
//! is a friend, each side answers the other's nonce with `auth{mac}` and the
//! link is registered (and `Connected` emitted) only after the peer's mac
//! verifies. A non-friend ("stranger") link is registered right after hello,
//! but may only carry `invite`/`invite_reply`/`ping` until pairing gives it a
//! secret. On an outgoing link the hello id must equal the id we dialed.

use crate::auth;
use crate::discovery::pick_candidate;
use crate::protocol::{Control, Message, PROTOCOL_VERSION};
use crate::transport::{Link, LinkReader, Recv};
use crate::{tcp, Conn, NetEvent, OutInvite, Shared, State};
use std::sync::Arc;
use std::time::Instant;

/// Longest accepted peer id.
const MAX_ID: usize = 128;

fn ctl(c: Control) -> Message {
    Message::Control(c)
}

/// Is `token` the session currently registered for `id`?
fn current(st: &State, id: &str, token: u64) -> bool {
    st.conns.get(id).is_some_and(|c| c.token == token)
}

enum Phase {
    /// Waiting for the peer's hello.
    Hello,
    /// Peer claims friend `id`; waiting for its `auth` response.
    Auth { id: String, name: String, since: Instant },
    /// Registered in `State::conns` (authenticated friend or stranger).
    Ready { id: String, name: String },
}

/// Drive one link until it closes, times out, misbehaves or we shut down.
/// `dialed` is the peer id we connected to (outgoing), `None` if accepted.
/// Returns whether the link was registered for the peer (for an outgoing
/// link: the dialed id was proven).
pub(crate) fn run_session(
    shared: Arc<Shared>,
    link: Arc<dyn Link>,
    mut reader: Box<dyn LinkReader>,
    dialed: Option<String>,
) -> bool {
    let token = shared.token();
    let t = shared.timing.clone();
    let my_nonce = auth::random_hex();
    let mut phase = Phase::Hello;
    let mut registered = false;
    let mut last_recv = Instant::now();
    let mut last_ping = Instant::now();
    let mut ok = link.send(&ctl(Control::Hello {
        id: shared.id.clone(),
        name: shared.name.clone(),
        v: PROTOCOL_VERSION,
        nonce: my_nonce.clone(),
    }));
    while ok && !shared.stopping() {
        match reader.recv() {
            Recv::Closed | Recv::Invalid => break,
            Recv::Idle => {}
            Recv::Msg(msg) => {
                last_recv = Instant::now();
                let (next, keep) = step(&shared, token, &link, dialed.as_deref(), &my_nonce, phase, msg);
                phase = next;
                if !keep {
                    break;
                }
                if matches!(phase, Phase::Ready { .. }) {
                    registered = true;
                }
            }
        }
        let now = Instant::now();
        if now.duration_since(last_recv) > t.conn_timeout {
            break;
        }
        if let Phase::Auth { since, .. } = &phase {
            if now.duration_since(*since) > t.conn_timeout {
                break; // claimed a friend id but never proved it
            }
        }
        if now.duration_since(last_ping) >= t.ping_interval {
            ok = link.send(&ctl(Control::Ping));
            last_ping = now;
        }
        if let Phase::Ready { id, .. } = &phase {
            ok = ok && timers(&shared, token, &link, id);
        }
    }
    link.close();
    let reg_id = match phase {
        Phase::Ready { id, .. } => Some(id),
        _ => None,
    };
    cleanup(&shared, token, reg_id, dialed.as_deref());
    registered
}

/// Advance the handshake / handle one message. Returns the new phase and
/// whether to keep the link.
fn step(
    shared: &Shared,
    token: u64,
    link: &Arc<dyn Link>,
    dialed: Option<&str>,
    my_nonce: &str,
    phase: Phase,
    msg: Message,
) -> (Phase, bool) {
    match phase {
        Phase::Hello => {
            let Message::Control(Control::Hello { id, name, nonce, .. }) = msg else {
                return (Phase::Hello, false); // anything before hello is a protocol error
            };
            if id.is_empty() || id.len() > MAX_ID || id == shared.id || !auth::valid_random(&nonce) {
                return (Phase::Hello, false);
            }
            if dialed.is_some_and(|d| d != id) {
                return (Phase::Hello, false); // answered by someone other than who we dialed
            }
            let secret = shared.state().secret(&id);
            match secret {
                Some(secret) => {
                    let mac = auth::respond(&secret, &nonce, &shared.id, &id).expect("valid secret and nonce");
                    let keep = link.send(&ctl(Control::Auth { mac }));
                    (Phase::Auth { id, name, since: Instant::now() }, keep)
                }
                None => {
                    let keep = register(shared, token, link, dialed.is_some(), &id, &name, false);
                    (Phase::Ready { id, name }, keep)
                }
            }
        }
        Phase::Auth { id, name, since } => match msg {
            Message::Control(Control::Auth { mac }) => {
                let secret = shared.state().secret(&id);
                let verified = secret.is_some_and(|s| auth::verify(&s, my_nonce, &id, &shared.id, &mac));
                if !verified {
                    return (Phase::Auth { id, name, since }, false);
                }
                let keep = register(shared, token, link, dialed.is_some(), &id, &name, true);
                (Phase::Ready { id, name }, keep)
            }
            Message::Control(Control::Ping) => (Phase::Auth { id, name, since }, true),
            _ => (Phase::Auth { id, name, since }, false),
        },
        Phase::Ready { id, name } => {
            let keep = handle(shared, token, &id, &name, msg);
            (Phase::Ready { id, name }, keep)
        }
    }
}

/// Record the session for `id`. `authed` = the peer proved the friendship
/// secret. `false` = reject the link.
fn register(shared: &Shared, token: u64, link: &Arc<dyn Link>, outgoing: bool, id: &str, name: &str, authed: bool) -> bool {
    let dialer = if outgoing { shared.id.clone() } else { id.to_string() };
    let mut send_invite = None;
    {
        let mut st = shared.state();
        let mut announced = false;
        if let Some(old) = st.conns.get(id) {
            // An unauthenticated link never displaces an existing one (so a
            // stranger can't hijack a pending invite by claiming its id).
            // Between authenticated links: a newer link from the same dialer
            // replaces a stale one; links dialed by different sides resolve
            // to the one dialed by the smaller id, identically on both peers.
            let min = if shared.id.as_str() < id { shared.id.as_str() } else { id };
            if !authed || (old.dialer != dialer && dialer != min) {
                return false;
            }
            announced = old.announced;
            old.link.close();
        }
        let connected = authed && !announced;
        if let Some(inv) = st.invites_out.get_mut(id) {
            if !inv.sent && !authed {
                inv.sent = true;
                inv.since = Instant::now();
                send_invite = Some(inv.pet.clone());
            }
        }
        st.conns.insert(
            id.to_string(),
            Conn { token, link: link.clone(), name: name.to_string(), announced: authed, dialer, since: Instant::now() },
        );
        if connected {
            shared.emit(NetEvent::Connected { id: id.to_string(), name: name.to_string() });
        }
    }
    match send_invite {
        Some(pet) => link.send(&ctl(Control::Invite { pet })),
        None => true,
    }
}

/// Handle one message from a registered peer. `false` = drop the link.
fn handle(shared: &Shared, token: u64, id: &str, name: &str, msg: Message) -> bool {
    let mut st = shared.state();
    if !current(&st, id, token) {
        return false; // replaced by a newer link
    }
    // `announced` = authenticated friend link (via auth or pairing on it).
    let authed = st.conns.get(id).is_some_and(|c| c.announced);
    match msg {
        Message::Control(Control::Hello { .. } | Control::Ping | Control::Auth { .. }) => true,
        Message::Control(Control::Invite { pet }) => {
            if !authed && !st.is_friend(id) && !st.invites_in.contains_key(id) {
                st.invites_in.insert(id.to_string(), Instant::now());
                shared.emit(NetEvent::Invited { from: id.to_string(), name: name.to_string(), pet });
            }
            true
        }
        Message::Control(Control::InviteReply { accepted, secret }) => {
            if authed || st.invites_out.remove(id).is_none() {
                return true; // unsolicited: ignore
            }
            let secret = secret.filter(|s| accepted && auth::valid_random(s));
            let Some(secret) = secret else {
                shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: false });
                return false; // declined (or accepted without a usable secret)
            };
            shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: true });
            st.add_friend(id, &secret);
            if let Some(c) = st.conns.get_mut(id) {
                c.announced = true;
            }
            shared.emit(NetEvent::Connected { id: id.to_string(), name: name.to_string() });
            true
        }
        Message::Control(Control::Unfriend) => {
            if !authed {
                return true;
            }
            st.remove_friend(id);
            st.invites_in.remove(id);
            st.invites_out.remove(id);
            false
        }
        Message::Visit(v) => {
            // Only authenticated friends may deliver visits; others are ignored.
            if authed && st.is_friend(id) {
                shared.emit(NetEvent::Received { from: id.to_string(), msg: v });
            }
            true
        }
    }
}

/// Invite expiry and idle-stranger checks. `false` = drop the link.
fn timers(shared: &Shared, token: u64, link: &Arc<dyn Link>, id: &str) -> bool {
    let t = &shared.timing;
    let now = Instant::now();
    let mut st = shared.state();
    if !current(&st, id, token) {
        return false; // registration was rejected or the link was replaced
    }
    let decline = st.invites_in.get(id).is_some_and(|s| now.duration_since(*s) > t.invite_timeout);
    if decline {
        st.invites_in.remove(id);
    }
    let expired = st
        .invites_out
        .get(id)
        .is_some_and(|i| i.sent && now.duration_since(i.since) > t.invite_timeout + t.conn_timeout);
    if expired {
        st.invites_out.remove(id);
        shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: false });
    }
    let authed = st.conns.get(id).is_some_and(|c| c.announced);
    // A stranger link with no invite in flight has no business staying open.
    let idle = !authed
        && !st.invites_in.contains_key(id)
        && !st.invites_out.contains_key(id)
        && st.conns.get(id).is_some_and(|c| now.duration_since(c.since) > t.conn_timeout);
    drop(st);
    if decline && !link.send(&ctl(Control::InviteReply { accepted: false, secret: None })) {
        return false;
    }
    !(idle || (expired && !authed))
}

/// Session teardown. `registered` is the id this link was registered under
/// (if any); `dialed` the id we dialed (if outgoing) — dialing/invite
/// bookkeeping always uses the dialed id, never the id the peer claimed.
fn cleanup(shared: &Shared, token: u64, registered: Option<String>, dialed: Option<&str>) {
    if let Some(d) = dialed {
        if registered.as_deref() != Some(d) {
            dial_failed(shared, d);
        }
    }
    let mut st = shared.state();
    if let Some(d) = dialed {
        st.dialing.remove(d);
    }
    let Some(id) = registered else {
        return;
    };
    if !current(&st, &id, token) {
        return; // a newer link owns this peer now (or registration was refused)
    }
    let conn = st.conns.remove(&id).expect("current conn exists");
    st.invites_in.remove(&id);
    if st.invites_out.remove(&id).is_some() {
        shared.emit(NetEvent::InviteAnswered { from: id.clone(), accepted: false });
    }
    if conn.announced {
        shared.emit(NetEvent::Disconnected { id });
    }
}

/// A dial to `id` ended without the peer proving that id.
pub(crate) fn dial_failed(shared: &Shared, id: &str) {
    let mut st = shared.state();
    st.dialing.remove(id);
    if st.invites_out.get(id).is_some_and(|i| !i.sent) {
        st.invites_out.remove(id);
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
    let now = Instant::now();
    if let Some(link) = st.conns.get(peer_id).map(|c| c.link.clone()) {
        st.invites_out.insert(peer_id.to_string(), OutInvite { pet: pet.to_string(), sent: true, since: now });
        drop(st);
        link.send(&ctl(Control::Invite { pet: pet.to_string() }));
        return;
    }
    let Some(addr) = st.seen.get(peer_id).and_then(pick_candidate) else {
        shared.emit(NetEvent::InviteAnswered { from: peer_id.to_string(), accepted: false });
        return;
    };
    // Sent by `register` once the dialed peer says hello with this id.
    st.invites_out.insert(peer_id.to_string(), OutInvite { pet: pet.to_string(), sent: false, since: now });
    if st.dialing.insert(peer_id.to_string()) {
        drop(st);
        tcp::dial(shared, peer_id.to_string(), addr);
    }
}

pub(crate) fn answer_invite(shared: &Shared, peer_id: &str, accept: bool) {
    let mut st = shared.state();
    if st.invites_in.remove(peer_id).is_none() {
        return;
    }
    let Some(link) = st.conns.get(peer_id).map(|c| c.link.clone()) else {
        return;
    };
    let secret = accept.then(auth::random_hex);
    if let Some(s) = &secret {
        st.add_friend(peer_id, s);
        if let Some(c) = st.conns.get_mut(peer_id) {
            if !c.announced {
                c.announced = true;
                shared.emit(NetEvent::Connected { id: peer_id.to_string(), name: c.name.clone() });
            }
        }
    }
    drop(st);
    // On decline the inviter closes the link after reading the reply (closing
    // here first could reset the connection before the reply is read).
    link.send(&ctl(Control::InviteReply { accepted: accept, secret }));
}
