//! 사다리 게임임: commit–reveal 시드와 방장·참가자 상태 기계 (스펙 `2026-11-04-screenpet-ladder-design.md` §3–§6).
//!
//! 링크는 1:1이라 게임은 별 모양임: 방장이 모으고 전달함. 공정성: 모든 참가자(방장 포함)가 누구든 밝히기 전에
//! 비밀 nonce를 커밋하고, 시드는 모든 nonce와 잠긴 게임의 고정 바이너리 인코딩을 해시하므로 한 명만 정직해도
//! 아무도 결과를 미리 고를 수 없음. 참가자마다 다른 목록을 보인 방장은 이모지 4개 지문으로 들킴.
//! 고친 방장은 그래도 밝힌 값을 먼저 모아 결과를 보고 취소한 뒤 다시 할 수 있고, 취소의 `who`도 그의 주장일 뿐임.
//! 막을 수는 없고 드러낼 수만 있음: 게임이 취소됨.
//!
//! 기계는 I/O를 안 함: `Instant`를 받고 보낼 메시지를 돌려주므로 모두 Linux에서 시험함.
//! 와이어 문자열은 검사만 하고 다듬거나 자르지 않음 (모든 빌드가 같은 바이트를 해시해야 함).
//! 진입점: `HostGame`(방장), `GuestGame`(참가자). src-tauri의 `ladder.rs`가 부름.

use std::time::{Duration, Instant};

use blake2::{Blake2s256, Digest};

use crate::noise::{blake2s, hex32, hex_array, random32, to_hex};
use crate::protocol::{LadderMsg, WirePlayer, WireResult, MAX_NAME};

/// 한 판의 인원 (방장 포함). core `screenpet_core::ladder`와 같은 값이어야 함. net은 core에 의존하지 않아 따로 두고,
/// `tests/ladder.rs`가 둘이 같은지 확인함
pub const MIN_PLAYERS: usize = 2;
/// 한 판의 최대 인원 (방장 포함)
pub const MAX_PLAYERS: usize = 10;
/// 결과 칸 글자 수 상한 (core와 같음)
pub const RESULT_MAX_CHARS: usize = 12;
/// 와이어의 펫 id와 스타일은 짧은 ASCII 라벨임
const MAX_LABEL: usize = 32;
/// 이만큼 답이 없는 초대는 "아니오"로 침
pub const INVITE_TIMEOUT: Duration = Duration::from_secs(30);
/// 잠근 뒤 모든 nonce가 이 안에 와야 함
pub const REVEAL_TIMEOUT: Duration = Duration::from_secs(15);
/// 참가한 이가 끝내 잠그지 않는 방장을 포기하는 시간 (초대 창 + 편집 시간)
pub const LOCK_WAIT: Duration = Duration::from_secs(120);
/// 잠긴 참가자가 방장의 밝힌 값들을 기다리는 시간 (자기 마감 + 여유)
const REVEALS_WAIT: Duration = Duration::from_secs(20);

/// 해시마다 앞에 붙이는 용도 표시임 (같은 바이트가 다른 용도의 해시로 쓰이지 않게). 바꾸면 판끼리 결과가 달라짐
const COMMIT_TAG: &[u8] = b"screenpet-ladder-commit-v1";
/// 잠금 해시 용도 표시
const LOCK_TAG: &[u8] = b"screenpet-ladder-lock-v1";
/// 시드 해시 용도 표시
const SEED_TAG: &[u8] = b"screenpet-ladder-seed-v1";
/// 지문 해시 용도 표시
const PRINT_TAG: &[u8] = b"screenpet-ladder-print-v1";

// ---- crypto -------------------------------------------------------------------

/// 참가자 nonce의 커밋을 만듦 (게임 id와 토큰에 묶음)
pub fn commit(game_id: &[u8; 16], token: &[u8; 16], nonce: &[u8; 32]) -> [u8; 32] {
    blake2s(&[COMMIT_TAG, game_id, token, nonce])
}

/// 방장이 잠근 참가자임 (와이어 문자열을 받은 그대로 둠)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedPlayer {
    /// 게임별 참가자 토큰
    pub token: [u8; 16],
    /// nonce 커밋
    pub commit: [u8; 32],
    /// 표시 이름
    pub name: String,
    /// 펫 종류
    pub pet: String,
    /// "" = 둥근 그림 (와이어에서 빠짐)
    pub style: String,
}

/// 방장이 잠근 결과 칸임. `mood`는 와이어 라벨 그대로임 ("good" | "bad" | "none" | 더 새것은 none으로 보임)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedResult {
    /// 결과 글
    pub text: String,
    /// 분위기 라벨
    pub mood: String,
}

/// 길이(u32 LE)를 앞에 붙여 문자열 바이트를 버퍼에 씀
fn push_str(buf: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    buf.extend_from_slice(&(b.len() as u16).to_le_bytes());
    buf.extend_from_slice(b);
}

/// 잠긴 게임의 고정 바이너리 인코딩을 해시함 (JSON 바이트는 안 씀: 필드 순서·공백·숫자 모양이 빌드마다 다를 수 있음)
pub fn lock_hash(game_id: &[u8; 16], host: u8, players: &[LockedPlayer], results: &[LockedResult]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(64 + players.len() * 96 + results.len() * 24);
    buf.extend_from_slice(LOCK_TAG);
    buf.extend_from_slice(game_id);
    buf.push(players.len() as u8);
    buf.push(host);
    for p in players {
        buf.extend_from_slice(&p.token);
        buf.extend_from_slice(&p.commit);
        push_str(&mut buf, &p.name);
        push_str(&mut buf, &p.pet);
        push_str(&mut buf, &p.style);
    }
    for r in results {
        push_str(&mut buf, &r.text);
        push_str(&mut buf, &r.mood);
    }
    blake2s(&[&buf])
}

/// 잠금 해시와 모든 nonce(자리 순)로 시드를 만듦
pub fn seed(lock_hash: &[u8; 32], nonces: &[[u8; 32]]) -> [u8; 32] {
    let mut h = Blake2s256::new();
    h.update(SEED_TAG);
    h.update(lock_hash);
    for n in nonces {
        h.update(n);
    }
    h.finalize().into()
}

/// BLAKE2s(seed ‖ counter u32 LE) 블록에서 차례로 꺼내는 리틀 엔디언 u32 흐름임
pub struct Stream {
    seed: [u8; 32],
    counter: u32,
    block: [u8; 32],
    at: usize,
}

impl Stream {
    /// 시드에서 새 흐름을 만듦
    pub fn new(seed: [u8; 32]) -> Stream {
        Stream { seed, counter: 0, block: [0; 32], at: 32 }
    }

    /// 다음 u32를 꺼냄 (블록이 떨어지면 다음 블록을 만듦)
    pub fn next_u32(&mut self) -> u32 {
        if self.at == 32 {
            self.block = blake2s(&[&self.seed, &self.counter.to_le_bytes()]);
            self.counter = self.counter.wrapping_add(1);
            self.at = 0;
        }
        let v = u32::from_le_bytes(self.block[self.at..self.at + 4].try_into().expect("4 bytes"));
        self.at += 4;
        v
    }
}

/// BLAKE2s(tag ‖ seed)의 앞 24비트임: 6비트 이모지 번호 넷
pub fn print_bits(seed: &[u8; 32]) -> u32 {
    let d = blake2s(&[PRINT_TAG, seed]);
    (u32::from(d[0]) << 16) | (u32::from(d[1]) << 8) | u32::from(d[2])
}

// ---- validation ---------------------------------------------------------------------

/// 16바이트 hex를 읽음
fn hex16(s: &str) -> Option<[u8; 16]> {
    hex_array::<16>(s)
}

/// 16바이트 난수를 만듦
fn random16() -> [u8; 16] {
    random32()[..16].try_into().expect("16 bytes")
}

/// 방장이 보내야 하는 결과 글인지 봄: 비어 있지 않고, 앞뒤 공백·제어 문자 없고, 최대 12글자
pub fn valid_result_text(s: &str) -> bool {
    !s.is_empty() && s.trim() == s && !s.chars().any(char::is_control) && s.chars().count() <= RESULT_MAX_CHARS
}

/// 표시 이름이 올바른지 봄: 공백만이 아니고, 제어 문자 없고, `MAX_NAME`글자 이하
fn valid_name(s: &str) -> bool {
    !s.trim().is_empty() && !s.chars().any(char::is_control) && s.chars().count() <= MAX_NAME
}

/// 라벨이 영숫자·`_`·`-`로만 된 `MAX_LABEL`자 이하인지 봄
fn valid_label(s: &str) -> bool {
    s.len() <= MAX_LABEL && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

/// 결과 칸 목록이 1..=`MAX_PLAYERS`개이고 모두 올바른지 봄
fn valid_results(results: &[WireResult]) -> bool {
    (1..=MAX_PLAYERS).contains(&results.len()) && results.iter().all(|r| valid_result_text(&r.text) && valid_label(&r.mood))
}

/// 와이어 결과를 잠긴 결과로 바꿈
fn locked_results(results: &[WireResult]) -> Vec<LockedResult> {
    results.iter().map(|r| LockedResult { text: r.text.clone(), mood: r.mood.clone() }).collect()
}

/// 잠긴 결과를 와이어 결과로 바꿈
fn wire_results(results: &[LockedResult]) -> Vec<WireResult> {
    results.iter().map(|r| WireResult { text: r.text.clone(), mood: r.mood.clone() }).collect()
}

/// 빈 스타일은 와이어에서 뺌
fn wire_style(style: &str) -> Option<String> {
    (!style.is_empty()).then(|| style.to_string())
}

// ---- shared state ----------------------------------------------------------------------

/// 이 설치가 게임에 들고 오는 것임
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    /// 표시 이름
    pub name: String,
    /// 펫 종류
    pub pet: String,
    /// "" = 둥근 그림
    pub style: String,
}

/// 초대를 거절한 까닭임
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    /// 싫다고 함
    No,
    /// 바쁨
    Busy,
    /// 이미 다른 판 중임
    Playing,
}

impl DeclineReason {
    /// 와이어 라벨을 돌려줌
    pub fn wire(self) -> &'static str {
        match self {
            DeclineReason::No => "no",
            DeclineReason::Busy => "busy",
            DeclineReason::Playing => "playing",
        }
    }

    /// 와이어 라벨을 읽음 (모르면 `No`)
    fn from_wire(s: &str) -> DeclineReason {
        match s {
            "busy" => DeclineReason::Busy,
            "playing" => DeclineReason::Playing,
            _ => DeclineReason::No,
        }
    }
}

/// 방장이 보는 초대받은 이의 상태임
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// 답 기다림
    Waiting,
    /// 참가함
    Joined,
    /// 거절함
    Declined(DeclineReason),
    /// 시간 안에 답 없음
    TimedOut,
    /// 나감
    Left,
}

/// 게임이 취소된 까닭임
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    /// 방장이 게임을 멈춤 (또는 초대를 닫음)
    Host,
    /// 누군가 끊기거나 나감
    Left,
    /// 마감이 지남
    Timeout,
    /// 잘못됐거나 앞뒤가 안 맞는 메시지 (또는 커밋 확인 실패)
    Invalid,
    /// 방장 전용, 안 보냄: 초대받은 모두가 거절·시간 초과·나감
    Nobody,
}

impl CancelReason {
    /// 와이어 라벨을 돌려줌
    pub fn wire(self) -> &'static str {
        match self {
            CancelReason::Host => "host",
            CancelReason::Left => "left",
            CancelReason::Timeout => "timeout",
            CancelReason::Invalid => "invalid",
            CancelReason::Nobody => "nobody",
        }
    }

    /// 와이어 라벨을 읽음 (모르면 `Invalid`)
    fn from_wire(s: &str) -> CancelReason {
        match s {
            "host" => CancelReason::Host,
            "left" => CancelReason::Left,
            "timeout" => CancelReason::Timeout,
            "nobody" => CancelReason::Nobody,
            _ => CancelReason::Invalid,
        }
    }
}

/// 끝난 게임임: 같은 잠금이면 정직한 모든 화면에서 같음
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// 시드
    pub seed: [u8; 32],
    /// 이모지 지문 비트
    pub print: u32,
    /// `players` 안의 방장 자리
    pub host: usize,
    /// 잠긴 참가자들 (토큰 순)
    pub players: Vec<LockedPlayer>,
    /// 잠긴 결과 칸들
    pub results: Vec<LockedResult>,
    /// `players` 안의 이 설치 자리
    pub me: usize,
}

impl Outcome {
    /// 이 게임 시드의 u32 흐름을 새로 만듦
    pub fn stream(&self) -> Stream {
        Stream::new(self.seed)
    }
}

/// 게임 단계임
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// 방장: 초대를 보내고 답을 모음
    Inviting,
    /// 참가자: 초대를 보였고 아직 안 답함
    Invited,
    /// 참가자: 참가했고 잠금을 기다림
    Joined,
    /// 잠겼고 밝힌 값들을 기다림
    Locked,
    /// 끝남
    Done(Outcome),
    /// 취소됨. `who` = 관련된 참가자의 표시 이름 (있으면)
    Cancelled { reason: CancelReason, who: Option<String> },
}

impl Phase {
    /// 끝났거나 취소됐는지 봄
    pub fn is_over(&self) -> bool {
        matches!(self, Phase::Done(_) | Phase::Cancelled { .. })
    }
}

/// 잠금과 nonce들로 끝난 게임을 만듦
fn outcome(game_id: [u8; 16], host: usize, players: Vec<LockedPlayer>, results: Vec<LockedResult>, nonces: &[[u8; 32]], me: usize) -> Outcome {
    let l = lock_hash(&game_id, host as u8, &players, &results);
    let seed = seed(&l, nonces);
    Outcome { seed, print: print_bits(&seed), host, players, results, me }
}

// ---- host ---------------------------------------------------------------------------------

/// 방장이 보는 초대받은 친구 하나임
struct Invitee {
    id: String,
    answer: Answer,
    joined: Option<LockedPlayer>,
}

/// 방장이 잠근 뒤의 상태임
struct Locked {
    host: usize,
    players: Vec<LockedPlayer>,
    results: Vec<LockedResult>,
    /// 자리별 친구 id (None = 방장)
    ids: Vec<Option<String>>,
    /// 자리별 받은 nonce
    nonces: Vec<Option<[u8; 32]>>,
    since: Instant,
}

/// 한 게임의 방장 쪽임. 모든 메서드는 보낼 메시지를 `(친구 id, 메시지)`로 돌려줌
pub struct HostGame {
    game_id: [u8; 16],
    me: LockedPlayer,
    nonce: [u8; 32],
    invitees: Vec<Invitee>,
    since: Instant,
    locked: Option<Locked>,
    phase: Phase,
}

/// 방장이 보낼 메시지 목록: `(친구 id, 메시지)`
pub type Out = Vec<(String, LadderMsg)>;

impl HostGame {
    /// 게임을 시작함: `invitees`(서로 다른 친구 id 1..=9개)를 초대함. `draft` = 지금 계획한 결과 칸으로 초대에 보임.
    /// 최종 칸은 `start`에 줌 (몇 칸인지는 누가 참가하느냐로 정해짐). 인자가 범위 밖이면 None임
    pub fn new(me: Seat, draft: &[LockedResult], invitees: &[String], now: Instant) -> Option<(HostGame, Out)> {
        let mut ids = invitees.to_vec();
        ids.sort();
        ids.dedup();
        let draft = wire_results(draft);
        if ids.len() != invitees.len() || !(1..MAX_PLAYERS).contains(&ids.len()) || !valid_results(&draft) || !valid_seat(&me) {
            return None;
        }
        let game_id = random16();
        let token = random16();
        let nonce = random32();
        let me = LockedPlayer { token, commit: commit(&game_id, &token, &nonce), name: me.name, pet: me.pet, style: me.style };
        let invite = LadderMsg::LadderInvite { game_id: to_hex(&game_id), host: me.name.clone(), results: draft, invited: invitees.len() as u8 };
        let out = invitees.iter().map(|id| (id.clone(), invite.clone())).collect();
        let invitees = invitees.iter().map(|id| Invitee { id: id.clone(), answer: Answer::Waiting, joined: None }).collect();
        Some((HostGame { game_id, me, nonce, invitees, since: now, locked: None, phase: Phase::Inviting }, out))
    }

    /// 게임 id(hex)를 돌려줌
    pub fn game_id(&self) -> String {
        to_hex(&self.game_id)
    }

    /// 지금 단계를 돌려줌
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// 초대받은 이마다의 답을 돌려줌
    pub fn answers(&self) -> Vec<(String, Answer)> {
        self.invitees.iter().map(|i| (i.id.clone(), i.answer.clone())).collect()
    }

    /// 답의 종류별 수 (대기, 참가, 거절, 바쁨, 다른 판, 시간 초과, 나감)임. 틱마다 바뀌었는지 볼 때 씀 (복사 없음)
    pub fn answer_counts(&self) -> [u8; 7] {
        let mut c = [0u8; 7];
        for i in &self.invitees {
            let k = match i.answer {
                Answer::Waiting => 0,
                Answer::Joined => 1,
                Answer::Declined(DeclineReason::No) => 2,
                Answer::Declined(DeclineReason::Busy) => 3,
                Answer::Declined(DeclineReason::Playing) => 4,
                Answer::TimedOut => 5,
                Answer::Left => 6,
            };
            c[k] += 1;
        }
        c
    }

    /// 참가한 초대받은 이 수 (지금 잠글 수 있는 수)
    pub fn joined(&self) -> usize {
        self.invitees.iter().filter(|i| i.answer == Answer::Joined).count()
    }

    /// 모든 초대받은 이가 답했는지 봄
    pub fn all_answered(&self) -> bool {
        self.invitees.iter().all(|i| i.answer != Answer::Waiting)
    }

    /// 참가자 토큰 뒤의 친구 id를 돌려줌 (방장 화면은 친구 별명을 쓸 수 있음)
    pub fn friend_of(&self, token: &[u8; 16]) -> Option<String> {
        self.invitees.iter().find(|i| i.joined.as_ref().is_some_and(|p| &p.token == token)).map(|i| i.id.clone())
    }

    /// 친구 `from`이 보낸 사다리 메시지를 처리하고 보낼 것을 돌려줌
    pub fn on_message(&mut self, from: &str, msg: LadderMsg, now: Instant) -> Out {
        let gid = self.game_id();
        // 늦은 참가 (초대 30초가 지났거나, 이미 잠갔거나 끝난 판): 답이 없으면 그 친구는 "기다리는 중"에 갇힘 — 초대가 끝났다고 알림
        if let LadderMsg::LadderJoin { game_id, .. } = &msg {
            let late = *game_id == gid
                && self.invitees.iter().any(|i| i.id == from && !matches!(i.answer, Answer::Joined))
                && !(self.phase == Phase::Inviting && self.invitees.iter().any(|i| i.id == from && i.answer == Answer::Waiting));
            if late {
                return vec![(from.to_string(), self.cancel_msg(CancelReason::Host, None))];
            }
        }
        if self.phase.is_over() {
            return Vec::new();
        }
        let out = self.handle(from, msg, &gid, now);
        self.check_nobody();
        out
    }

    /// 모두 거절했거나 답이 없거나 나갔음(참가 0명): 기다리는 화면에 갇히지 않게 판을 끝냄
    fn check_nobody(&mut self) {
        if self.phase == Phase::Inviting && self.all_answered() && self.joined() == 0 {
            self.phase = Phase::Cancelled { reason: CancelReason::Nobody, who: None };
        }
    }

    /// 이 게임 메시지를 종류별로 나눠 처리함
    fn handle(&mut self, from: &str, msg: LadderMsg, gid: &str, now: Instant) -> Out {
        let gid = gid.to_string();
        match msg {
            LadderMsg::LadderJoin { game_id, token, commit, name, pet, style } if game_id == gid && self.phase == Phase::Inviting => {
                let Some(inv) = self.invitees.iter().position(|i| i.id == from && i.answer == Answer::Waiting) else { return Vec::new() };
                let style = style.unwrap_or_default();
                let parsed = hex16(&token).zip(hex32(&commit));
                let Some((token, commit)) = parsed.filter(|_| valid_name(&name) && valid_label(&pet) && valid_label(&style)) else {
                    self.invitees[inv].answer = Answer::Left;
                    return vec![(from.to_string(), self.cancel_msg(CancelReason::Invalid, None))];
                };
                let taken = token == self.me.token || self.invitees.iter().any(|i| i.joined.as_ref().is_some_and(|p| p.token == token));
                if taken {
                    self.invitees[inv].answer = Answer::Left;
                    return vec![(from.to_string(), self.cancel_msg(CancelReason::Invalid, None))];
                }
                self.invitees[inv].answer = Answer::Joined;
                self.invitees[inv].joined = Some(LockedPlayer { token, commit, name, pet, style });
                Vec::new()
            }
            LadderMsg::LadderDecline { game_id, reason } if game_id == gid && self.phase == Phase::Inviting => {
                if let Some(i) = self.invitees.iter_mut().find(|i| i.id == from && i.answer == Answer::Waiting) {
                    i.answer = Answer::Declined(DeclineReason::from_wire(&reason));
                }
                Vec::new()
            }
            LadderMsg::LadderReveal { game_id, nonce } if game_id == gid => self.on_reveal(from, &nonce),
            // 참가자가 잠그기·공개를 거부했음 (내 커밋이 바뀜 등) — 끊김이 아니라 "정보가 맞지 않음"으로 모두에게 알림
            LadderMsg::LadderCancel { game_id, reason, .. } if game_id == gid && reason == CancelReason::Invalid.wire() && self.phase == Phase::Locked => {
                let who = self.locked.as_ref().and_then(|l| l.ids.iter().position(|i| i.as_deref() == Some(from)).map(|seat| l.players[seat].token));
                match who {
                    Some(t) => self.cancel(CancelReason::Invalid, Some(t)),
                    None => Vec::new(),
                }
            }
            LadderMsg::LadderCancel { game_id, .. } if game_id == gid => self.player_left(from, now),
            _ => Vec::new(),
        }
    }

    /// 참가자의 밝힌 nonce를 커밋과 맞춰 보고, 다 모이면 모두에게 돌리고 끝냄
    fn on_reveal(&mut self, from: &str, nonce: &str) -> Out {
        let Some(l) = self.locked.as_mut() else { return Vec::new() };
        let Some(seat) = l.ids.iter().position(|i| i.as_deref() == Some(from)) else { return Vec::new() };
        if l.nonces[seat].is_some() {
            return Vec::new();
        }
        let player = &l.players[seat];
        match hex32(nonce).filter(|n| commit(&self.game_id, &player.token, n) == player.commit) {
            Some(n) => l.nonces[seat] = Some(n),
            None => {
                let who = Some(player.token);
                return self.cancel(CancelReason::Invalid, who);
            }
        }
        if l.nonces.iter().any(Option::is_none) {
            return Vec::new();
        }
        let nonces: Vec<[u8; 32]> = l.nonces.iter().map(|n| n.expect("all revealed")).collect();
        let msg = LadderMsg::LadderReveals { game_id: to_hex(&self.game_id), nonces: nonces.iter().map(|n| to_hex(n)).collect() };
        let out = l.ids.iter().flatten().map(|id| (id.clone(), msg.clone())).collect();
        let done = outcome(self.game_id, l.host, l.players.clone(), l.results.clone(), &nonces, l.host);
        self.phase = Phase::Done(done);
        out
    }

    /// 참가자가 나감: 잠근 뒤면 게임을 취소하고, 전이면 그 답만 나감으로 둠
    fn player_left(&mut self, id: &str, _now: Instant) -> Out {
        if let Some(l) = &self.locked {
            if self.phase == Phase::Locked {
                if let Some(seat) = l.ids.iter().position(|i| i.as_deref() == Some(id)) {
                    let who = Some(l.players[seat].token);
                    return self.cancel(CancelReason::Left, who);
                }
            }
            return Vec::new();
        }
        if let Some(i) = self.invitees.iter_mut().find(|i| i.id == id && matches!(i.answer, Answer::Waiting | Answer::Joined)) {
            i.answer = Answer::Left;
            i.joined = None;
        }
        Vec::new()
    }

    /// 친구 링크가 끊김
    pub fn on_disconnected(&mut self, id: &str, now: Instant) -> Out {
        if self.phase.is_over() {
            return Vec::new();
        }
        let out = self.player_left(id, now);
        self.check_nobody();
        out
    }

    /// 최종 결과 칸(참가자 + 방장마다 하나)으로 게임을 잠금. 아직 기다리는 초대받은 이에게는 초대가 닫혔다고 알림.
    /// 잠글 수 없으면(단계가 틀림, 참가 0명, 칸 수·글이 틀림) None임
    pub fn start(&mut self, results: &[LockedResult], now: Instant) -> Option<Out> {
        let wire = wire_results(results);
        let joined: Vec<(String, LockedPlayer)> =
            self.invitees.iter().filter(|i| i.answer == Answer::Joined).filter_map(|i| Some((i.id.clone(), i.joined.clone()?))).collect();
        if self.phase != Phase::Inviting || joined.is_empty() || wire.len() != joined.len() + 1 || !valid_results(&wire) {
            return None;
        }
        let mut seats: Vec<(Option<String>, LockedPlayer)> = joined.into_iter().map(|(id, p)| (Some(id), p)).collect();
        seats.push((None, self.me.clone()));
        seats.sort_by_key(|s| s.1.token);
        let host = seats.iter().position(|s| s.0.is_none()).expect("host seated");
        let ids: Vec<Option<String>> = seats.iter().map(|s| s.0.clone()).collect();
        let players: Vec<LockedPlayer> = seats.into_iter().map(|s| s.1).collect();
        let mut nonces = vec![None; players.len()];
        nonces[host] = Some(self.nonce);
        let lock = LadderMsg::LadderLock {
            game_id: self.game_id(),
            host: host as u8,
            players: players
                .iter()
                .map(|p| WirePlayer { token: to_hex(&p.token), name: p.name.clone(), pet: p.pet.clone(), style: wire_style(&p.style), commit: to_hex(&p.commit) })
                .collect(),
            results: wire,
        };
        let mut out: Out = ids.iter().flatten().map(|id| (id.clone(), lock.clone())).collect();
        let closed = self.cancel_msg(CancelReason::Host, None);
        for i in self.invitees.iter_mut().filter(|i| i.answer == Answer::Waiting) {
            i.answer = Answer::TimedOut;
            out.push((i.id.clone(), closed.clone()));
        }
        self.locked = Some(Locked { host, players, results: results.to_vec(), ids, nonces, since: now });
        self.phase = Phase::Locked;
        Some(out)
    }

    /// 방장이 포기함 (창 닫음, 앱 끝남)
    pub fn abort(&mut self) -> Out {
        if self.phase.is_over() {
            return Vec::new();
        }
        self.cancel(CancelReason::Host, None)
    }

    /// 마감을 봄: 30초 지난 답 없는 초대, 잠근 뒤 15초 지나도 안 온 밝힌 값
    pub fn tick(&mut self, now: Instant) -> Out {
        match &self.phase {
            Phase::Inviting if now.duration_since(self.since) >= INVITE_TIMEOUT => {
                for i in self.invitees.iter_mut().filter(|i| i.answer == Answer::Waiting) {
                    i.answer = Answer::TimedOut;
                }
                self.check_nobody();
                Vec::new()
            }
            Phase::Locked => {
                let l = self.locked.as_ref().expect("locked");
                if now.duration_since(l.since) < REVEAL_TIMEOUT {
                    return Vec::new();
                }
                let who = l.nonces.iter().position(Option::is_none).map(|seat| l.players[seat].token);
                self.cancel(CancelReason::Timeout, who)
            }
            _ => Vec::new(),
        }
    }

    /// 취소 메시지를 만듦
    fn cancel_msg(&self, reason: CancelReason, who: Option<[u8; 16]>) -> LadderMsg {
        LadderMsg::LadderCancel { game_id: self.game_id(), reason: reason.wire().to_string(), who: who.map(|t| to_hex(&t)) }
    }

    /// 아직 게임에 있는 모두에게 게임을 끝냄
    fn cancel(&mut self, reason: CancelReason, who: Option<[u8; 16]>) -> Out {
        let msg = self.cancel_msg(reason, who);
        let who_name = who.and_then(|t| self.name_of(&t));
        let to: Vec<String> = match &self.locked {
            Some(l) => l.ids.iter().flatten().cloned().collect(),
            None => self.invitees.iter().filter(|i| matches!(i.answer, Answer::Waiting | Answer::Joined)).map(|i| i.id.clone()).collect(),
        };
        self.phase = Phase::Cancelled { reason, who: who_name };
        to.into_iter().map(|id| (id, msg.clone())).collect()
    }

    /// 토큰의 표시 이름을 돌려줌
    fn name_of(&self, token: &[u8; 16]) -> Option<String> {
        if let Some(l) = &self.locked {
            return l.players.iter().find(|p| &p.token == token).map(|p| p.name.clone());
        }
        self.invitees.iter().filter_map(|i| i.joined.as_ref()).find(|p| &p.token == token).map(|p| p.name.clone())
    }
}

/// 자리 정보(이름, 펫, 스타일)가 올바른지 봄
fn valid_seat(me: &Seat) -> bool {
    valid_name(&me.name) && valid_label(&me.pet) && valid_label(&me.style)
}

// ---- player ---------------------------------------------------------------------------------

/// 참가자 자신의 토큰·nonce·커밋임
struct Mine {
    token: [u8; 16],
    nonce: [u8; 32],
    commit: [u8; 32],
}

/// 한 게임의 참가자 쪽임 (방장 메시지만 받음, 앱이 길을 찾아 줌)
pub struct GuestGame {
    host_id: String,
    game_id: [u8; 16],
    host_name: String,
    draft: Vec<LockedResult>,
    invited: u8,
    mine: Option<Mine>,
    locked: Option<(usize, Vec<LockedPlayer>, Vec<LockedResult>)>,
    since: Instant,
    phase: Phase,
}

impl GuestGame {
    /// 친구 `from`의 올바른 초대면 게임을 만듦. 아니면 None임 (무시할 것)
    pub fn on_invite(from: &str, msg: LadderMsg, now: Instant) -> Option<GuestGame> {
        let LadderMsg::LadderInvite { game_id, host, results, invited } = msg else { return None };
        let game_id = hex16(&game_id)?;
        if !valid_name(&host) || !valid_results(&results) || !(1..MAX_PLAYERS as u8).contains(&invited) {
            return None;
        }
        Some(GuestGame {
            host_id: from.to_string(),
            game_id,
            host_name: host,
            draft: locked_results(&results),
            invited,
            mine: None,
            locked: None,
            since: now,
            phase: Phase::Invited,
        })
    }

    /// 방장 친구 id를 돌려줌
    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    /// 방장 표시 이름을 돌려줌
    pub fn host_name(&self) -> &str {
        &self.host_name
    }

    /// 게임 id(hex)를 돌려줌
    pub fn game_id(&self) -> String {
        to_hex(&self.game_id)
    }

    /// 초대에 보인 결과 칸을 돌려줌 (최종 칸은 잠금과 함께 옴)
    pub fn draft(&self) -> &[LockedResult] {
        &self.draft
    }

    /// 방장이 초대한 친구 수를 돌려줌 (방장 빼고)
    pub fn invited(&self) -> u8 {
        self.invited
    }

    /// 지금 단계를 돌려줌
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// 참가하기 전에 초대가 끝났는지 봄 (방장이 닫음, 방장 떠남, 시간 초과): 알릴 것 없이 초대만 사라짐
    pub fn ended_unanswered(&self) -> bool {
        self.phase.is_over() && self.mine.is_none()
    }

    /// 새 토큰과 커밋한 nonce로 참가함. 이미 답했으면 None임
    pub fn join(&mut self, me: Seat, now: Instant) -> Option<LadderMsg> {
        if self.phase != Phase::Invited || !valid_seat(&me) {
            return None;
        }
        let token = random16();
        let nonce = random32();
        let c = commit(&self.game_id, &token, &nonce);
        self.mine = Some(Mine { token, nonce, commit: c });
        self.phase = Phase::Joined;
        self.since = now;
        Some(LadderMsg::LadderJoin { game_id: self.game_id(), token: to_hex(&token), commit: to_hex(&c), name: me.name, pet: me.pet, style: wire_style(&me.style) })
    }

    /// 거절함 (또는 바쁨·이미 다른 판). 이미 답했으면 None임
    pub fn decline(&mut self, reason: DeclineReason) -> Option<LadderMsg> {
        if self.phase != Phase::Invited {
            return None;
        }
        self.phase = Phase::Cancelled { reason: CancelReason::Host, who: None };
        Some(LadderMsg::LadderDecline { game_id: self.game_id(), reason: reason.wire().to_string() })
    }

    /// 참가한 게임에서 나감 (창 닫음, 앱 끝남): 방장에게 알림
    pub fn quit(&mut self) -> Option<LadderMsg> {
        match self.phase {
            Phase::Invited => self.decline(DeclineReason::No),
            Phase::Joined | Phase::Locked => {
                self.phase = Phase::Cancelled { reason: CancelReason::Left, who: None };
                Some(LadderMsg::LadderCancel { game_id: self.game_id(), reason: CancelReason::Left.wire().to_string(), who: None })
            }
            _ => None,
        }
    }

    /// 이 게임의 올바른 잠금의 최종 칸이, 초대 칸에서 몇 개만 뺀 것이 아니면 돌려줌
    /// (인원이 줄어드는 건 보통이고, 다른 글·분위기·더 많은 사본은 참가한 이유가 아님).
    /// 앱은 이런 잠금을 우리 nonce를 밝히는 `on_message`에 넘기기 전에 참가자에게 물음
    pub fn lock_changed_results(&self, msg: &LadderMsg) -> Option<Vec<LockedResult>> {
        let LadderMsg::LadderLock { game_id, host, players, results } = msg else { return None };
        if *game_id != self.game_id() || self.phase != Phase::Joined {
            return None;
        }
        let (_, _, results) = self.check_lock(*host, players, results)?;
        let mut left = self.draft.clone();
        let shrunk = results.iter().all(|r| left.iter().position(|d| d == r).map(|i| left.swap_remove(i)).is_some());
        (!shrunk).then_some(results)
    }

    /// 참가자가 칸이 바뀐 잠금을 거절함: 밝히지 않고 게임은 취소됨
    pub fn refuse_lock(&mut self) -> Option<LadderMsg> {
        (self.phase == Phase::Joined).then(|| self.invalid()).flatten()
    }

    /// 방장 메시지를 처리함. 보낼 답(잠금 뒤의 우리 밝힌 값)을 돌려줌
    pub fn on_message(&mut self, msg: LadderMsg, now: Instant) -> Option<LadderMsg> {
        if self.phase.is_over() {
            return None;
        }
        let gid = self.game_id();
        match msg {
            LadderMsg::LadderLock { game_id, host, players, results } if game_id == gid && self.phase == Phase::Joined => {
                match self.check_lock(host, &players, &results) {
                    Some(locked) => {
                        self.locked = Some(locked);
                        self.phase = Phase::Locked;
                        self.since = now;
                        let mine = self.mine.as_ref().expect("joined");
                        Some(LadderMsg::LadderReveal { game_id: gid, nonce: to_hex(&mine.nonce) })
                    }
                    None => self.invalid(),
                }
            }
            LadderMsg::LadderReveals { game_id, nonces } if game_id == gid && self.phase == Phase::Locked => {
                let (host, players, results) = self.locked.clone().expect("locked");
                let mine = self.mine.as_ref().expect("joined");
                let parsed: Option<Vec<[u8; 32]>> = nonces.iter().map(|n| hex32(n)).collect();
                let ok = parsed.filter(|n| {
                    n.len() == players.len() && n.iter().zip(&players).all(|(n, p)| commit(&self.game_id, &p.token, n) == p.commit) && n.contains(&mine.nonce)
                });
                let Some(nonces) = ok else { return self.invalid() };
                let me = players.iter().position(|p| p.token == mine.token).expect("checked in the lock");
                self.phase = Phase::Done(outcome(self.game_id, host, players, results, &nonces, me));
                None
            }
            LadderMsg::LadderCancel { game_id, reason, who } if game_id == gid => {
                let who = who.as_deref().and_then(hex16).and_then(|t| self.name_of(&t));
                self.phase = Phase::Cancelled { reason: CancelReason::from_wire(&reason), who };
                None
            }
            _ => None,
        }
    }

    /// 잘못된 메시지로 게임을 취소함 (방장에게 알림)
    fn invalid(&mut self) -> Option<LadderMsg> {
        self.phase = Phase::Cancelled { reason: CancelReason::Invalid, who: None };
        Some(LadderMsg::LadderCancel { game_id: self.game_id(), reason: CancelReason::Invalid.wire().to_string(), who: None })
    }

    /// 잠금은 우리 토큰과 커밋을 그대로, 서로 다른 토큰을 오름차순으로, 2..=10명, 참가자마다 결과 하나를 담아야 함
    fn check_lock(&self, host: u8, players: &[WirePlayer], results: &[WireResult]) -> Option<(usize, Vec<LockedPlayer>, Vec<LockedResult>)> {
        let mine = self.mine.as_ref()?;
        let n = players.len();
        if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) || results.len() != n || host as usize >= n || !valid_results(results) {
            return None;
        }
        let mut locked = Vec::with_capacity(n);
        for p in players {
            let style = p.style.clone().unwrap_or_default();
            if !valid_name(&p.name) || !valid_label(&p.pet) || !valid_label(&style) {
                return None;
            }
            locked.push(LockedPlayer { token: hex16(&p.token)?, commit: hex32(&p.commit)?, name: p.name.clone(), pet: p.pet.clone(), style });
        }
        if !locked.windows(2).all(|w| w[0].token < w[1].token) {
            return None; // unsorted or duplicate tokens
        }
        let me = locked.iter().position(|p| p.token == mine.token)?;
        if locked[me].commit != mine.commit || me == host as usize {
            return None;
        }
        Some((host as usize, locked, locked_results(results)))
    }

    /// 토큰의 표시 이름을 돌려줌
    fn name_of(&self, token: &[u8; 16]) -> Option<String> {
        self.locked.as_ref().and_then(|(_, players, _)| players.iter().find(|p| &p.token == token).map(|p| p.name.clone()))
    }

    /// 방장 링크가 끊김: 이미 끝나지 않았으면 끝남
    pub fn on_host_disconnected(&mut self) {
        if !self.phase.is_over() {
            let who = Some(self.host_name.clone());
            self.phase = Phase::Cancelled { reason: CancelReason::Left, who };
        }
    }

    /// 마감을 봄. 포기할 때 방장에게 보낼 메시지를 돌려줌: 답 없는 초대(자동 "아니오")나 끝내 안 온 잠금("나감")
    pub fn tick(&mut self, now: Instant) -> Option<LadderMsg> {
        let waited = now.duration_since(self.since);
        match self.phase {
            Phase::Invited if waited >= INVITE_TIMEOUT => {
                let m = self.decline(DeclineReason::No);
                self.phase = Phase::Cancelled { reason: CancelReason::Timeout, who: None };
                m
            }
            // 방장에게도 알림 — 그래야 방장이 나간 사람을 빼고 잠금 (안 알리면 잠근 뒤 공개 15초가 지나 판 전체가 취소됨)
            Phase::Joined if waited >= LOCK_WAIT => {
                self.phase = Phase::Cancelled { reason: CancelReason::Timeout, who: Some(self.host_name.clone()) };
                Some(LadderMsg::LadderCancel { game_id: self.game_id(), reason: CancelReason::Left.wire().to_string(), who: None })
            }
            Phase::Locked if waited >= REVEALS_WAIT => {
                self.phase = Phase::Cancelled { reason: CancelReason::Timeout, who: Some(self.host_name.clone()) };
                None
            }
            _ => None,
        }
    }
}

/// 방장·참가자 상태 기계와 고정 벡터 시험임
#[cfg(test)]
mod tests {
    use super::*;

    fn seat(name: &str) -> Seat {
        Seat { name: name.into(), pet: "cat".into(), style: String::new() }
    }

    fn results(texts: &[(&str, &str)]) -> Vec<LockedResult> {
        texts.iter().map(|(t, m)| LockedResult { text: t.to_string(), mood: m.to_string() }).collect()
    }

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("friend{i}")).collect()
    }

    /// 손으로 이은 방장과 참가자 `n`명: 모든 메시지가 바로 상대에게 감
    struct Game {
        host: HostGame,
        guests: Vec<(String, GuestGame)>,
        now: Instant,
    }

    impl Game {
        fn invite(n: usize) -> Game {
            let now = Instant::now();
            let draft = results(&[("당첨", "good"), ("꽝", "bad")]);
            let (host, out) = HostGame::new(seat("방장"), &draft, &ids(n), now).expect("valid game");
            let guests = out.into_iter().map(|(id, m)| (id.clone(), GuestGame::on_invite("host", m, now).expect("valid invite"))).collect();
            Game { host, guests, now }
        }

        fn deliver_to_host(&mut self, from: &str, m: LadderMsg) {
            let out = self.host.on_message(from, m, self.now);
            self.route(out);
        }

        fn route(&mut self, out: Out) {
            for (to, m) in out {
                let now = self.now;
                let reply = self.guests.iter_mut().find(|(id, _)| *id == to).and_then(|(_, g)| g.on_message(m, now));
                if let Some(r) = reply {
                    self.deliver_to_host(&to, r);
                }
            }
        }

        fn join_one(&mut self, i: usize, name: &str) {
            let id = self.guests[i].0.clone();
            let m = self.guests[i].1.join(seat(name), self.now).expect("join");
            self.deliver_to_host(&id, m);
        }

        fn decline_one(&mut self, i: usize, reason: DeclineReason) {
            let id = self.guests[i].0.clone();
            let m = self.guests[i].1.decline(reason).expect("decline");
            self.deliver_to_host(&id, m);
        }

        fn join_all(&mut self) {
            for i in 0..self.guests.len() {
                self.join_one(i, &format!("손님{i}"));
            }
        }

        fn lock(&mut self) {
            let n = self.host.joined() + 1;
            let r: Vec<LockedResult> = (0..n).map(|i| LockedResult { text: format!("칸{i}"), mood: "none".into() }).collect();
            let out = self.host.start(&r, self.now).expect("lockable");
            self.route(out);
        }
    }

    fn done(p: &Phase) -> &Outcome {
        match p {
            Phase::Done(o) => o,
            other => panic!("not done: {other:?}"),
        }
    }

    #[test]
    fn honest_players_all_reach_the_same_outcome() {
        for n in 1..MAX_PLAYERS {
            let mut g = Game::invite(n);
            g.join_all();
            g.lock();
            let host = done(g.host.phase()).clone();
            assert_eq!(host.players.len(), n + 1);
            for (_, guest) in &g.guests {
                let o = done(guest.phase());
                assert_eq!((o.seed, o.print, &o.players, &o.results, o.host), (host.seed, host.print, &host.players, &host.results, host.host));
                assert!(o.players[o.me].name.starts_with("손님"));
            }
            assert_eq!(host.players[host.me].name, "방장");
            assert!(host.players.windows(2).all(|w| w[0].token < w[1].token));
        }
    }

    #[test]
    fn declines_and_timeouts_leave_the_rest_playing() {
        let mut g = Game::invite(3);
        g.decline_one(0, DeclineReason::Busy);
        g.join_one(1, "둘");
        assert_eq!(g.host.answers()[0].1, Answer::Declined(DeclineReason::Busy));
        assert!(!g.host.all_answered());
        // 셋째는 답이 없음 — 30초 뒤 불참
        g.now += INVITE_TIMEOUT;
        g.host.tick(g.now);
        assert_eq!(g.host.answers()[2].1, Answer::TimedOut);
        assert!(g.host.all_answered());
        // 결과 칸 수는 참가자 수(방장 + 1)와 같아야 잠김
        assert!(g.host.start(&results(&[("a", "good"), ("b", "bad"), ("c", "none")]), g.now).is_none());
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad")]), g.now).expect("two players");
        g.route(out);
        assert_eq!(done(g.host.phase()).players.len(), 2);
        assert_eq!(done(g.host.phase()).seed, done(g.guests[1].1.phase()).seed);
    }

    #[test]
    fn waiting_invitees_are_told_the_invitation_closed() {
        let mut g = Game::invite(2);
        g.join_one(0, "일");
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad")]), g.now).unwrap();
        assert!(out.iter().any(|(to, m)| to == "friend1" && matches!(m, LadderMsg::LadderCancel { reason, .. } if reason == "host")));
        g.route(out);
        assert!(matches!(g.guests[1].1.phase(), Phase::Cancelled { reason: CancelReason::Host, .. }));
    }

    #[test]
    fn nobody_joined_cannot_lock() {
        let mut g = Game::invite(2);
        assert!(g.host.start(&results(&[("a", "good")]), g.now).is_none());
    }

    #[test]
    fn a_forged_reveal_cancels_for_everyone() {
        let mut g = Game::invite(2);
        g.join_all();
        let n = g.host.joined() + 1;
        let r: Vec<LockedResult> = (0..n).map(|i| LockedResult { text: format!("칸{i}"), mood: "none".into() }).collect();
        g.host.start(&r, g.now).unwrap();
        // 첫 손님 대신 다른 난수를 공개함
        let gid = g.host.game_id();
        let out = g.host.on_message("friend0", LadderMsg::LadderReveal { game_id: gid, nonce: to_hex(&random32()) }, g.now);
        assert!(matches!(g.host.phase(), Phase::Cancelled { reason: CancelReason::Invalid, who: Some(name) } if name == "손님0"));
        assert_eq!(out.len(), 2, "everyone locked in is told");
        assert!(out.iter().all(|(_, m)| matches!(m, LadderMsg::LadderCancel { reason, who: Some(_), .. } if reason == "invalid")));
    }

    #[test]
    fn a_player_rejects_a_lock_that_changed_its_commitment() {
        let mut g = Game::invite(1);
        g.join_one(0, "일");
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad")]), g.now).unwrap();
        let (_, LadderMsg::LadderLock { game_id, host, mut players, results }) = out.into_iter().next().unwrap() else { panic!("lock") };
        let mine = players.iter_mut().find(|p| p.name == "일").unwrap();
        mine.commit = to_hex(&random32());
        let reply = g.guests[0].1.on_message(LadderMsg::LadderLock { game_id, host, players, results }, g.now);
        assert!(matches!(reply, Some(LadderMsg::LadderCancel { reason, .. }) if reason == "invalid"));
        assert!(matches!(g.guests[0].1.phase(), Phase::Cancelled { reason: CancelReason::Invalid, .. }));
    }

    fn locked_lock(g: &mut Game) -> LadderMsg {
        g.join_one(0, "일");
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad")]), g.now).unwrap();
        out.into_iter().next().unwrap().1
    }

    #[test]
    fn a_player_rejects_malformed_locks() {
        type Tamper = fn(&mut Vec<WirePlayer>, &mut Vec<WireResult>, &mut u8);
        let tamper: [Tamper; 6] = [
            |p, _, _| p.reverse(),                              // 토큰 순서가 아님
            |p, _, _| p[1].token = p[0].token.clone(),           // 토큰 중복
            |_, r, _| r.push(r[0].clone()),                      // 결과 칸 수 ≠ 인원
            |_, r, _| r[0].text = " 공백".into(),                // 앞뒤 공백 (다듬지 않는다)
            |_, _, h| *h = 9,                                    // 방장 자리가 범위 밖
            |p, _, _| p[0].name = "줄\n바꿈".into(),             // 제어 문자
        ];
        for (i, t) in tamper.iter().enumerate() {
            let mut g = Game::invite(1);
            let LadderMsg::LadderLock { game_id, mut host, mut players, mut results } = locked_lock(&mut g) else { panic!("lock") };
            t(&mut players, &mut results, &mut host);
            let reply = g.guests[0].1.on_message(LadderMsg::LadderLock { game_id, host, players, results }, g.now);
            assert!(matches!(reply, Some(LadderMsg::LadderCancel { .. })), "tamper {i} accepted");
        }
    }

    #[test]
    fn a_player_rejects_reveals_that_do_not_open_the_commitments() {
        // 공개 목록은 자리 순서대로 약속을 열어야 함: 한 칸을 바꿈 · 하나 빠뜨림 · 전부 바꿈
        type Tamper = fn(Vec<String>) -> Vec<String>;
        let tamper: [Tamper; 3] = [
            |n| vec![n[0].clone(), to_hex(&random32())],
            |n| vec![n[0].clone()],
            |_| vec![to_hex(&random32()), to_hex(&random32())],
        ];
        for (i, t) in tamper.iter().enumerate() {
            let mut g = Game::invite(1);
            let lock = locked_lock(&mut g);
            let LadderMsg::LadderReveal { nonce, game_id } = g.guests[0].1.on_message(lock, g.now).expect("reveal") else { panic!("reveal") };
            let reply = g.guests[0].1.on_message(LadderMsg::LadderReveals { game_id, nonces: t(vec![nonce]) }, g.now);
            assert!(matches!(reply, Some(LadderMsg::LadderCancel { .. })), "tamper {i} accepted");
            assert!(matches!(g.guests[0].1.phase(), Phase::Cancelled { reason: CancelReason::Invalid, .. }), "tamper {i}: {:?}", g.guests[0].1.phase());
        }
    }

    #[test]
    fn a_host_showing_different_lists_is_caught_by_the_print() {
        // 방장이 두 손님에게 결과 칸 글자가 다른 잠그기를 보냄 — 각자는 끝나지만 지문이 달라짐
        let mut g = Game::invite(2);
        g.join_all();
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad"), ("c", "none")]), g.now).unwrap();
        let mut replies = Vec::new();
        for (to, m) in out {
            let LadderMsg::LadderLock { game_id, host, players, mut results } = m else { continue };
            if to == "friend1" {
                results[0].text = "z".into();
            }
            let now = g.now;
            let guest = &mut g.guests.iter_mut().find(|(id, _)| *id == to).unwrap().1;
            replies.push((to.clone(), guest.on_message(LadderMsg::LadderLock { game_id, host, players, results }, now).unwrap()));
        }
        for (from, r) in replies {
            g.deliver_to_host(&from, r);
        }
        let a = done(g.guests[0].1.phase()).print;
        let b = done(g.guests[1].1.phase()).print;
        assert_ne!(a, b);
        assert_eq!(done(g.host.phase()).print, a);
    }

    #[test]
    fn missing_reveals_time_out_with_a_name() {
        let mut g = Game::invite(2);
        g.join_all();
        // 손님1의 공개만 보내지 않음 (잠그기를 직접 나눠 줌)
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad"), ("c", "none")]), g.now).unwrap();
        for (to, m) in out {
            if to == "friend0" {
                let now = g.now;
                let r = g.guests[0].1.on_message(m, now).unwrap();
                g.deliver_to_host("friend0", r);
            }
        }
        assert_eq!(*g.host.phase(), Phase::Locked);
        g.now += REVEAL_TIMEOUT - Duration::from_millis(1);
        assert!(g.host.tick(g.now).is_empty());
        g.now += Duration::from_millis(1);
        let out = g.host.tick(g.now);
        assert!(matches!(g.host.phase(), Phase::Cancelled { reason: CancelReason::Timeout, who: Some(n) } if n == "손님1"));
        assert_eq!(out.len(), 2);
        g.route(out);
        assert!(matches!(g.guests[0].1.phase(), Phase::Cancelled { reason: CancelReason::Timeout, who: Some(n) } if n == "손님1"));
    }

    #[test]
    fn disconnects_before_and_after_the_lock() {
        let mut g = Game::invite(2);
        g.join_all();
        assert!(g.host.on_disconnected("friend1", g.now).is_empty());
        assert_eq!(g.host.answers()[1].1, Answer::Left);
        assert_eq!(g.host.joined(), 1);
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad")]), g.now).unwrap();
        assert!(out.iter().all(|(to, _)| to == "friend0"));
        // 잠근 뒤 끊기면 판 취소 (공개 전)
        let out = g.host.on_disconnected("friend0", g.now);
        assert!(matches!(g.host.phase(), Phase::Cancelled { reason: CancelReason::Left, who: Some(n) } if n == "손님0"));
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn players_give_up_on_a_silent_host() {
        let now = Instant::now();
        let (_, out) = HostGame::new(seat("방장"), &results(&[("a", "good")]), &ids(1), now).unwrap();
        let mut g = GuestGame::on_invite("host", out[0].1.clone(), now).unwrap();
        assert_eq!(g.tick(now + INVITE_TIMEOUT - Duration::from_millis(1)), None);
        assert!(matches!(g.tick(now + INVITE_TIMEOUT), Some(LadderMsg::LadderDecline { reason, .. }) if reason == "no"));
        assert!(matches!(g.phase(), Phase::Cancelled { reason: CancelReason::Timeout, .. }));
        let mut g = GuestGame::on_invite("host", out[0].1.clone(), now).unwrap();
        g.join(seat("일"), now).unwrap();
        g.tick(now + LOCK_WAIT);
        assert!(matches!(g.phase(), Phase::Cancelled { reason: CancelReason::Timeout, who: Some(n) } if n == "방장"));
        let mut g = GuestGame::on_invite("host", out[0].1.clone(), now).unwrap();
        g.join(seat("일"), now).unwrap();
        g.on_host_disconnected();
        assert!(matches!(g.phase(), Phase::Cancelled { reason: CancelReason::Left, who: Some(n) } if n == "방장"));
    }

    #[test]
    fn an_invite_that_ends_before_any_answer_is_marked_for_a_silent_close() {
        let invite = |now| {
            let (_, out) = HostGame::new(seat("방장"), &results(&[("a", "good")]), &ids(1), now).unwrap();
            GuestGame::on_invite("host", out[0].1.clone(), now).unwrap()
        };
        let now = Instant::now();
        let cancel = |g: &GuestGame| LadderMsg::LadderCancel { game_id: g.game_id(), reason: "host".into(), who: None };
        // 아직 답을 기다리는 초대는 그대로 보임
        let mut g = invite(now);
        assert!(!g.ended_unanswered());
        // 방장이 닫음 · 방장이 끊김 · 시간 초과
        let m = cancel(&g);
        g.on_message(m, now);
        assert!(g.ended_unanswered());
        let mut g = invite(now);
        g.on_host_disconnected();
        assert!(g.ended_unanswered());
        let mut g = invite(now);
        g.tick(now + INVITE_TIMEOUT);
        assert!(g.ended_unanswered());
        // 참가한 뒤의 취소는 안내해야 함
        let mut g = invite(now);
        g.join(seat("일"), now).unwrap();
        assert!(!g.ended_unanswered());
        let m = cancel(&g);
        g.on_message(m, now);
        assert!(g.phase().is_over());
        assert!(!g.ended_unanswered());
    }

    /// 초대에 "당첨 1 + 꽝 2" (3칸)를 보여 주고 친구 둘을 초대해 첫 친구가 참가한 상태를 만듦
    fn joined_to_a_draft() -> (HostGame, GuestGame, Instant) {
        let now = Instant::now();
        let draft = results(&[("당첨", "good"), ("꽝", "bad"), ("꽝", "bad")]);
        let (mut host, out) = HostGame::new(seat("방장"), &draft, &ids(2), now).unwrap();
        let mut guest = GuestGame::on_invite("host", out[0].1.clone(), now).unwrap();
        host.on_message("friend0", guest.join(seat("일"), now).unwrap(), now);
        (host, guest, now)
    }

    #[test]
    fn slots_that_only_shrank_from_the_invite_need_no_confirmation() {
        let (mut host, guest, now) = joined_to_a_draft();
        let out = host.start(&results(&[("꽝", "bad"), ("당첨", "good")]), now).unwrap();
        assert_eq!(guest.lock_changed_results(&out[0].1), None);
    }

    #[test]
    fn slots_that_changed_from_the_invite_are_shown_before_we_reveal() {
        let changed: [&[(&str, &str)]; 3] = [
            &[("벌칙 커피", "bad"), ("꽝", "bad")],        // 글자가 바뀜
            &[("당첨", "bad"), ("꽝", "bad")],             // 기분이 바뀜
            &[("당첨", "good"), ("당첨", "good")],         // 같은 글자라도 초대 때보다 많음
        ];
        for slots in changed {
            let (mut host, mut guest, now) = joined_to_a_draft();
            let out = host.start(&results(slots), now).unwrap();
            let lock = out[0].1.clone();
            assert_eq!(guest.lock_changed_results(&lock), Some(results(slots)));
            // 묻는 동안 아직 아무것도 공개하지 않았고, 물은 것 자체는 상태를 바꾸지 않음
            assert_eq!(*guest.phase(), Phase::Joined);
            // 계속하면 평소처럼 공개함
            assert!(matches!(guest.on_message(lock, now), Some(LadderMsg::LadderReveal { .. })));
            assert_eq!(*guest.phase(), Phase::Locked);
        }
    }

    #[test]
    fn refusing_changed_slots_cancels_the_game_without_revealing() {
        let (mut host, mut guest, now) = joined_to_a_draft();
        let bad = results(&[("벌칙 커피", "bad"), ("꽝", "bad")]);
        host.start(&bad, now).unwrap();
        let reply = guest.refuse_lock().expect("tells the host");
        assert!(matches!(guest.phase(), Phase::Cancelled { reason: CancelReason::Invalid, .. }));
        host.on_message("friend0", reply, now);
        assert!(matches!(host.phase(), Phase::Cancelled { reason: CancelReason::Invalid, who: Some(n) } if n == "일"), "{:?}", host.phase());
        // 이미 끝났으면 다시 알리지 않음
        assert_eq!(guest.refuse_lock(), None);
    }

    #[test]
    fn only_a_valid_lock_for_this_game_is_held_for_confirmation() {
        let (mut host, mut guest, now) = joined_to_a_draft();
        let out = host.start(&results(&[("벌칙 커피", "bad"), ("꽝", "bad")]), now).unwrap();
        let LadderMsg::LadderLock { game_id, host: seat, players, results: slots } = out[0].1.clone() else { panic!("lock") };
        // 다른 판의 잠그기
        let other = LadderMsg::LadderLock { game_id: to_hex(&[9; 16]), host: seat, players: players.clone(), results: slots.clone() };
        assert_eq!(guest.lock_changed_results(&other), None);
        // 형식이 틀린 잠그기는 묻지 않고 평소대로 거부함 (글자가 화면에 나오지 않음)
        let mut long = slots.clone();
        long[0].text = "너무 길어서 칸에 들어가지 않는 글자".into();
        let malformed = LadderMsg::LadderLock { game_id: game_id.clone(), host: seat, players: players.clone(), results: long };
        assert_eq!(guest.lock_changed_results(&malformed), None);
        // 이미 잠긴 뒤에는 묻지 않음
        let lock = LadderMsg::LadderLock { game_id, host: seat, players, results: slots };
        guest.on_message(lock.clone(), now).unwrap();
        assert_eq!(guest.lock_changed_results(&lock), None);
    }

    #[test]
    fn a_player_who_rejects_the_lock_is_reported_as_invalid_not_left() {
        let mut g = Game::invite(2);
        g.join_all();
        let out = g.host.start(&results(&[("a", "good"), ("b", "bad"), ("c", "none")]), g.now).unwrap();
        let gid = g.host.game_id();
        // friend0이 조작된 잠그기를 잡았음
        let out2 = g.host.on_message("friend0", LadderMsg::LadderCancel { game_id: gid, reason: "invalid".into(), who: None }, g.now);
        assert!(matches!(g.host.phase(), Phase::Cancelled { reason: CancelReason::Invalid, who: Some(n) } if n == "손님0"), "{:?}", g.host.phase());
        assert!(out2.iter().all(|(_, m)| matches!(m, LadderMsg::LadderCancel { reason, .. } if reason == "invalid")));
        let _ = out;
    }

    #[test]
    fn a_game_nobody_joins_ends_instead_of_waiting_forever() {
        // 둘 다 거절
        let mut g = Game::invite(2);
        for i in 0..2 {
            g.decline_one(i, DeclineReason::No);
        }
        assert!(matches!(g.host.phase(), Phase::Cancelled { reason: CancelReason::Nobody, .. }), "{:?}", g.host.phase());
        // 하나는 거절, 하나는 답 없음
        let mut g = Game::invite(2);
        g.decline_one(0, DeclineReason::Busy);
        assert_eq!(*g.host.phase(), Phase::Inviting);
        g.host.tick(g.now + INVITE_TIMEOUT);
        assert!(matches!(g.host.phase(), Phase::Cancelled { reason: CancelReason::Nobody, .. }));
        // 한 명이라도 참가했으면 끝내지 않음
        let mut g = Game::invite(2);
        g.join_one(0, "일");
        g.host.tick(g.now + INVITE_TIMEOUT);
        assert_eq!(*g.host.phase(), Phase::Inviting);
        // 끊겨서 0명
        let mut g = Game::invite(1);
        g.host.on_disconnected("friend0", g.now);
        assert!(matches!(g.host.phase(), Phase::Cancelled { reason: CancelReason::Nobody, .. }));
    }

    #[test]
    fn answer_counts_follow_the_answers() {
        let mut g = Game::invite(3);
        assert_eq!(g.host.answer_counts(), [3, 0, 0, 0, 0, 0, 0]);
        g.join_one(0, "일");
        g.decline_one(1, DeclineReason::Busy);
        assert_eq!(g.host.answer_counts(), [1, 1, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn a_late_join_is_told_the_invitation_closed() {
        let now = Instant::now();
        let (mut host, out) = HostGame::new(seat("방장"), &results(&[("a", "good")]), &ids(2), now).unwrap();
        let mut late = GuestGame::on_invite("host", out[0].1.clone(), now).unwrap();
        let mut ok = GuestGame::on_invite("host", out[1].1.clone(), now).unwrap();
        host.on_message("friend1", ok.join(seat("둘"), now).unwrap(), now);
        host.tick(now + INVITE_TIMEOUT); // friend0은 시간 초과
        let join = late.join(seat("일"), now + INVITE_TIMEOUT).unwrap();
        let reply = host.on_message("friend0", join.clone(), now + INVITE_TIMEOUT);
        assert!(matches!(&reply[..], [(to, LadderMsg::LadderCancel { reason, .. })] if to == "friend0" && reason == "host"), "{reply:?}");
        late.on_message(reply[0].1.clone(), now);
        assert!(matches!(late.phase(), Phase::Cancelled { reason: CancelReason::Host, .. }));
        // 잠근 뒤·끝난 뒤의 늦은 참가도 같음
        host.start(&results(&[("a", "good"), ("b", "bad")]), now).unwrap();
        assert_eq!(host.on_message("friend0", join.clone(), now).len(), 1);
        host.abort();
        assert_eq!(host.on_message("friend0", join, now).len(), 1);
        // 이미 참가한 친구가 같은 참가를 또 보내면 무시
        assert!(host.on_message("friend1", LadderMsg::LadderJoin { game_id: host.game_id(), token: to_hex(&[9; 16]), commit: to_hex(&[9; 32]), name: "둘".into(), pet: "cat".into(), style: None }, now).is_empty());
    }

    #[test]
    fn a_player_who_gives_up_on_the_lock_tells_the_host() {
        let now = Instant::now();
        let (mut host, out) = HostGame::new(seat("방장"), &results(&[("a", "good")]), &ids(2), now).unwrap();
        let mut a = GuestGame::on_invite("host", out[0].1.clone(), now).unwrap();
        let mut b = GuestGame::on_invite("host", out[1].1.clone(), now).unwrap();
        host.on_message("friend0", a.join(seat("일"), now).unwrap(), now);
        host.on_message("friend1", b.join(seat("둘"), now).unwrap(), now);
        // 방장이 칸을 고치느라 2분 넘게 잠그지 않았음 — a가 포기하고 알림
        let bye = a.tick(now + LOCK_WAIT).expect("tells the host");
        host.on_message("friend0", bye, now + LOCK_WAIT);
        assert_eq!(host.answers()[0].1, Answer::Left);
        let out = host.start(&results(&[("a", "good"), ("b", "bad")]), now + LOCK_WAIT).expect("locks without the one who left");
        assert!(out.iter().all(|(to, m)| to == "friend1" || !matches!(m, LadderMsg::LadderLock { .. })));
    }

    #[test]
    fn invites_and_joins_are_validated() {
        let now = Instant::now();
        assert!(HostGame::new(seat("방장"), &results(&[("a", "good")]), &[], now).is_none());
        assert!(HostGame::new(seat("방장"), &results(&[("a", "good")]), &ids(10), now).is_none(), "ten invitees + host = 11");
        assert!(HostGame::new(seat("방장"), &results(&[("a", "good")]), &["x".into(), "x".into()], now).is_none());
        assert!(HostGame::new(seat("방장"), &results(&[(" a", "good")]), &ids(1), now).is_none());
        let bad = LadderMsg::LadderInvite { game_id: "zz".into(), host: "A".into(), results: vec![], invited: 1 };
        assert!(GuestGame::on_invite("h", bad, now).is_none());
        // 이상한 참가 신청은 그 사람만 빠짐
        let mut g = Game::invite(2);
        let gid = g.host.game_id();
        let out = g.host.on_message("friend0", LadderMsg::LadderJoin { game_id: gid.clone(), token: "nothex".into(), commit: "c".into(), name: "x".into(), pet: "cat".into(), style: None }, g.now);
        assert_eq!(out.len(), 1);
        assert_eq!(g.host.answers()[0].1, Answer::Left);
        // 초대하지 않은 친구의 참가는 무시
        let out = g.host.on_message("stranger", LadderMsg::LadderJoin { game_id: gid, token: to_hex(&[1; 16]), commit: to_hex(&[2; 32]), name: "x".into(), pet: "cat".into(), style: None }, g.now);
        assert!(out.is_empty());
        assert_eq!(g.host.joined(), 0);
    }

    #[test]
    fn install_ids_never_reach_other_players() {
        // 초대부터 끝까지, 방장과 손님이 주고받는 모든 메시지에서
        let friends = ids(3);
        let now = Instant::now();
        let (mut host, invites) = HostGame::new(seat("방장"), &results(&[("a", "good")]), &friends, now).unwrap();
        let mut wire = invites.clone();
        let mut guests: Vec<(String, GuestGame)> =
            invites.into_iter().map(|(to, m)| (to.clone(), GuestGame::on_invite(&to, m, now).unwrap())).collect();
        let mut from_guests = Vec::new();
        for (i, (id, g)) in guests.iter_mut().enumerate() {
            from_guests.push((id.clone(), g.join(seat(&format!("손님{i}")), now).unwrap()));
        }
        for (id, m) in std::mem::take(&mut from_guests) {
            wire.push((id.clone(), m.clone()));
            wire.extend(host.on_message(&id, m, now));
        }
        let r: Vec<LockedResult> = (0..4).map(|i| LockedResult { text: format!("칸{i}"), mood: "none".into() }).collect();
        let locks = host.start(&r, now).unwrap();
        wire.extend(locks.clone());
        for (to, m) in locks {
            let (id, g) = guests.iter_mut().find(|(id, _)| *id == to).unwrap();
            let reply = g.on_message(m, now).expect("reveal");
            wire.push((id.clone(), reply.clone()));
            wire.extend(host.on_message(&to, reply, now));
        }
        assert!(matches!(host.phase(), Phase::Done(_)), "the whole game was covered: {:?}", host.phase());
        for (_, m) in &wire {
            let json = String::from_utf8(crate::protocol::Message::Ladder(m.clone()).to_json()).unwrap();
            for id in &friends {
                assert!(!json.contains(id.as_str()), "{json}");
            }
        }
    }

    #[test]
    fn length_prefixes_keep_fields_apart() {
        let gid = [7u8; 16];
        let p = |name: &str, pet: &str| LockedPlayer { token: [1; 16], commit: [2; 32], name: name.into(), pet: pet.into(), style: String::new() };
        let r = results(&[("a", "good")]);
        assert_ne!(lock_hash(&gid, 0, &[p("ab", "c")], &r), lock_hash(&gid, 0, &[p("a", "bc")], &r));
        assert_ne!(lock_hash(&gid, 0, &[p("a", "b")], &r), lock_hash(&gid, 1, &[p("a", "b")], &r));
        assert_ne!(lock_hash(&gid, 0, &[p("a", "b")], &r), lock_hash(&gid, 0, &[p("a", "b")], &results(&[("a", "bad")])));
    }

    #[test]
    fn hidden_characters_in_a_name_stay_in_the_hash() {
        // 화면에는 지워서 보이지만, 해시는 받은 글자 그대로 — 지우면 빌드마다 시드가 달라질 수 있음
        assert!(valid_name("나\u{200B}비"));
        let gid = [7u8; 16];
        let p = |name: &str| LockedPlayer { token: [1; 16], commit: [2; 32], name: name.into(), pet: "cat".into(), style: String::new() };
        let r = results(&[("a", "good")]);
        assert_ne!(lock_hash(&gid, 0, &[p("나비")], &r), lock_hash(&gid, 0, &[p("나\u{200B}비")], &r));
    }

    /// 모든 빌드는 같은 게임에서 같은 시드·흐름·지문을 끌어내야 함. 이것이 바뀌면 프로토콜이 바뀐 것임
    #[test]
    fn fixed_vectors() {
        let gid: [u8; 16] = core::array::from_fn(|i| i as u8);
        let t1 = [0x11u8; 16];
        let t2 = [0x22u8; 16];
        let n1 = [0xa1u8; 32];
        let n2 = [0xb2u8; 32];
        let c1 = commit(&gid, &t1, &n1);
        let players = vec![
            LockedPlayer { token: t1, commit: c1, name: "나비".into(), pet: "cat".into(), style: String::new() },
            LockedPlayer { token: t2, commit: commit(&gid, &t2, &n2), name: "Bob".into(), pet: "dino_green".into(), style: "pixel".into() },
        ];
        let res = results(&[("커피 쏘기", "bad"), ("통과", "good")]);
        let l = lock_hash(&gid, 1, &players, &res);
        let s = seed(&l, &[n1, n2]);
        let mut st = Stream::new(s);
        let first: Vec<u32> = (0..10).map(|_| st.next_u32()).collect();
        assert_eq!(to_hex(&c1), FIXED_COMMIT);
        assert_eq!(to_hex(&l), FIXED_LOCK);
        assert_eq!(to_hex(&s), FIXED_SEED);
        assert_eq!(first, FIXED_STREAM);
        assert_eq!(print_bits(&s), FIXED_PRINT);
    }

    // Python hashlib.blake2s로 spec §3.2 인코딩을 따로 계산해 맞춰 본 값 (2026-11-04)
    const FIXED_COMMIT: &str = "aa5bd038fe17d86108aa3b692c4b8fdb0594389b9c8c25cfdfb9aff6e794f072";
    const FIXED_LOCK: &str = "498fa043f9c3d5ec5558def8a0ef11272858947d75e4738af9ff84651b3301ae";
    const FIXED_SEED: &str = "32c5e530c6ff1fc20c3714b008806ef50e411653c8ed223194779e9d2490ecf3";
    const FIXED_STREAM: [u32; 10] = [2453936071, 3814138068, 1911014964, 2019035796, 1773265253, 2762172785, 882518042, 427569910, 3181564715, 1878795270];
    const FIXED_PRINT: u32 = 7442194;
}
