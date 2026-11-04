# ScreenPet 사다리타기 (친구들과 미니게임) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.
> Note: tasks here are written as precise requirements + test lists rather than full code (the app wiring sits on `network.rs`/`game_loop.rs`, which are large and still changing). Implementers design the code; reviewers check against the spec. 공정성에 걸린 부분(인코딩·해시·난수열·순열)은 고정 테스트 벡터로 못 박는다.

**Goal:** 연결된 친구를 최대 9명 초대해(방장 포함 10명) 사다리타기를 한다. 결과는 참가자 모두의 난수로 정해지는 완전한 랜덤이고, 모든 참가자 화면에 같은 사다리를 아기 펫 + 이름표로 보여 준다.

**Architecture:** 순수 로직은 리눅스에서 테스트되는 두 곳에 둔다. core `ladder.rs`는 해시를 모르는 그림 쪽(난수열 → 균등 순열 → 그 순열을 그리는 사다리, 길 따라가기, 글자·이름 처리). net `ladder.rs`는 암호 쪽(커밋·잠그기 고정 인코딩·시드·난수열·지문)과 I/O 없는 방장·참가자 상태 기계. `screenpet-net`은 프로토콜 버전을 2로 올려 상대 버전을 기억하고, 예전 판에는 사다리 메시지를 보내지 않는다. 앱은 상태 기계를 네트워크·창·트레이에 잇기만 하고, 사다리 펫은 실제 펫 창이 아니라 사다리 창(`ui/ladder.html`) 안에 그린다.

**Tech Stack:** Rust (core, net, Tauri 2), `blake2`(net에 이미 있음), 바닐라 JS, node 하네스 + pytest.

**Spec:** `docs/superpowers/specs/2026-11-04-screenpet-ladder-design.md` (phase 3 spec 위에 쌓음)

## Global Constraints

- 인원 2~10명(방장 포함), 결과 칸 수 = 인원. 결과 글자 12자(줄바꿈·탭·제어 문자 없음), 기분 `good | bad | none`. 이름 `clamp_name`.
- 시간: 초대 답 30초, 공개 15초, 게임 화면이 뜬 뒤 `ladder_start` 없이 1.5초 뒤 각자 시작. 같은 친구의 초대 1분 3회, 받은 초대 처리 중 1개, 한 설치당 동시에 한 판.
- 해시 문자열은 spec §3.2·§3.3 그대로 (`screenpet-ladder-commit-v1`, `-lock-v1`, `-seed-v1`, `-print-v1`). 난수열 = `BLAKE2s(seed ‖ 카운터 u32 LE)` 이어 붙이기, 정수는 거부 샘플링, 순열은 Fisher–Yates. **사다리 경로에 `fastrand` 금지** (장식 줄 포함 — 모두 시드 난수열에서).
- 설치 id는 다른 참가자에게 보내지 않는다 — 판마다 새로 만드는 16바이트 토큰만.
- 앱 프로토콜 `screenpet_net::protocol::PROTOCOL_VERSION` 1 → 2. 중계 `screenpet_relay::proto::PROTOCOL_VERSION`은 건드리지 않는다. `v < 2`인 상대에게는 `Ladder` 메시지를 절대 보내지 않는다 (예전 판은 모르는 `type`이면 연결을 끊는다 — `peers.rs` `Message::parse` 실패 → `break`).
- 거절: `Pet::refuses_arrivals()`(집중 중·하우스 안) → 자동 `busy`. 전체 화면은 거절 사유가 아니다. 초대 창은 포커스를 가져가지 않고 클릭으로만 답한다.
- 화면 글자는 `textContent`로만. 사다리 창은 주 모니터 가운데 (`pair.html`과 같은 자리 잡기).
- 리눅스에서 src-tauri 컴파일 불가 (CI 검증). 플랫폼 영향이 있는 core 변경은 `cargo check -p screenpet-core --target x86_64-pc-windows-gnu`와 `--target aarch64-apple-darwin`. 커밋 끝 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. 브랜치 `feat/ladder` (main 위).

---

### Task 1: core — 순열·사다리·길 따라가기 (`ladder.rs`)

**Files:** Create `crates/screenpet-core/src/ladder.rs`; Modify `crates/screenpet-core/src/lib.rs` (`pub mod ladder;`).

**Interfaces — Produces:**
```rust
pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 10;
pub const MIN_ROWS: usize = 8;
pub const RESULT_MAX_CHARS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
pub enum Mood { Good, Bad, None }

/// rows[r]에 든 c = 그 줄에서 세로줄 c와 c+1을 잇는 가로줄 (한 줄 안에서 이웃하지 않음)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ladder { pub columns: usize, pub rows: Vec<Vec<usize>> }

/// [0, bound) 균등 정수 (거부 샘플링 — 나머지 치우침 없음)
pub fn uniform(next: &mut impl FnMut() -> u32, bound: u32) -> u32;
/// Fisher–Yates. perm[i] = 위 칸 i가 닿는 결과 칸
pub fn permutation(n: usize, next: &mut impl FnMut() -> u32) -> Vec<usize>;
/// perm이 되도록 홀짝 전치 정렬로 가로줄을 만들고, 장식 쌍(같은 자리 연달아 두 줄)으로 MIN_ROWS 이상 채운다
pub fn build(perm: &[usize], next: &mut impl FnMut() -> u32) -> Ladder;
/// 위 칸 start에서 내려간 길: 맨 위와 각 줄을 지난 뒤의 세로줄 (길이 = 줄 수 + 1), 마지막이 결과 칸
pub fn trace(ladder: &Ladder, start: usize) -> Vec<usize>;
/// 결과 칸 글자 정리: 앞뒤 공백 제거, 제어 문자 있거나 비었거나 12자 넘으면 None
pub fn clean_result(raw: &str) -> Option<String>;
/// 같은 표시 이름에 " ②", " ③" … (처음 것은 그대로)
pub fn disambiguate(names: &[String]) -> Vec<String>;
/// 화면 표시용 줄임: max자 넘으면 max자 + "…"
pub fn ellipsize(s: &str, max: usize) -> String;
/// 칸 너비 (기본 72, 작업 영역 폭에 맞춰 최소 48까지 줄임, 최소 4칸 폭)
pub fn column_width(players: usize, work_width: f64) -> f64;
/// 지문 24비트 → SAS 이모지 번호 4개 (6비트씩)
pub fn print_indices(bits24: u32) -> [usize; 4];
```
- 장식 쌍은 결과를 바꾸지 않아야 한다 (같은 c를 r, r+1 줄에 — 그 두 줄의 이웃 칸과 겹치지 않게). 장식 위치도 `next`에서만 뽑는다.
- 테스트:
  - 2~10명 × 시드 2000개 (테스트 안 난수열은 간단한 결정적 생성기): `trace(start).last() == perm[start]`, 한 줄 안에 `c`와 `c+1`이 같이 없음, `rows.len() >= MIN_ROWS`, 모든 c < columns − 1.
  - 균등성: 3명 × 60000번 → 순열 6가지 카이제곱(자유도 5, 유의수준 0.001 임계값 20.52 미만). 10명 × 20000번 → 위 칸 0의 결과 칸 분포 카이제곱(자유도 9, 임계값 27.88 미만).
  - `uniform`: bound가 2의 거듭제곱이 아닐 때 거부가 일어나는 입력에서도 범위 안·치우침 없음 (bound 3, 꽉 찬 u32 경계값).
  - **고정 벡터**: 정해진 u32 수열 → 정해진 perm과 rows (값을 테스트에 그대로 적어 둔다 — 알고리즘이 바뀌면 깨지게).
  - `clean_result`("", "  당첨  ", 13자, 탭 포함, 이모지), `disambiguate`(["나비","나비","곰","나비"] → ["나비","나비 ②","곰","나비 ③"]), `ellipsize`, `column_width`(10명 × 좁은 화면 → 48 하한), `print_indices`.
- 커밋 `feat(core): ladder — uniform permutation drawn as a ladder`.

### Task 2: net — 프로토콜 버전 2, 상대 버전 기억, `Ladder` 메시지

**Files:** Modify `crates/screenpet-net/src/{protocol,peers,lib}.rs`, `crates/screenpet-net/tests/net.rs`.

**Interfaces — Produces:**
```rust
pub const PROTOCOL_VERSION: u32 = 2;
pub const LADDER_MIN_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] #[serde(tag = "type", rename_all = "snake_case")]
pub enum LadderMsg {
    LadderInvite { game_id: String, host: String, results: Vec<WireResult>, invited: u8 },
    LadderJoin { game_id: String, token: String, commit: String, name: String, pet: String, #[serde(default)] style: Option<String> },
    LadderDecline { game_id: String, reason: String },             // "no" | "busy" | "playing"
    LadderLock { game_id: String, host: u8, players: Vec<WirePlayer>, results: Vec<WireResult> },   // host = players 안 방장 자리
    LadderReveal { game_id: String, nonce: String },
    LadderReveals { game_id: String, nonces: Vec<String> },
    LadderStart { game_id: String, mode: String },                 // "one_by_one" | "all"
    LadderCancel { game_id: String, reason: String, #[serde(default)] who: Option<String> },
}
pub struct WirePlayer { pub token: String, pub name: String, pub pet: String, pub style: Option<String>, pub commit: String }
pub struct WireResult { pub text: String, pub mood: String }
pub enum Message { Control(Control), Visit(Visit), Ladder(LadderMsg) }   // untagged, 끝에 추가

// lib.rs
NetEvent::Ladder { from: String, msg: LadderMsg }
impl Net {
    /// 상대가 연결돼 있고 v >= LADDER_MIN_VERSION일 때만 보낸다. 아니면 false (보내지 않음)
    pub fn send_ladder(&self, peer_id: &str, msg: LadderMsg) -> bool;
    pub fn peer_version(&self, peer_id: &str) -> Option<u32>;
}
```
- `peers`: 받은 `hello.v`를 친구 연결(`Conn`)에 저장. 다시 연결되거나 hello를 다시 받으면 새 값으로. (`Connected` 이벤트는 바꾸지 않는다 — 앱은 필요할 때 `peer_version`을 묻는다.) `Ladder` 메시지는 **친구 연결에서만** `NetEvent::Ladder`로 넘긴다 (친구 아닌 연결에서 오면 무시 — 방문 메시지와 같다). 길이·hex 형식 검사는 Task 3 상태 기계가 하고, 여기선 파싱만.
- `Hello`의 `v` 필드는 이미 필수 `u32` — 예전 판(1)과 서로 파싱 문제없음을 테스트로 확인.
- 테스트 (`tests/net.rs`, localhost):
  - 두 Net 모두 새 판 → `peer_version == Some(2)`, `send_ladder` true, 받은 쪽 `NetEvent::Ladder`.
  - **예전 판 흉내**: hello `v: 1`을 보내는 상대 → `peer_version == Some(1)`, `send_ladder` false, 그 뒤 ping이 계속 오가고 연결이 끊기지 않음.
  - `Message` JSON 왕복 (모든 `LadderMsg` 변형), 모르는 `type` → `parse` None (기존 동작 그대로).
- 커밋 `feat(net): protocol v2 — remember peer versions, ladder messages only to v2 friends`.

### Task 3: net — 사다리 암호와 방장·참가자 상태 기계 (`ladder.rs`)

**Files:** Create `crates/screenpet-net/src/ladder.rs`; Modify `crates/screenpet-net/src/lib.rs` (`pub mod ladder;`).

**Interfaces — Produces:**
```rust
pub fn commit(game_id: &[u8; 16], token: &[u8; 16], nonce: &[u8; 32]) -> [u8; 32];
/// spec §3.2의 고정 바이너리 인코딩을 해시한 L (JSON 바이트를 해시하지 않는다)
pub fn lock_hash(game_id: &[u8; 16], host: u8, players: &[LockedPlayer], results: &[LockedResult]) -> [u8; 32];
pub fn seed(lock_hash: &[u8; 32], nonces: &[[u8; 32]]) -> [u8; 32];
/// BLAKE2s(seed ‖ 카운터 u32 LE) 블록을 이어 붙인 u32 LE 수열
pub struct Stream { /* seed, counter, buf */ }
impl Stream { pub fn new(seed: [u8; 32]) -> Stream; pub fn next_u32(&mut self) -> u32; }
/// 앞 24비트
pub fn print_bits(seed: &[u8; 32]) -> u32;

pub struct LockedPlayer { pub token: [u8; 16], pub commit: [u8; 32], pub name: String, pub pet: String, pub style: String }
/// net은 core에 의존하지 않는다 — mood는 u8 (0 none, 1 good, 2 bad), 앱이 core `Mood`로 바꾼다
pub struct LockedResult { pub text: String, pub mood: u8 }

/// 방장 쪽. 보낼 것은 Vec<(받는 친구 id, LadderMsg)>로 돌려준다 (I/O 없음)
pub struct HostGame { .. }
impl HostGame {
    pub fn new(game_id: [u8; 16], me: Seat, results: Vec<LockedResult>, invitees: Vec<String>, now: Instant) -> (HostGame, Vec<(String, LadderMsg)>);
    pub fn on_message(&mut self, from: &str, msg: LadderMsg, now: Instant) -> Vec<(String, LadderMsg)>;
    pub fn on_disconnected(&mut self, id: &str, now: Instant) -> Vec<(String, LadderMsg)>;
    pub fn start(&mut self, now: Instant) -> Vec<(String, LadderMsg)>;     // 방장이 `시작` — 잠그기
    pub fn tick(&mut self, now: Instant) -> Vec<(String, LadderMsg)>;      // 30초·15초 시간 초과
    pub fn phase(&self) -> Phase;
}
/// 참가자 쪽
pub struct GuestGame { .. }
impl GuestGame {
    pub fn on_invite(from: &str, invite: LadderMsg, now: Instant) -> Option<GuestGame>;  // 형식 틀리면 None
    pub fn join(&mut self, me: Seat, now: Instant) -> LadderMsg;                         // 토큰·난수 생성(getrandom), 커밋
    pub fn decline(&mut self, reason: &str) -> LadderMsg;
    pub fn on_message(&mut self, msg: LadderMsg, now: Instant) -> Option<LadderMsg>;        // 잠그기 → 공개 자동
    pub fn on_host_disconnected(&mut self);
    pub fn tick(&mut self, now: Instant);
    pub fn phase(&self) -> Phase;
}
pub struct Seat { pub name: String, pub pet: String, pub style: String }
pub enum Phase {
    Inviting { answered: Vec<(String, Answer)> },   // 방장만
    Invited,                                       // 참가자: 답 기다림
    Joined, Locked,
    Done(Outcome),
    Cancelled { reason: CancelReason, who: Option<String> },   // who = 이름 (토큰 → 잠그기 목록의 이름)
}
pub struct Outcome { pub seed: [u8; 32], pub players: Vec<LockedPlayer>, pub results: Vec<LockedResult>, pub me: usize }
```
- 형식 검사: game_id 16바이트 hex, token 16바이트, commit·nonce 32바이트, 인원 2~10, 결과 칸 수 = 인원, 글자 12자·제어 문자 없음, mood 문자열은 모르면 none. 틀리면 그 판 `Cancelled { Invalid }` (연결은 끊지 않음).
- 참가자: 잠그기에 **내 토큰·커밋이 그대로** 있어야 하고, 토큰이 서로 달라야 한다. 공개 모음은 토큰 순 개수가 맞고 **모두** 커밋과 맞아야 `Done`.
- 방장: 잠그기 전 참가자 끊김은 빼고 계속(방장 말고 1명 이상), 잠근 뒤 끊김·공개 15초 초과는 모두에게 `LadderCancel { who }`.
- 잠그기·공개 메시지 어디에도 설치 id가 없다 (`who`도 토큰).
- 테스트:
  - **고정 벡터**: 정해진 game_id·토큰·난수·이름(한글 포함)·결과 → 정해진 `commit`, `lock_hash`, `seed`, `Stream` 첫 8개, `print_bits` (값을 테스트에 적는다).
  - 인코딩 경계: 이름 순서가 바뀌면 `lock_hash`가 달라짐, 길이 접두가 있어 ("ab","c")와 ("a","bc")가 다른 해시.
  - 메모리 안 3명 시뮬레이션(방장 HostGame + GuestGame 둘을 메시지로 직접 잇기): 모두 같은 seed·`Outcome`.
  - 조작: 공개값 하나 바꿈 → 취소, 잠그기에서 내 커밋 바꿈 → 취소, 토큰 중복 → 취소, 공개 개수 틀림 → 취소, 방장이 참가자마다 다른 목록 → 각자 Done이지만 seed가 다름 (지문이 달라 드러남 — 이 동작을 테스트로 기록).
  - 시간: 초대 30초 무응답 → 그 친구 불참, 공개 15초 → `Cancelled { Timeout, who }`.
  - 잠그기·공개 JSON에 설치 id 문자열이 없음.
- 커밋 `feat(net): ladder commit–reveal seed and host/guest state machines`.

### Task 4: net — localhost 3자 테스트 + QA 봇

**Files:** Modify `crates/screenpet-net/tests/net.rs`; Create `crates/screenpet-net/examples/ladder_bot.rs`.
- 통합 테스트: Net 세 개(방장 + 둘, 서로 다른 포트, 미리 친구로 고정) 위에서 HostGame/GuestGame을 실제 `send_ladder`·`NetEvent::Ladder`로 돌려 같은 seed. 한 참가자를 잠근 뒤 shutdown → 나머지는 `Cancelled { Left, who }`.
- `ladder_bot`: `cargo run -p screenpet-net --example ladder_bot -- --dir <상태 폴더> --name 봇1 [--relay]`.
  - 상태 폴더에 키·친구 목록 저장 (다시 켜도 같은 친구). 친구 신청이 오면 SAS를 출력하고 자동 확인.
  - 초대 오면 자동 참가 (`--decline`, `--busy`, `--drop-after-lock` 옵션으로 거절·바쁨·잠근 뒤 끊김 흉내), 끝나면 seed hex와 지문 이모지 출력.
  - 한 PC에서 봇 여러 개 + 앱 하나로 10명 판 시험 가능 (봇마다 다른 `--dir`).
- 커밋 `feat(net): three-way ladder test and a headless ladder bot for QA`.

### Task 5: 앱 — 상태 기계 연결, 트레이, 설정, 펫 반응

**Files:** Create `src-tauri/src/ladder.rs`; Modify `src-tauri/src/{main,network,tray,game_loop}.rs`, `crates/screenpet-core/src/config.rs` (+ 테스트).
- config: `ladder_invites: bool` (기본 true, 없으면 true), `last_ladder: Option<LastLadder>` (`{ host, at, rows: [{name, text, mood}], print }` — 가장 최근 한 판). 저장은 게임 루프 단일 writer 규칙 유지. config 테스트: 기본값, 예전 설정 파일 읽기, 왕복.
- `network.rs`: 설정 창 목록의 흐림·"(업데이트 필요)"는 `net.peer_version(id)`로 그때그때 본다. `NetEvent::Ladder` → `ladder.rs`. `Disconnected` → 상태 기계 `on_disconnected`/`on_host_disconnected`. 1초마다 `tick`.
- 받은 초대: `ladder_invites` 꺼짐 → 자동 `no`, 내 펫 `refuses_arrivals()` → 자동 `busy`, 이미 판 중 → `playing`, 같은 친구 1분 3회 초과 → 조용히 `no`. 아니면 초대 창.
- 트레이 `친구랑 놀기 ▸`: `🎲 사다리타기…` (v2 친구가 하나라도 연결돼 있고 내가 집중 중·하우스 안이 아닐 때만 활성), `사다리 초대 받기` (체크), `🎲 지난 사다리 결과` (`last_ladder` 있을 때만).
- 창: `ladder` 라벨 하나 (설정·초대·게임·결과를 상태로 바꿈). 초대로 열 때는 `.focused(false)`, `set_focus()`를 부르지 않는다. 방장 설정·지난 결과는 사용자가 연 것이므로 포커스를 준다. 크기는 `ladder::column_width` × 인원 기준.
- 커맨드: `ladder_state`(첫 로드), `ladder_host(friends, results)`, `ladder_answer(join: bool)`, `ladder_begin(results)`(방장 `시작`), `ladder_close`, `ladder_seen`(내 펫 도착 → 진짜 펫 반응). `모두 한꺼번에`는 각자 화면에서만이라 커맨드가 없다. 이벤트 `ladder-state` (상태 바뀔 때). 창 라벨이 `ladder`인 것만 받는다.
- `Done` → core `ladder::permutation` + `build`를 `Stream`으로 돌려 사다리를 만들고 웹뷰에 보냄.
- 표시 이름 (spec §2): 내 것 "나". 별명은 **누가 어느 친구인지 아는 경우에만** — 방장 화면은 모든 참가자에게(토큰 ↔ 친구 id를 `ladder_join`을 받은 링크로 안다), 참가자 화면은 방장(`LadderLock.host`)에게만 내 별명. 나머지는 잠그기 목록의 이름. 그다음 `disambiguate`.
- 내 펫 반응: 사다리 창의 내 펫이 결과 칸에 닿으면(`ladder_seen`) good → `level_up`, bad → `tilt` + 결과 말풍선, none → 없음. core에 `Pet::react_to_result(good)`를 더한다 (기존 상태만 쓴다 — `annoyed`는 끝나면 잠들어서 쓰지 않는다. 하우스 안·놀러 감·공중·먹는 중이면 false).
- 검증: `cargo test -p screenpet-core`(config), `node --check ui/*.js`, CI windows/macos 빌드 초록. 커밋 `feat(app): ladder games with friends — tray, invites, results`.

### Task 6: 웹뷰 — `ladder.html` / `ladder.js` + 하네스

**Files:** Create `ui/ladder.html`, `ui/ladder.js`, `tools/ladder_harness.js`, `tools/test_ladder_ui.py`; Modify `ui/common.js` (시트 한 프레임을 주어진 크기로 그리는 함수, 필요하면), `ui/style.css`(필요 시).
- 화면 네 가지 (`ladder-state.view`): `setup`(친구 체크 목록 — v1은 흐리게 + "업데이트 필요", 바쁨 표시, 결과 칸 글자·기분, 미리 채우기 "당첨 k개"), 답이 다 모인 뒤 결과 칸 수 > 참가 인원 + 1이면: 미리 채우기는 당첨 수를 지키며 다시 만들고, 직접 적은 칸은 "칸이 n개 많아요 — 지울 칸을 골라 주세요" + 칸마다 ✕, 맞을 때까지 `시작` 잠금 (spec §1-3), `invite`(방장·인원·결과 칸·개인정보 안내·`참가`/`안 할래요` — 버튼은 `mousedown`/`click`만, keydown으로 누르지 않음), `game`(사다리·펫·이름표·진행), `result`(표·지문·`결과 복사`·닫기).
- 펫: `pets/<id>/sheet_baby.png` 또는 `pets_pixel/<id>/sheet_baby.png`(없으면 동글, 모르는 id면 `cat`), `pet.json`의 `walk`/`fall`/`slide`/`level_up`/`annoyed`/`sit_front` 행. 프레임은 `frameIndex`, 픽셀은 `pixelFrameSize`. 가로줄 이동 = `walk` 좌우 반전, 세로줄 = `fall`(시트에 `slide`가 있으면 `slide`).
- 진행: 가로줄은 가려 두고 지나간 길만 그린다. 기본 한 마리씩 (위 칸 순), `모두 한꺼번에` 누르면 남은 펫 동시에. `game` 화면이 뜬 뒤 1.5초 안에 `ladder_start`가 안 와도 시작.
- 글자: 이름표 6자 + "…", 결과 칸 6자 + "…"(두 줄), 표에선 전체. 모두 `textContent`.
- `결과 복사`: `code.js`의 `copyText`를 `common.js`로 옮겨 같이 쓰고, 형식은 spec §1.6.
- 하네스 (`levelup_harness.js` 방식, 가짜 DOM): 10명 이름표가 겹치지 않음, 긴 이름·결과 줄임, 같은 이름 번호, 결과 표 글자, 복사 글자(지문 포함), 모르는 동물 → cat, 픽셀 없는 동물 → 동글, 초대 화면에서 Enter keydown이 답하지 않음.
- 커밋 `feat(ui): ladder window — setup, invite, game and results`.

### Task 7: 문서 — 도움말·README

**Files:** Modify `ui/help.html`, `README.md`.
- 도움말 `💞 친구 맺기` 다음에 `🎲 사다리타기` 묶음: 시작 방법, 결과는 모두의 난수로 정해짐, 지문 이모지 비교, 바쁠 땐 자동 거절, 초대 받기 끄기, 지난 결과.
- README 사용법에 한 줄, 수동 테스트 체크리스트에 `### 사다리타기 (PC 두 대 + 봇)`:
  - [ ] 예전 판 친구는 흐리게 "(업데이트 필요)"이고 고를 수 없음, 연결은 끊기지 않음
  - [ ] 초대 창이 뜰 때 메모장에 치던 글자가 끊기지 않음 (포커스 안 뺏음), Enter로 참가되지 않음
  - [ ] 집중 중·하우스 안이면 초대 창 없이 방장 쪽에 "바빠요"
  - [ ] 봇 8개 + PC 두 대로 10명 판: 모든 화면 지문 이모지가 같음, 결과 표가 같음
  - [ ] 잠근 뒤 봇 하나 끊기(`--drop-after-lock`) → 모두 "<이름>과 연결이 끊겨 취소됐어요"
  - [ ] 결과 공개 직후 방장 앱 종료 → 참가자 화면은 3초 뒤 스스로 진행
  - [ ] 결과 기분 😀/😢에 따라 사다리 펫과 내 진짜 펫이 만세/시무룩
  - [ ] `결과 복사` → 채팅창에 붙여 넣으면 이름 → 결과 + 지문
  - [ ] `지난 사다리 결과`가 앱 재시작 뒤에도 열림
  - [ ] 125%·150% 배율 노트북에서 10명 사다리 창이 화면 안에 들어옴
  - [ ] 사다리 초대 받기 끄면 창 없이 거절
  - [ ] 4칸 직접 적고 3명 초대 → 1명 거절: "칸이 1개 많아요", ✕로 지울 때까지 `시작` 잠김 / 미리 채우기는 꽝만 하나 줄어듦
- 커밋 `docs: ladder games in help and the manual checklist`.

### Task 8: 최종 검토 → PR (버전·릴리스는 메인테이너)

- 전체 리뷰 (spec 대조, 특히 §3 공정성·§4 버전·설치 id 비노출), `cargo test -p screenpet-core`, `cargo test -p screenpet-net`, `cargo test -p screenpet-relay`, 두 `cargo check --target`, `cd tools && python -m pytest -q`.
- PR에서는 버전을 올리지 않는다. PR 설명에 "프로토콜 v2 — 친구도 새 판이어야 같이 할 수 있다, minor(0.5.0) 권장"을 적는다.
- 머지 뒤 (메인테이너): 버전 파일 5개 → `chore: vX.Y.Z`, main CI 초록 확인 뒤 태그, 릴리스 노트에 "친구도 이 버전 이상이어야 사다리타기를 같이 할 수 있어요".
- CI는 `screenpet-net`에서 `--test relay`만 돌린다 — 사다리 상태 기계(`--lib`)·`tests/ladder.rs`를 CI에 넣을지 메인테이너가 정한다.
