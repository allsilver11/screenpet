//! Linked devices (📱 내 다른 컴퓨터): one person's two computers each keep
//! their own identity but share their friend lists.
//!
//! - **At most two computers.** An install links at most one other computer
//!   (`State::device`); linking another is refused until it is unlinked.
//! - **Linking takes a code.** Over an authenticated friend link, computer A
//!   shows a fresh random code for friend B (`CODE_LEN` symbols, valid
//!   `CODE_TTL`, single use, cancelled after `CODE_TRIES` wrong tries); the
//!   user types it on B. The code never goes on the wire: B proves it knows
//!   K = BLAKE2s(label || code) over both nonces and both static keys, and
//!   only then does A prove it back (A never answers a wrong guess with
//!   anything that could test another guess offline). Both then store the
//!   same long-term secret S = BLAKE2s(label || K || keys || nonces).
//! - **Every later link proves S again** (fresh nonces from both sides, each
//!   side checks the other's proof). Until it checks out the link is just a
//!   friend: `device_friends` is neither sent nor taken. A wrong proof, or a
//!   peer that holds no secret for us, unlinks both (`device_unlink`).
//! - `device_friends` (my friends) only ever adds friends I do not know.
//! - `my_devices` (my other computer, at most `MAX_DEVICES`) goes to my other
//!   friends; they add it as a friend that remembers who introduced it
//!   (`via`) and leaves together with that friend. Never an id I see on the
//!   LAN, invite or am pairing with; and a real pairing with an introduced id
//!   replaces the introduced key.
//!
//! Everything received is checked here: install-id format, 32-byte key, never
//! my own id or key, never an id or key already pinned (no key is ever
//! replaced), `MAX_FRIENDS` in all. Friends added this way are *unconfirmed*
//! (`State::pending`) until they prove on a link that they pinned us too; until
//! then a hello saying "I don't know you" only backs off instead of removing
//! them (their introduction to us may simply not have arrived yet). One that
//! stays unconfirmed for `PENDING_DAYS`, or says so `PENDING_STRIKES` times,
//! is dropped.

use crate::noise::{self, blake2s, hex32, random32, to_hex};
use crate::protocol::{clamp_name, Control, WireFriend, MAX_DEVICES, MAX_FRIENDS};
use crate::secure::SecureLink;
use crate::{NetEvent, Shared, State};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A link code lives this long …
pub(crate) const CODE_TTL: Duration = Duration::from_secs(10 * 60);
/// … and is cancelled after this many wrong tries.
pub(crate) const CODE_TRIES: u32 = 5;
/// Code symbols: no I, L, O, 0 or 1 (easy to mix up when typing).
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
pub(crate) const CODE_LEN: usize = 8;

const L_CODE: &[u8] = b"screenpet-device/1 code";
const L_CODE_PROOF: &[u8] = b"screenpet-device/1 code proof";
const L_SECRET: &[u8] = b"screenpet-device/1 secret";
const L_PROOF: &[u8] = b"screenpet-device/1 proof";

/// The code this computer shows for one friend (one at a time).
pub(crate) struct LinkCode {
    pub friend: String,
    key: [u8; 32],
    expires: Instant,
    wrong: u32,
}

impl LinkCode {
    fn live_for(&self, id: &str, now: Instant) -> bool {
        self.friend == id && now < self.expires
    }
}

/// Typed a code for the peer of this link: K, my nonce, its nonce once it came.
struct Entering {
    key: [u8; 32],
    nb: [u8; 32],
    na: Option<[u8; 32]>,
}

/// Device-link state of one friend link (gone with the link).
#[derive(Default)]
pub(crate) struct DevLink {
    /// My nonce for proving the stored secret on this link (sent on connect).
    pub mine: Option<[u8; 32]>,
    theirs: Option<[u8; 32]>,
    /// The peer proved our stored secret on this link, or we just linked on it.
    pub proven: bool,
    entering: Option<Entering>,
    /// I hold a code for the peer and sent my nonce: (nb, na).
    holding: Option<([u8; 32], [u8; 32])>,
}

/// A fresh code as shown to the user, `ABCD-EFGH`, and its key.
pub(crate) fn new_code(friend: &str) -> (String, LinkCode) {
    let mut code = String::new();
    while code.len() < CODE_LEN {
        // Rejection sampling keeps every symbol equally likely
        let n = CODE_ALPHABET.len();
        for b in random32() {
            if usize::from(b) < 256 / n * n && code.len() < CODE_LEN {
                code.push(char::from(CODE_ALPHABET[usize::from(b) % n]));
            }
        }
    }
    let key = code_key(&code);
    let shown = format!("{}-{}", &code[..4], &code[4..]);
    (shown, LinkCode { friend: friend.to_string(), key, expires: Instant::now() + CODE_TTL, wrong: 0 })
}

/// A typed code (any case, spaces and dashes ignored) → its key, if it is one.
pub(crate) fn typed_code_key(typed: &str) -> Option<[u8; 32]> {
    let code: String = typed.chars().filter(|c| !c.is_whitespace() && *c != '-').map(|c| c.to_ascii_uppercase()).collect();
    (code.len() == CODE_LEN && code.bytes().all(|b| CODE_ALPHABET.contains(&b))).then(|| code_key(&code))
}

fn code_key(code: &str) -> [u8; 32] {
    blake2s(&[L_CODE, code.as_bytes()])
}

fn enter_proof(k: &[u8; 32], nb: &[u8; 32], na: &[u8; 32], enterer: &[u8; 32], holder: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_CODE_PROOF, b"enter", k, nb, na, enterer, holder])
}

fn hold_proof(k: &[u8; 32], nb: &[u8; 32], na: &[u8; 32], holder: &[u8; 32], enterer: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_CODE_PROOF, b"hold", k, nb, na, holder, enterer])
}

fn derive_secret(k: &[u8; 32], holder: &[u8; 32], enterer: &[u8; 32], na: &[u8; 32], nb: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_SECRET, k, holder, enterer, na, nb])
}

fn secret_proof(s: &[u8; 32], prover: &[u8; 32], verifier: &[u8; 32], n_verifier: &[u8; 32], n_prover: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_PROOF, s, prover, verifier, n_verifier, n_prover])
}

/// Compare proofs without an early exit.
fn same(a: &[u8; 32], b: Option<[u8; 32]>) -> bool {
    b.is_some_and(|b| a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0)
}

/// Start typing `key` (a code friend link `token` holds for us): my nonce goes out.
pub(crate) fn start_entering(st: &mut State, token: u64, key: [u8; 32]) -> Option<Control> {
    let nb = random32();
    st.conns.get_mut(&token)?.dev.entering = Some(Entering { key, nb, na: None });
    Some(Control::DeviceCode { nonce: to_hex(&nb) })
}

/// A fresh nonce for proving the stored secret on a new link to my linked computer.
pub(crate) fn link_nonce() -> [u8; 32] {
    random32()
}

fn no(reason: &str, left: Option<u32>) -> Control {
    Control::DeviceNo { reason: reason.to_string(), left }
}

fn dev(st: &mut State, token: u64) -> Option<&mut DevLink> {
    st.conns.get_mut(&token).map(|c| &mut c.dev)
}

/// Linked to `id` with secret `s`, proven on link `token`.
fn link(shared: &Shared, st: &mut State, token: u64, id: &str, s: [u8; 32]) {
    st.device = Some((id.to_string(), s));
    st.via.remove(id); // mine now, not introduced by anyone
    st.link_code = None;
    if let Some(d) = dev(st, token) {
        d.proven = true;
        d.entering = None;
        d.holding = None;
    }
    shared.emit(NetEvent::DeviceLinked { id: id.to_string() });
}

fn linked_elsewhere(st: &State, id: &str) -> bool {
    st.device.as_ref().is_some_and(|(d, _)| d != id)
}

/// A `device_*` message on friend link `token` from `id` (authenticated,
/// version-checked and budgeted by the caller). Returns the replies to send.
pub(crate) fn on_link_message(shared: &Shared, st: &mut State, token: u64, id: &str, msg: Control) -> Vec<Control> {
    let now = Instant::now();
    let me = noise::public_key(&shared.static_key);
    let Some(them) = st.conns.get(&token).map(|c| c.remote_key) else { return Vec::new() };
    let fail = |reason: &str, left: u32| shared.emit(NetEvent::DeviceLinkFailed { id: id.to_string(), reason: reason.to_string(), left });
    match msg {
        // I hold a code: the enterer starts
        Control::DeviceCode { nonce } => {
            let Some(nb) = hex32(&nonce) else { return Vec::new() };
            if linked_elsewhere(st, id) {
                return vec![no("taken", None)];
            }
            if !st.link_code.as_ref().is_some_and(|c| c.live_for(id, now)) {
                return vec![no("none", None)];
            }
            let na = random32();
            let Some(d) = dev(st, token) else { return Vec::new() };
            d.holding = Some((nb, na));
            vec![Control::DeviceCodeNonce { nonce: to_hex(&na) }]
        }
        Control::DeviceCodeProof { proof } => {
            let Some((nb, na)) = dev(st, token).and_then(|d| d.holding.take()) else { return Vec::new() };
            if linked_elsewhere(st, id) {
                return vec![no("taken", None)];
            }
            let Some(code) = st.link_code.as_mut().filter(|c| c.live_for(id, now)) else { return vec![no("none", None)] };
            if !same(&enter_proof(&code.key, &nb, &na, &them, &me), hex32(&proof)) {
                code.wrong += 1;
                let left = CODE_TRIES.saturating_sub(code.wrong);
                if left == 0 {
                    st.link_code = None;
                    fail("locked", 0);
                    return vec![no("locked", None)];
                }
                fail("wrong", left);
                return vec![no("wrong", Some(left))];
            }
            let key = code.key;
            link(shared, st, token, id, derive_secret(&key, &me, &them, &na, &nb));
            vec![Control::DeviceCodeOk { proof: to_hex(&hold_proof(&key, &nb, &na, &me, &them)) }]
        }
        // I typed a code: the holder answers
        Control::DeviceCodeNonce { nonce } => {
            let Some(na) = hex32(&nonce) else { return Vec::new() };
            let Some(e) = dev(st, token).and_then(|d| d.entering.as_mut()).filter(|e| e.na.is_none()) else { return Vec::new() };
            e.na = Some(na);
            vec![Control::DeviceCodeProof { proof: to_hex(&enter_proof(&e.key, &e.nb, &na, &me, &them)) }]
        }
        Control::DeviceCodeOk { proof } => {
            let Some(e) = dev(st, token).and_then(|d| d.entering.take()) else { return Vec::new() };
            let Some(na) = e.na else { return Vec::new() };
            if !same(&hold_proof(&e.key, &e.nb, &na, &them, &me), hex32(&proof)) {
                fail("wrong", 0);
                return Vec::new();
            }
            if linked_elsewhere(st, id) {
                fail("taken", 0);
                return Vec::new();
            }
            link(shared, st, token, id, derive_secret(&e.key, &them, &me, &na, &e.nb));
            Vec::new()
        }
        Control::DeviceNo { reason, left } => {
            if dev(st, token).and_then(|d| d.entering.take()).is_some() {
                let reason = if matches!(reason.as_str(), "wrong" | "locked" | "taken") { reason.as_str() } else { "none" };
                fail(reason, left.unwrap_or(0).min(CODE_TRIES));
            }
            Vec::new()
        }
        // My linked computer proves our secret on this link
        Control::DeviceNonce { nonce } => {
            let Some(n) = hex32(&nonce) else { return Vec::new() };
            let Some(s) = st.device.as_ref().filter(|(d, _)| d == id).map(|(_, s)| *s) else {
                return vec![Control::DeviceUnlink]; // I hold no secret for it: it should forget too
            };
            let Some(d) = dev(st, token) else { return Vec::new() };
            let (Some(mine), None) = (d.mine, d.theirs) else { return Vec::new() };
            d.theirs = Some(n);
            vec![Control::DeviceProof { proof: to_hex(&secret_proof(&s, &me, &them, &n, &mine)) }]
        }
        Control::DeviceProof { proof } => {
            let Some(s) = st.device.as_ref().filter(|(d, _)| d == id).map(|(_, s)| *s) else { return Vec::new() };
            let Some(d) = dev(st, token) else { return Vec::new() };
            let (Some(mine), Some(theirs), false) = (d.mine, d.theirs, d.proven) else { return Vec::new() };
            if same(&secret_proof(&s, &them, &me, &mine, &theirs), hex32(&proof)) {
                d.proven = true;
                shared.emit(NetEvent::DeviceProven { id: id.to_string() });
                return Vec::new();
            }
            // It does not hold our secret: we are not linked any more
            st.device = None;
            shared.emit(NetEvent::DeviceUnlinked { id: id.to_string() });
            vec![Control::DeviceUnlink]
        }
        Control::DeviceUnlink => {
            if let Some(d) = dev(st, token) {
                d.proven = false;
            }
            if st.device.as_ref().is_some_and(|(d, _)| d == id) {
                st.device = None;
                shared.emit(NetEvent::DeviceUnlinked { id: id.to_string() });
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// An unconfirmed friend is dropped after this many days.
const PENDING_DAYS: u64 = 7;
/// … or after this many authenticated hellos saying it doesn't know us.
pub(crate) const PENDING_STRIKES: u32 = 3;

pub(crate) fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

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
        st.pending.insert(id.to_string(), unix_now());
    }
    ok
}

/// `device_friends` from friend `from` (already authenticated, budgeted, and
/// on a link where it proved our stored secret).
pub(crate) fn on_device_friends(shared: &Shared, st: &mut State, from: &str, friends: Vec<WireFriend>, want: bool) {
    if !st.device.as_ref().is_some_and(|(d, _)| d == from) {
        return; // only from my linked computer
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
    if st.device.as_ref().is_some_and(|(d, _)| d == from) {
        return; // my own computer's friends come as `device_friends` instead
    }
    if st.via.contains_key(from) {
        return; // one hop only: a computer a friend introduced introduces no one
    }
    for d in devices.into_iter().take(MAX_DEVICES) {
        if st.via.values().filter(|v| v.as_str() == from).count() >= MAX_DEVICES {
            break;
        }
        // Never someone I may be about to pair with: a made-up key under their
        // id would stand in the way of the real one
        let known = st.seen.contains_key(&d.id) || st.invites_out.contains_key(&d.id) || crate::peers::pairing_with(st, &d.id);
        if d.id != from && !known && adopt(shared, st, &d.id, &d.key) {
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

/// Forget unconfirmed friend `id` (and whoever it introduced). Returns links
/// for `close_dropped`, like `drop_introduced`.
pub(crate) fn drop_pending(shared: &Shared, st: &mut State, id: &str) -> Vec<Arc<SecureLink>> {
    st.remove_friend(id);
    st.invites_out.remove(id);
    let mut links: Vec<_> = st.demote_friend_link(id).into_iter().collect();
    shared.emit(NetEvent::FriendDropped { id: id.to_string() });
    links.extend(drop_introduced(shared, st, id));
    links
}

/// Drop the friends unconfirmed for `PENDING_DAYS` (called every tick).
pub(crate) fn expire_pending(shared: &Shared) {
    let now = unix_now();
    let mut st = shared.state();
    let old: Vec<String> = st.pending.iter().filter(|(_, t)| now.saturating_sub(**t) >= PENDING_DAYS * 24 * 60 * 60).map(|(id, _)| id.clone()).collect();
    let mut links = Vec::new();
    for id in old {
        links.extend(drop_pending(shared, &mut st, &id));
    }
    drop(st);
    close_dropped(links);
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
    fn a_code_expires_and_belongs_to_one_friend() {
        let (shown, code) = new_code("zz");
        assert_eq!(typed_code_key(&shown), Some(code.key));
        assert_eq!(typed_code_key(&shown.to_lowercase().replace('-', " ")), Some(code.key));
        let now = Instant::now();
        assert!(code.live_for("zz", now));
        assert!(!code.live_for("yy", now));
        assert!(!code.live_for("zz", now + CODE_TTL));
        assert_eq!(typed_code_key("ABCD-EFG1"), None, "1 is not a code symbol");
    }

    #[test]
    fn install_ids_are_32_lowercase_hex_digits() {
        assert!(is_install_id(&"0a".repeat(16)));
        assert!(!is_install_id(&"0A".repeat(16)));
        assert!(!is_install_id(&"0a".repeat(15)));
        assert!(!is_install_id(&format!("{}g", "0".repeat(31))));
        assert!(!is_install_id(""));
    }
}
