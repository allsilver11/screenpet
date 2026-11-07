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
//! - **Both users confirm.** A matched code only asks each user "본인의 PC가
//!   맞습니까?" (`NetEvent::DeviceConfirm`); S stays on that link alone
//!   (`DevLink::confirming`) — not stored, nothing shared — until both pressed
//!   확인 (`device_confirm` each way). A no, closing the window, no answer
//!   within `Timing::device_confirm_timeout`, or losing the link cancels on
//!   both sides (`device_unlink`, `NetEvent::DeviceLinkCancelled`) and S is
//!   gone; a restart finds nothing.
//! - **Every later link proves S again** (fresh nonces from both sides, each
//!   side checks the other's proof). Until it checks out the link is just a
//!   friend: `device_friends` is neither sent nor taken. A wrong proof, or a
//!   peer that holds no secret for us, unlinks both (`device_unlink`).
//! - **One of the two plays.** On each proven link both send `device_active`
//!   (who plays as (term, id), and how long ago their user last used them);
//!   the highest (term, id) plays, the computer used last claims on meeting,
//!   and `Net::claim_active` claims at any time. The other is on standby: no
//!   friend links (none dialed, friends' keys closed before hello, open ones
//!   closed after `Timing::standby_grace`). Out of reach = both play.
//! - **The pet and its growth follow the computer that plays.** It sends
//!   `device_state` (selected pet, every pet's care) on the proven link right
//!   after both met, when it starts playing, on a change (coalesced to one per
//!   `Timing::device_state_every`) and every `Timing::device_state_refresh`;
//!   the one on standby takes it from the computer that plays only. A claim
//!   (`Net::claim_active`) waits up to `device_state_every` for the other's
//!   last state (`handover`), taken unless the claimer changed its own since
//!   it last took one. The claim that settles who plays as the two meet again
//!   (`meet`) hands nothing over: the computer used last wins.
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
use crate::protocol::{clamp_name, Control, WireCare, WireFriend, MAX_DEVICES, MAX_FRIENDS};
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

/// A matched code waiting for both users' 확인 (gone with the link).
pub(crate) struct Confirming {
    secret: [u8; 32],
    /// I pressed 확인 (and sent `device_confirm`).
    mine: bool,
    /// The other computer sent `device_confirm`.
    theirs: bool,
    until: Instant,
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
    /// The code matched on this link; waiting for both users' 확인.
    pub confirming: Option<Confirming>,
    /// 📱 I told my state (who plays, my idle time) on this proven link.
    active_sent: bool,
    /// The other computer's state came on this link (and I answered it).
    active_heard: bool,
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

/// The code matched with `id` on link `token` (secret `s`): ask the user,
/// keep `s` on this link only until both confirm.
fn ask(shared: &Shared, st: &mut State, token: u64, id: &str, s: [u8; 32]) {
    st.link_code = None; // single use
    let until = Instant::now() + shared.timing.device_confirm_timeout;
    let Some(d) = dev(st, token) else { return };
    d.entering = None;
    d.holding = None;
    d.confirming = Some(Confirming { secret: s, mine: false, theirs: false, until });
    shared.emit(NetEvent::DeviceConfirm { id: id.to_string() });
}

/// Both confirmed on link `token`: linked to `id`, proven on that link.
/// `false` (nothing linked) if I got linked to another computer meanwhile.
fn link(shared: &Shared, st: &mut State, token: u64, id: &str) -> bool {
    let Some(s) = dev(st, token).and_then(|d| d.confirming.take()).map(|c| c.secret) else { return false };
    if linked_elsewhere(st, id) {
        shared.emit(NetEvent::DeviceLinkCancelled { id: id.to_string() });
        return false;
    }
    st.device = Some((id.to_string(), s));
    st.active = (0, shared.id.clone()); // a new pair starts over
    st.via.remove(id); // mine now, not introduced by anyone
    st.link_code = None;
    if let Some(d) = dev(st, token) {
        d.proven = true;
        d.entering = None;
        d.holding = None;
    }
    shared.emit(NetEvent::DeviceLinked { id: id.to_string() });
    true
}

/// The user's answer for friend `id` (see `Net::confirm_device`): the link to
/// tell and what to tell it, or `None` if nothing waits for an answer.
pub(crate) fn answer(shared: &Shared, st: &mut State, id: &str, yes: bool) -> Option<(Arc<SecureLink>, Control)> {
    let token = *st.friend_links.get(id)?;
    let conn = st.conns.get_mut(&token)?;
    let to = conn.link.clone();
    let c = conn.dev.confirming.as_mut()?;
    if !yes {
        conn.dev.confirming = None;
        shared.emit(NetEvent::DeviceLinkCancelled { id: id.to_string() });
        return Some((to, Control::DeviceUnlink));
    }
    if c.mine {
        return None; // already answered
    }
    c.mine = true;
    if c.theirs && !link(shared, st, token, id) {
        return Some((to, Control::DeviceUnlink));
    }
    Some((to, Control::DeviceConfirm))
}

/// Cancel the confirms nobody answered in time (called every tick).
pub(crate) fn expire_confirms(shared: &Shared) {
    let now = Instant::now();
    let mut gone = Vec::new();
    {
        let mut st = shared.state();
        for c in st.conns.values_mut() {
            if c.dev.confirming.as_ref().is_some_and(|k| now >= k.until) {
                c.dev.confirming = None;
                gone.push((c.peer_id.clone(), c.link.clone()));
            }
        }
    }
    for (id, link) in gone {
        shared.emit(NetEvent::DeviceLinkCancelled { id });
        link.send(&crate::protocol::Message::Control(Control::DeviceUnlink));
    }
}

fn linked_elsewhere(st: &State, id: &str) -> bool {
    st.device.as_ref().is_some_and(|(d, _)| d != id)
}

/// A `device_*` message on friend link `token` from `id` (authenticated,
/// version-checked and budgeted by the caller). Returns the replies to send.
pub(crate) fn on_link_message(shared: &Shared, st: &mut State, token: u64, id: &str, msg: Control) -> Vec<Control> {
    let now = Instant::now();
    let me = shared.public_key;
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
            ask(shared, st, token, id, derive_secret(&key, &me, &them, &na, &nb));
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
            ask(shared, st, token, id, derive_secret(&e.key, &them, &me, &na, &e.nb));
            Vec::new()
        }
        // The other user pressed 확인
        Control::DeviceConfirm => {
            let Some(c) = dev(st, token).and_then(|d| d.confirming.as_mut()) else { return Vec::new() };
            c.theirs = true;
            if c.mine && !link(shared, st, token, id) {
                return vec![Control::DeviceUnlink];
            }
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
        // My linked computer says who plays (its state once per link, or a claim)
        Control::DeviceActive { term, you, idle, meet } => {
            let Some(term) = term.filter(|t| *t <= MAX_TERM) else { return Vec::new() };
            if !st.device.as_ref().is_some_and(|(d, _)| d == id) {
                return Vec::new();
            }
            let was_playing = st.device_standby && st.active.1 == shared.id;
            let Some(d) = dev(st, token).filter(|d| d.proven) else { return Vec::new() };
            // I played and we had met on this link: a claim takes over from me
            let was_playing = was_playing && d.active_heard;
            let first = idle.is_some() && !d.active_heard;
            if idle.is_some() {
                d.active_heard = true;
            }
            let holder = if you { shared.id.clone() } else { id.to_string() };
            if (term, &holder) > (st.active.0, &st.active.1) {
                st.active = (term, holder);
            }
            // We meet again: the computer used last plays (equal: the smaller id)
            let mine = st.last_input.map_or(u64::MAX, |t| now.duration_since(t).as_secs());
            let theirs = idle.unwrap_or(u64::MAX);
            let used_last = mine < theirs || (mine == theirs && shared.id.as_str() < id);
            let mut replies = if first && used_last && st.active.1 != shared.id { vec![claim_in(st, &shared.id, true)] } else { Vec::new() };
            if first {
                // Both states met on this link: it counts as reachable from now
                st.device_seen = Some(now);
            }
            decide(shared, st, now);
            // It took over from me: my last state goes along (never when we
            // just met again — then the computer used last wins as it is)
            if was_playing && !meet && st.active.1 != shared.id {
                if let Some(s) = st.dstate.clone() {
                    st.dstate_dirty = false;
                    replies.push(Control::DeviceState { pet: s.pet, care: s.care, handover: true });
                }
            }
            replies
        }
        // 📱 My linked computer's pet and care
        Control::DeviceState { pet, care, handover } => {
            if !st.device.as_ref().is_some_and(|(d, _)| d == id) || !dev(st, token).is_some_and(|d| d.proven) {
                return Vec::new();
            }
            let playing = st.active.1 == shared.id;
            let take = if handover {
                // I just took over: its last state, unless I changed mine since I last took one
                if playing {
                    st.handover_until = None;
                }
                playing && !st.dstate_edited
            } else {
                st.active.1 == id // from the computer that plays only
            };
            if !take {
                return Vec::new();
            }
            let s = clean_state(pet, care);
            // Taken over: mine to share now (it may have missed something meanwhile)
            st.dstate_dirty = handover;
            st.dstate_edited = false;
            if st.dstate.as_ref() != Some(&s) {
                st.dstate = Some(s.clone());
                shared.emit(NetEvent::DeviceState { from: id.to_string(), pet: s.pet, care: s.care });
            }
            Vec::new()
        }
        Control::DeviceUnlink => {
            if let Some(d) = dev(st, token) {
                d.proven = false;
                if d.confirming.take().is_some() {
                    shared.emit(NetEvent::DeviceLinkCancelled { id: id.to_string() });
                }
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

/// Terms beyond this are nonsense (JSON numbers stay exact up to 2^53).
const MAX_TERM: u64 = 1 << 53;

/// Claim the play in `st` (my id `me`): the claim to send. `meet` = it
/// settles who plays as we meet again (nothing is handed over).
fn claim_in(st: &mut State, me: &str, meet: bool) -> Control {
    st.active = ((st.active.0 + 1).min(MAX_TERM), me.to_string());
    Control::DeviceActive { term: Some(st.active.0), you: false, idle: None, meet }
}

/// 📱 The user plays on this computer: the claim and the proven link to my
/// linked computer to send it on (`None` if there is no such link; the
/// claim still counts here).
pub(crate) fn claim(shared: &Shared) -> Option<(Arc<SecureLink>, Control)> {
    let mut st = shared.state();
    let now = Instant::now();
    st.last_input = Some(now);
    if st.active.1 != shared.id {
        // Wait a moment for the other computer's last state before sharing mine
        st.handover_until = Some(now + shared.timing.device_state_every);
    }
    let msg = claim_in(&mut st, &shared.id, false);
    decide(shared, &mut st, Instant::now());
    let link = proven_device_link(&st)?;
    Some((link, msg))
}

/// The link to my linked computer, if it is up, proven and knows `device_active`.
fn proven_device_link(st: &State) -> Option<Arc<SecureLink>> {
    let (d, _) = st.device.as_ref()?;
    st.friend_conn(d).filter(|c| matches!(c.kind, crate::Kind::Friend) && c.dev.proven && c.v >= crate::protocol::DEVICES_MIN_VERSION).map(|c| c.link.clone())
}

/// Who plays now (my linked computer out of reach for `conn_timeout` = both):
/// updates `State::standby` / `device_standby` and reports a change. Called
/// every tick and at once when who plays changes, so `Net::standby` is right
/// as soon as a claim is made or heard. Returns whether this one is on standby.
fn decide(shared: &Shared, st: &mut State, now: Instant) -> bool {
    let device = st.device.as_ref().map(|(d, _)| d.clone());
    let reachable = device.is_some() && st.device_seen.is_some_and(|t| now.duration_since(t) <= shared.timing.conn_timeout);
    let standby = reachable && device.as_deref() == Some(st.active.1.as_str());
    let other = reachable && st.active.1 == shared.id;
    if (standby, other) != (st.standby, st.device_standby) {
        if standby && !st.standby {
            st.standby_since = Some(now);
        }
        st.standby = standby;
        st.device_standby = other;
        shared.emit(NetEvent::Standby { me: standby, device: other });
    }
    standby
}

/// 📱 Every tick: tell my linked computer my state on each new proven link,
/// work out who plays, report changes, and on standby close friend links
/// once the grace is over. My linked computer out of reach for `conn_timeout`
/// = both play.
pub(crate) fn tick_active(shared: &Shared) {
    let now = Instant::now();
    let mut send = Vec::new();
    let mut close = Vec::new();
    {
        let mut st = shared.state();
        let device = st.device.as_ref().map(|(d, _)| d.clone());
        if let Some(link) = proven_device_link(&st) {
            let idle = st.last_input.map_or(0, |t| now.duration_since(t).as_secs());
            let (term, you) = (st.active.0, device.as_deref() == Some(st.active.1.as_str()));
            let token = device.as_ref().and_then(|d| st.friend_links.get(d).copied());
            if let Some(d) = token.and_then(|t| dev(&mut st, t)) {
                if !d.active_sent {
                    d.active_sent = true;
                    send.push((link, Control::DeviceActive { term: Some(term), you, idle: Some(idle), meet: false }));
                }
                // Only once both states met on this link (no flip before that)
                if d.active_heard {
                    st.device_seen = Some(now);
                }
            }
        }
        let standby = decide(shared, &mut st, now);
        // (after my state on this link: the other hears who plays first)
        send.extend(state_due(shared, &mut st, now));
        if standby && st.standby_since.is_some_and(|t| now.duration_since(t) >= shared.timing.standby_grace) {
            for (id, t) in &st.friend_links {
                if st.standby_blocks(id) {
                    if let Some(c) = st.conns.get(t).filter(|c| !c.link.closing()) {
                        close.push(c.link.clone());
                    }
                }
            }
        }
    }
    for (link, msg) in send {
        link.send(&crate::protocol::Message::Control(msg));
    }
    for l in close {
        l.finish();
    }
}

/// 📱 Most pets in one `device_state` (both ways), …
pub(crate) const MAX_CARE: usize = 64;
/// … pet ids (folder names: a-z, 0-9, _) and stage names this long at most.
const MAX_PET_ID: usize = 32;
const MAX_STAGE: usize = 16;

/// 📱 A selected pet and every pet's care, shape-checked (`clean_state`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SharedState {
    pub pet: Option<String>,
    pub care: Vec<WireCare>,
}

fn is_word(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Keep what looks like a pet: pet ids and stages as words, finite numbers,
/// one entry per pet (the first), at most `MAX_CARE`. Species and value
/// ranges are the app's to check.
pub(crate) fn clean_state(pet: Option<String>, care: Vec<WireCare>) -> SharedState {
    let mut kept: Vec<WireCare> = Vec::new();
    for c in care {
        if kept.len() >= MAX_CARE {
            break;
        }
        let ok = is_word(&c.pet, MAX_PET_ID) && is_word(&c.stage, MAX_STAGE) && c.stamina.is_finite() && c.fullness.is_finite();
        if ok && !kept.iter().any(|k| k.pet == c.pet) {
            kept.push(c);
        }
    }
    SharedState { pet: pet.filter(|p| is_word(p, MAX_PET_ID)), care: kept }
}

/// My pet or care changed (`Net::share_state`): it goes out while I play.
pub(crate) fn share(st: &mut State, pet: &str, care: Vec<WireCare>) {
    let s = clean_state(Some(pet.to_string()), care);
    if st.dstate.as_ref() != Some(&s) {
        st.dstate = Some(s);
        st.dstate_dirty = true;
        st.dstate_edited = true;
    }
}

/// 📱 My state for my linked computer, if it is due: I play, it is on
/// standby on a proven link where both states met, no handover is awaited;
/// and this link or my turn to play is new, or it changed and the last one
/// went out `device_state_every` ago, or `device_state_refresh` passed.
fn state_due(shared: &Shared, st: &mut State, now: Instant) -> Option<(Arc<SecureLink>, Control)> {
    if !(st.device_standby && st.active.1 == shared.id) {
        return None;
    }
    if st.handover_until.is_some_and(|t| now < t) {
        return None;
    }
    st.handover_until = None;
    let link = proven_device_link(st)?;
    let token = st.device.as_ref().and_then(|(d, _)| st.friend_links.get(d).copied())?;
    let met = st.conns.get(&token).is_some_and(|c| c.dev.active_sent && c.dev.active_heard);
    let s = st.dstate.clone().filter(|_| met)?;
    let t = &shared.timing;
    let due = match st.dstate_sent {
        Some((tk, term, at)) if (tk, term) == (token, st.active.0) => (st.dstate_dirty && now >= at + t.device_state_every) || now >= at + t.device_state_refresh,
        _ => true,
    };
    if !due {
        return None;
    }
    st.dstate_sent = Some((token, st.active.0, now));
    st.dstate_dirty = false;
    Some((link, Control::DeviceState { pet: s.pet, care: s.care, handover: false }))
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
        && key != shared.public_key
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
