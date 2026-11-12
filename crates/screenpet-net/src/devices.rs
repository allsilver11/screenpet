//! 연결된 기기(📱 내 다른 컴퓨터)임: 한 사람의 두 컴퓨터가 각자 신원은 지키면서 친구 목록을 나눔.
//!
//! - **컴퓨터는 최대 둘.** 설치 하나는 다른 컴퓨터 하나까지만 연결함 (`State::device`). 끊기 전엔 또 연결 못 함.
//! - **연결은 코드로.** 인증된 친구 링크 위에서 A가 친구 B용 새 난수 코드를 보여 줌
//!   (`CODE_LEN`기호, `CODE_TTL` 동안 유효, 한 번만, `CODE_TRIES`번 틀리면 취소). 사용자가 B에 입력함.
//!   코드는 선에 실리지 않음: B가 두 nonce와 두 정적 키에 대해 K = BLAKE2s(label || code)를 안다고 증명해야
//!   A가 그다음 증명함 (A는 틀린 추측에 오프라인으로 다른 추측을 시험할 만한 답을 주지 않음).
//!   그 뒤 둘 다 같은 장기 비밀 S = BLAKE2s(label || K || keys || nonces)를 저장함.
//! - **두 사용자가 확인함.** 코드가 맞으면 각 사용자에게 "본인의 PC가 맞습니까?"(`NetEvent::DeviceConfirm`)만 물음.
//!   둘 다 확인(`device_confirm` 양방향)할 때까지 S는 그 링크에만 있음(`DevLink::confirming`, 저장·공유 없음).
//!   아니오·창 닫기·`Timing::device_confirm_timeout` 안에 답 없음·링크 끊김이면 양쪽 다 취소하고
//!   (`device_unlink`, `NetEvent::DeviceLinkCancelled`) S는 사라짐. 재시작해도 남은 게 없음.
//! - **이후 링크마다 S를 다시 증명함** (양쪽 새 nonce, 서로 상대 증명을 확인). 통과 전엔 그냥 친구임:
//!   `device_friends`를 주지도 받지도 않음. 틀린 증명이나 우리 비밀이 없는 상대면 둘 다 끊음(`device_unlink`).
//! - **둘 중 하나만 놂.** 증명된 링크마다 둘 다 `device_active`(누가 (term, id)로 노는지, 사용자가 마지막으로
//!   쓴 지 얼마인지)를 보냄. 가장 높은 (term, id)가 놀고, 만나면 마지막에 쓴 컴퓨터가 차지하며,
//!   `Net::claim_active`로 언제든 차지할 수 있음. 다른 쪽은 대기: 친구 링크 없음(다이얼 안 함, hello 전에
//!   친구 키는 닫고, 열린 링크는 `Timing::standby_grace` 뒤 닫음). 서로 닿지 않으면 둘 다 놂.
//! - **펫과 성장은 노는 컴퓨터를 따라감.** 노는 쪽이 `device_state`(고른 펫, 모든 펫의 돌봄)를 증명된 링크로 보냄:
//!   둘이 만난 직후, 놀기 시작할 때, 바뀔 때(`Timing::device_state_every`당 하나로 모음),
//!   `Timing::device_state_refresh`마다. 대기 쪽은 노는 컴퓨터에서만 받음. 차지(`Net::claim_active`)는
//!   상대의 마지막 상태를 `device_state_every`까지 기다려(`handover`) 받음. 단 차지한 쪽이 마지막으로 받은 뒤
//!   자기 것을 바꿨으면 안 받음. 다시 만나 누가 놀지 정하는 차지(`meet`)는 아무것도 넘기지 않음: 마지막에 쓴 쪽이 이김.
//! - `device_friends`(내 친구들)는 내가 모르는 친구만 더함.
//! - `my_devices`(내 다른 컴퓨터, 최대 `MAX_DEVICES`)는 내 다른 친구들에게 감. 그들은 소개한 친구(`via`)를 기억하는
//!   친구로 더하고 그 친구와 함께 뺌. LAN에서 보이거나 초대·짝짓기 중인 id는 안 받고,
//!   소개된 id와 진짜 짝짓기를 하면 소개된 키를 바꿈.
//!
//! 받은 것은 모두 여기서 검사함: 설치 id 형식, 32바이트 키, 내 id·키 아님, 이미 고정된 id·키 아님(키는 절대 안 바꿈),
//! 전체 `MAX_FRIENDS`. 이렇게 더한 친구는 링크에서 우리도 고정했음을 증명할 때까지 *미확인*(`State::pending`)임.
//! 그동안 "너 모름" hello는 지우지 않고 물러나기만 함(우리 소개가 아직 안 갔을 수 있음).
//! `PENDING_DAYS` 동안 미확인이거나 그렇게 `PENDING_STRIKES`번 말하면 뺌.

use crate::noise::{self, blake2s, hex32, random32, to_hex};
use crate::protocol::{clamp_name, Control, WireCare, WireFriend, MAX_DEVICES, MAX_FRIENDS};
use crate::secure::SecureLink;
use crate::{NetEvent, Shared, State};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 연결 코드는 이만큼 살고 …
pub(crate) const CODE_TTL: Duration = Duration::from_secs(10 * 60);
/// … 이만큼 틀리면 취소됨
pub(crate) const CODE_TRIES: u32 = 5;
/// 코드 기호임: I, L, O, 0, 1은 뺌 (입력할 때 헷갈림)
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
/// 코드 기호 수
pub(crate) const CODE_LEN: usize = 8;

/// 코드 키 라벨
const L_CODE: &[u8] = b"screenpet-device/1 code";
/// 코드 증명 라벨
const L_CODE_PROOF: &[u8] = b"screenpet-device/1 code proof";
/// 장기 비밀 유도 라벨
const L_SECRET: &[u8] = b"screenpet-device/1 secret";
/// 장기 비밀 증명 라벨
const L_PROOF: &[u8] = b"screenpet-device/1 proof";

/// 이 컴퓨터가 친구 한 명에게 보여 주는 코드임 (한 번에 하나)
pub(crate) struct LinkCode {
    /// 코드를 받을 친구 id
    pub friend: String,
    key: [u8; 32],
    expires: Instant,
    wrong: u32,
}

impl LinkCode {
    /// 이 친구에게 아직 유효한 코드인지 봄
    fn live_for(&self, id: &str, now: Instant) -> bool {
        self.friend == id && now < self.expires
    }
}

/// 이 링크 상대에게 코드를 입력한 상태임: K, 내 nonce, 온 뒤의 상대 nonce
struct Entering {
    key: [u8; 32],
    nb: [u8; 32],
    na: Option<[u8; 32]>,
}

/// 코드가 맞아 두 사용자의 확인을 기다리는 상태임 (링크와 함께 사라짐)
pub(crate) struct Confirming {
    secret: [u8; 32],
    /// 내가 확인을 눌렀음 (`device_confirm`도 보냄)
    mine: bool,
    /// 다른 컴퓨터가 `device_confirm`을 보냈음
    theirs: bool,
    until: Instant,
}

/// 친구 링크 하나의 기기 연결 상태임 (링크와 함께 사라짐)
#[derive(Default)]
pub(crate) struct DevLink {
    /// 이 링크에서 저장된 비밀을 증명할 내 nonce (연결 때 보냄)
    pub mine: Option<[u8; 32]>,
    theirs: Option<[u8; 32]>,
    /// 상대가 이 링크에서 저장된 비밀을 증명했거나 방금 여기서 연결했음
    pub proven: bool,
    entering: Option<Entering>,
    /// 상대용 코드를 쥐고 내 nonce를 보냈음: (nb, na)
    holding: Option<([u8; 32], [u8; 32])>,
    /// 이 링크에서 코드가 맞았음. 두 사용자의 확인을 기다림
    pub confirming: Option<Confirming>,
    /// 📱 이 증명된 링크에 내 상태(누가 노는지, 내 쉰 시간)를 알렸음
    active_sent: bool,
    /// 이 링크로 다른 컴퓨터의 상태가 왔음 (그리고 답했음)
    active_heard: bool,
}

/// 사용자에게 보일 새 코드(`ABCD-EFGH`)와 그 키를 만듦
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

/// 입력한 코드(대소문자 무관, 공백·대시 무시)를 키로 바꿈. 코드가 아니면 None임
pub(crate) fn typed_code_key(typed: &str) -> Option<[u8; 32]> {
    let code: String = typed.chars().filter(|c| !c.is_whitespace() && *c != '-').map(|c| c.to_ascii_uppercase()).collect();
    (code.len() == CODE_LEN && code.bytes().all(|b| CODE_ALPHABET.contains(&b))).then(|| code_key(&code))
}

/// 코드에서 K를 만듦
fn code_key(code: &str) -> [u8; 32] {
    blake2s(&[L_CODE, code.as_bytes()])
}

/// 코드를 입력한 쪽의 증명임
fn enter_proof(k: &[u8; 32], nb: &[u8; 32], na: &[u8; 32], enterer: &[u8; 32], holder: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_CODE_PROOF, b"enter", k, nb, na, enterer, holder])
}

/// 코드를 쥔 쪽의 증명임
fn hold_proof(k: &[u8; 32], nb: &[u8; 32], na: &[u8; 32], holder: &[u8; 32], enterer: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_CODE_PROOF, b"hold", k, nb, na, holder, enterer])
}

/// 두 nonce와 두 키로 장기 비밀 S를 유도함
fn derive_secret(k: &[u8; 32], holder: &[u8; 32], enterer: &[u8; 32], na: &[u8; 32], nb: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_SECRET, k, holder, enterer, na, nb])
}

/// 저장된 비밀 S를 아는지 보이는 증명임
fn secret_proof(s: &[u8; 32], prover: &[u8; 32], verifier: &[u8; 32], n_verifier: &[u8; 32], n_prover: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_PROOF, s, prover, verifier, n_verifier, n_prover])
}

/// 일찍 빠져나가지 않고 증명을 비교함 (시간 공격 방지)
fn same(a: &[u8; 32], b: Option<[u8; 32]>) -> bool {
    b.is_some_and(|b| a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0)
}

/// `key`(친구 링크 `token`이 우리용으로 쥔 코드) 입력을 시작함: 내 nonce를 보낼 메시지를 돌려줌
pub(crate) fn start_entering(st: &mut State, token: u64, key: [u8; 32]) -> Option<Control> {
    let nb = random32();
    st.conns.get_mut(&token)?.dev.entering = Some(Entering { key, nb, na: None });
    Some(Control::DeviceCode { nonce: to_hex(&nb) })
}

/// 내 연결된 컴퓨터로의 새 링크에서 저장된 비밀을 증명할 새 nonce를 만듦
pub(crate) fn link_nonce() -> [u8; 32] {
    random32()
}

/// `device_no` 거절 메시지를 만듦
fn no(reason: &str, left: Option<u32>) -> Control {
    Control::DeviceNo { reason: reason.to_string(), left }
}

/// 링크 `token`의 기기 상태를 빌려줌
fn dev(st: &mut State, token: u64) -> Option<&mut DevLink> {
    st.conns.get_mut(&token).map(|c| &mut c.dev)
}

/// `token` 링크에서 `id`와 코드가 맞았음(비밀 `s`): 사용자에게 묻고, 둘 다 확인할 때까지 `s`는 이 링크에만 둠
fn ask(shared: &Shared, st: &mut State, token: u64, id: &str, s: [u8; 32]) {
    st.link_code = None; // single use
    let until = Instant::now() + shared.timing.device_confirm_timeout;
    let Some(d) = dev(st, token) else { return };
    d.entering = None;
    d.holding = None;
    d.confirming = Some(Confirming { secret: s, mine: false, theirs: false, until });
    shared.emit(NetEvent::DeviceConfirm { id: id.to_string() });
}

/// `token` 링크에서 둘 다 확인함: `id`와 연결하고 그 링크를 증명됨으로 둠.
/// 그사이 다른 컴퓨터와 연결됐으면 `false`(아무것도 안 함)임
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

/// 친구 `id`에 대한 사용자 답을 처리함 (`Net::confirm_device` 참고).
/// 알릴 링크와 메시지를 돌려주고, 답을 기다리는 게 없으면 None임
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

/// 시간 안에 아무도 답하지 않은 확인을 취소함 (틱마다 불림)
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

/// 이미 `id` 아닌 다른 컴퓨터와 연결됐는지 봄
fn linked_elsewhere(st: &State, id: &str) -> bool {
    st.device.as_ref().is_some_and(|(d, _)| d != id)
}

/// 친구 링크 `token`에서 `id`가 보낸 `device_*` 메시지를 처리함
/// (인증·버전 확인·예산은 호출자가 함). 보낼 답들을 돌려줌
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

/// 이보다 큰 term은 말이 안 됨 (JSON 숫자는 2^53까지 정확함)
const MAX_TERM: u64 = 1 << 53;

/// `st`에서 놀기를 차지함(내 id `me`): 보낼 차지 메시지를 돌려줌.
/// `meet`이면 다시 만나 누가 놀지 정하는 차지임 (아무것도 안 넘김)
fn claim_in(st: &mut State, me: &str, meet: bool) -> Control {
    st.active = ((st.active.0 + 1).min(MAX_TERM), me.to_string());
    Control::DeviceActive { term: Some(st.active.0), you: false, idle: None, meet }
}

/// 📱 사용자가 이 컴퓨터에서 놂: 차지 메시지와 그걸 보낼 증명된 링크를 돌려줌.
/// 그런 링크가 없으면 None이지만 차지는 여기서 유효함
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

/// 내 연결된 컴퓨터로의 링크가 살아 있고 증명됐고 `device_active`를 알면 돌려줌
fn proven_device_link(st: &State) -> Option<Arc<SecureLink>> {
    let (d, _) = st.device.as_ref()?;
    st.friend_conn(d).filter(|c| matches!(c.kind, crate::Kind::Friend) && c.dev.proven && c.v >= crate::protocol::DEVICES_MIN_VERSION).map(|c| c.link.clone())
}

/// 지금 누가 노는지 정함 (연결된 컴퓨터가 `conn_timeout` 동안 안 닿으면 둘 다 놂).
/// `State::standby`·`device_standby`를 고치고 바뀌면 알림. 틱마다, 그리고 바뀔 때 바로 불려
/// 차지 직후에도 `Net::standby`가 맞음. 이 컴퓨터가 대기인지 돌려줌
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

/// 📱 틱마다 부름: 새 증명 링크마다 내 상태를 알리고, 누가 놀지 정하고, 바뀌면 알리고,
/// 대기 중이면 유예 뒤 친구 링크를 닫음. 연결된 컴퓨터가 `conn_timeout` 동안 안 닿으면 둘 다 놂
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

/// 📱 `device_state` 하나에 담는 펫 수 상한 (양방향) …
pub(crate) const MAX_CARE: usize = 64;
/// … 펫 id(폴더 이름: a-z, 0-9, _)와 단계 이름은 최대 이 길이임
const MAX_PET_ID: usize = 32;
/// 단계 이름 최대 길이
const MAX_STAGE: usize = 16;

/// 📱 고른 펫과 모든 펫의 돌봄임. 모양 검사를 거침 (`clean_state`)
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SharedState {
    /// 고른 펫 id
    pub pet: Option<String>,
    /// 펫별 돌봄 값
    pub care: Vec<WireCare>,
}

/// 소문자·숫자·`_`로만 된 `max`자 이하 낱말인지 봄
fn is_word(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// 펫처럼 보이는 것만 남김: id와 단계는 낱말, 숫자는 유한, 펫당 하나(첫 것), 최대 `MAX_CARE`개.
/// 종과 값 범위는 앱이 검사함
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

/// 내 펫이나 돌봄이 바뀜 (`Net::share_state`): 내가 노는 동안 내보냄
pub(crate) fn share(st: &mut State, pet: &str, care: Vec<WireCare>) {
    let s = clean_state(Some(pet.to_string()), care);
    if st.dstate.as_ref() != Some(&s) {
        st.dstate = Some(s);
        st.dstate_dirty = true;
        st.dstate_edited = true;
    }
}

/// 📱 연결된 컴퓨터에 보낼 내 상태가 있으면 돌려줌: 내가 놀고, 상대가 두 상태가 만난 증명 링크에서 대기 중이고,
/// 넘겨받기를 기다리지 않을 때. 그리고 링크나 내 차례가 새것이거나, 바뀌었고 지난 것이 `device_state_every` 전이거나,
/// `device_state_refresh`가 지났을 때임
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

/// 미확인 친구는 이만큼 날이 지나면 뺌
const PENDING_DAYS: u64 = 7;
/// … 또는 "너 모름"이라 말하는 인증된 hello가 이만큼 오면 뺌
pub(crate) const PENDING_STRIKES: u32 = 3;

/// 지금 유닉스 초를 돌려줌 (시계가 이상하면 0)
pub(crate) fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `new_install_id`가 만드는 모양의 설치 id인지 봄: 소문자 hex 32자리
pub(crate) fn is_install_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// 내 다른 컴퓨터나 친구 소개로 온 친구를 모든 면에서 새것일 때만 고정함. 더했는지 돌려줌
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

/// 친구 `from`이 보낸 `device_friends`를 처리함 (인증·예산·저장 비밀 증명을 거친 링크에서만)
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

/// 친구 `from`이 보낸 `my_devices`를 처리함 (인증·예산을 거친 것)
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

/// 친구 `gone`이 빠짐: 그가 소개한 기기(와 그들이 소개한 것)를 뺌.
/// 그들의 친구 링크를 돌려주고, 호출자가 상태 잠금을 푼 뒤 `unfriend`를 알리고 닫음
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

/// 미확인 친구 `id`(와 그가 소개한 것)를 잊음. `drop_introduced`처럼 `close_dropped`용 링크를 돌려줌
pub(crate) fn drop_pending(shared: &Shared, st: &mut State, id: &str) -> Vec<Arc<SecureLink>> {
    st.remove_friend(id);
    st.invites_out.remove(id);
    let mut links: Vec<_> = st.demote_friend_link(id).into_iter().collect();
    shared.emit(NetEvent::FriendDropped { id: id.to_string() });
    links.extend(drop_introduced(shared, st, id));
    links
}

/// `PENDING_DAYS` 동안 미확인인 친구를 뺌 (틱마다 불림)
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

/// 뺀 기기 링크에 더는 친구가 아님을 알리고 닫음
pub(crate) fn close_dropped(links: Vec<Arc<SecureLink>>) {
    for l in links {
        l.send(&crate::protocol::Message::Control(crate::protocol::Control::Unfriend));
        l.finish();
    }
}

/// 연결 코드와 설치 id 형식 시험임
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
