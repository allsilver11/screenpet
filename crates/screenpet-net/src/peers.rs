//! Per-peer session driver: `hello` handshake, invites, friendship, visit
//! delivery, ping and timeouts. Transport-independent: it only sees a
//! `Link`/`LinkReader` pair (see `transport`).

use crate::protocol::{Control, Message, PROTOCOL_VERSION};
use crate::transport::{Link, LinkReader, Recv};
use crate::{tcp, Conn, NetEvent, OutInvite, Shared, State};
use std::sync::Arc;
use std::time::Instant;

fn ctl(c: Control) -> Message {
    Message::Control(c)
}

/// Is `token` the session currently registered for `id`?
fn current(st: &State, id: &str, token: u64) -> bool {
    st.conns.get(id).is_some_and(|c| c.token == token)
}

/// Drive one link until it closes, times out, misbehaves or we shut down.
/// `dialed` is the peer id we connected to (outgoing), `None` if accepted.
pub(crate) fn run_session(shared: Arc<Shared>, link: Arc<dyn Link>, mut reader: Box<dyn LinkReader>, dialed: Option<String>) {
    let token = shared.token();
    let t = shared.timing.clone();
    let mut peer: Option<(String, String)> = None; // (id, name) after hello
    let mut last_recv = Instant::now();
    let mut last_ping = Instant::now();
    let mut ok = link.send(&ctl(Control::Hello {
        id: shared.id.clone(),
        name: shared.name.clone(),
        v: PROTOCOL_VERSION,
    }));
    while ok && !shared.stopping() {
        match reader.recv() {
            Recv::Closed | Recv::Invalid => break,
            Recv::Idle => {}
            Recv::Msg(msg) => {
                last_recv = Instant::now();
                ok = match &peer {
                    None => match msg {
                        Message::Control(Control::Hello { id, name, .. }) => {
                            let registered = register(&shared, token, &link, dialed.is_some(), &id, &name);
                            peer = Some((id, name));
                            registered
                        }
                        _ => false, // anything before hello is a protocol error
                    },
                    Some((id, name)) => handle(&shared, token, &link, id, name, msg),
                };
                if !ok {
                    break;
                }
            }
        }
        let now = Instant::now();
        if now.duration_since(last_recv) > t.conn_timeout {
            break;
        }
        if now.duration_since(last_ping) >= t.ping_interval {
            ok = link.send(&ctl(Control::Ping));
            last_ping = now;
        }
        if let Some((id, _)) = &peer {
            ok = ok && timers(&shared, token, &link, id);
        }
    }
    link.close();
    cleanup(&shared, token, peer.map(|p| p.0), dialed);
}

/// Record the session for `id` after its hello. `false` = reject the link.
fn register(shared: &Shared, token: u64, link: &Arc<dyn Link>, outgoing: bool, id: &str, name: &str) -> bool {
    if id.is_empty() || id == shared.id {
        return false;
    }
    let dialer = if outgoing { shared.id.clone() } else { id.to_string() };
    let mut connected = false;
    let mut send_invite = None;
    {
        let mut st = shared.state();
        if outgoing {
            st.dialing.remove(id);
        }
        let mut announced = false;
        if let Some(old) = st.conns.get(id) {
            // One link per peer. A newer link from the same dialer replaces a
            // stale one; between links dialed by different sides, both peers
            // keep the one dialed by the smaller id.
            let min = if shared.id.as_str() < id { shared.id.as_str() } else { id };
            if old.dialer != dialer && dialer != min {
                return false;
            }
            announced = old.announced;
            old.link.close();
        }
        if st.is_friend(id) && !announced {
            announced = true;
            connected = true;
        }
        if let Some(inv) = st.invites_out.get_mut(id) {
            if !inv.sent {
                inv.sent = true;
                inv.since = Instant::now();
                send_invite = Some(inv.pet.clone());
            }
        }
        st.conns.insert(
            id.to_string(),
            Conn { token, link: link.clone(), name: name.to_string(), announced, dialer, since: Instant::now() },
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
fn handle(shared: &Shared, token: u64, link: &Arc<dyn Link>, id: &str, name: &str, msg: Message) -> bool {
    let mut st = shared.state();
    if !current(&st, id, token) {
        return false; // replaced by a newer link
    }
    match msg {
        Message::Control(Control::Hello { .. }) | Message::Control(Control::Ping) => true,
        Message::Control(Control::Invite { pet }) => {
            if st.is_friend(id) {
                drop(st);
                return link.send(&ctl(Control::InviteReply { accepted: true }));
            }
            if !st.invites_in.contains_key(id) {
                st.invites_in.insert(id.to_string(), Instant::now());
                shared.emit(NetEvent::Invited { from: id.to_string(), name: name.to_string(), pet });
            }
            true
        }
        Message::Control(Control::InviteReply { accepted }) => {
            if st.invites_out.remove(id).is_none() {
                return true; // unsolicited: ignore
            }
            shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted });
            if !accepted {
                return st.is_friend(id);
            }
            st.add_friend(id);
            if let Some(c) = st.conns.get_mut(id) {
                if !c.announced {
                    c.announced = true;
                    shared.emit(NetEvent::Connected { id: id.to_string(), name: name.to_string() });
                }
            }
            true
        }
        Message::Control(Control::Unfriend) => {
            st.friends.retain(|f| f != id);
            st.invites_in.remove(id);
            st.invites_out.remove(id);
            false
        }
        Message::Visit(v) => {
            // Only friends may deliver visits; anyone else is ignored.
            if st.is_friend(id) && st.conns.get(id).is_some_and(|c| c.announced) {
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
        return true; // registration was rejected or replaced; reader will end
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
    // A non-friend link with no invite in flight has no business staying open.
    let idle = !st.is_friend(id)
        && !st.invites_in.contains_key(id)
        && !st.invites_out.contains_key(id)
        && st.conns.get(id).is_some_and(|c| now.duration_since(c.since) > t.conn_timeout);
    let friend = st.is_friend(id);
    drop(st);
    if decline && !link.send(&ctl(Control::InviteReply { accepted: false })) {
        return false;
    }
    !(idle || (expired && !friend))
}

fn cleanup(shared: &Shared, token: u64, peer: Option<String>, dialed: Option<String>) {
    let Some(id) = peer else {
        if let Some(d) = dialed {
            dial_failed(shared, &d);
        }
        return;
    };
    let mut st = shared.state();
    if !current(&st, &id, token) {
        return; // a newer link owns this peer now
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

/// A dial attempt ended before the peer said hello.
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
    let Some(addr) = st.seen.get(peer_id).map(|s| s.addr) else {
        shared.emit(NetEvent::InviteAnswered { from: peer_id.to_string(), accepted: false });
        return;
    };
    // Sent by `register` once the link says hello.
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
    if accept {
        st.add_friend(peer_id);
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
    link.send(&ctl(Control::InviteReply { accepted: accept }));
}
