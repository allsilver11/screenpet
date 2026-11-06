//! Linked devices (📱 내 다른 컴퓨터): one person's computers each keep their
//! own identity but share their friend lists.
//!
//! - `device_friends` (my friends) goes only to a friend I marked as my device,
//!   and is taken only from a friend I marked as my device — so it flows only
//!   on a link both sides marked. It only ever adds friends I do not know.
//! - `my_devices` (my other computers, at most `MAX_DEVICES`) goes to my other
//!   friends; they add those computers as friends that remember who introduced
//!   them (`via`) and leave together with that friend.
//!
//! Everything received is checked here: install-id format, 32-byte key, never
//! my own id or key, never an id or key already pinned (no key is ever
//! replaced), `MAX_FRIENDS` in all. Friends added this way are *unconfirmed*
//! (`State::pending`) until they prove on a link that they pinned us too; until
//! then a hello saying "I don't know you" only backs off instead of removing
//! them (their introduction to us may simply not have arrived yet).

use crate::noise;
use crate::protocol::{clamp_name, WireFriend, MAX_DEVICES, MAX_FRIENDS};
use crate::secure::SecureLink;
use crate::{NetEvent, Shared, State};
use std::sync::Arc;

/// An install id as `new_install_id` makes them: 32 lowercase hex digits.
pub(crate) fn is_install_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Pin a friend that came from another computer of mine or from a friend's
/// introduction, if it is new in every way. Returns whether it was added.
fn adopt(shared: &Shared, st: &mut State, id: &str, key: &str) -> bool {
    let Some(key) = noise::hex32(key) else { return false };
    let ok = is_install_id(id)
        && id != shared.id
        && key != [0; 32]
        && key != noise::public_key(&shared.static_key)
        && !st.is_friend(id)
        && !st.friends.iter().any(|f| f.public_key == key)
        && st.friends.len() < MAX_FRIENDS;
    if ok {
        st.add_friend(id, key);
        st.pending.insert(id.to_string());
    }
    ok
}

/// `device_friends` from friend `from` (already authenticated and budgeted).
pub(crate) fn on_device_friends(shared: &Shared, st: &mut State, from: &str, friends: Vec<WireFriend>, want: bool) {
    if !st.devices.contains(from) {
        return; // only on a link I marked as my device
    }
    for f in friends.into_iter().take(MAX_FRIENDS) {
        if f.id != from && adopt(shared, st, &f.id, &f.key) {
            let side = f.side.map(|s| clamp_name(&s)).filter(|s| !s.is_empty());
            shared.emit(NetEvent::FriendAdded { id: f.id, name: clamp_name(&f.name), via: None, side });
        }
    }
    shared.emit(NetEvent::DeviceSynced { from: from.to_string(), want });
}

/// `my_devices` from friend `from` (already authenticated and budgeted).
pub(crate) fn on_my_devices(shared: &Shared, st: &mut State, from: &str, devices: Vec<crate::protocol::WireDevice>) {
    if st.devices.contains(from) {
        return; // my own computer's computers come as its friends instead
    }
    if st.via.contains_key(from) {
        return; // one hop only: a computer a friend introduced introduces no one
    }
    for d in devices.into_iter().take(MAX_DEVICES) {
        if st.via.values().filter(|v| v.as_str() == from).count() >= MAX_DEVICES {
            break;
        }
        if d.id != from && adopt(shared, st, &d.id, &d.key) {
            st.via.insert(d.id.clone(), from.to_string());
            shared.emit(NetEvent::FriendAdded { id: d.id, name: String::new(), via: Some(from.to_string()), side: None });
        }
    }
}

/// Friend `gone` was removed: remove the devices it introduced (and theirs).
/// Returns their friend links, to be told `unfriend` and closed by the caller
/// once the state lock is released.
pub(crate) fn drop_introduced(shared: &Shared, st: &mut State, gone: &str) -> Vec<Arc<SecureLink>> {
    let mut links = Vec::new();
    let mut todo = vec![gone.to_string()];
    while let Some(via) = todo.pop() {
        let ids: Vec<String> = st.via.iter().filter(|(_, v)| **v == via).map(|(id, _)| id.clone()).collect();
        for id in ids {
            st.remove_friend(&id);
            st.invites_out.remove(&id);
            if let Some(l) = st.demote_friend_link(&id) {
                links.push(l);
            }
            shared.emit(NetEvent::FriendDropped { id: id.clone() });
            todo.push(id);
        }
    }
    links
}

/// Tell the dropped devices' links we are no longer friends, and close them.
pub(crate) fn close_dropped(links: Vec<Arc<SecureLink>>) {
    for l in links {
        l.send(&crate::protocol::Message::Control(crate::protocol::Control::Unfriend));
        l.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_ids_are_32_lowercase_hex_digits() {
        assert!(is_install_id(&"0a".repeat(16)));
        assert!(!is_install_id(&"0A".repeat(16)));
        assert!(!is_install_id(&"0a".repeat(15)));
        assert!(!is_install_id(&format!("{}g", "0".repeat(31))));
        assert!(!is_install_id(""));
    }
}
