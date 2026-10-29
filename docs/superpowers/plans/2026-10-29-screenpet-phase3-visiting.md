# ScreenPet 3 (친구 화면 놀러 가기) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.
> Note: tasks here are written as precise requirements + test lists rather than full code (networking and multi-window refactors depend on code that is still changing in phase 2B). Implementers design the code; reviewers check against the spec.

**Goal:** 같은 네트워크(직접) 또는 인터넷(중계 서버)으로 ScreenPet끼리 서로 찾아 한 번 허락하면 계속 연결되고, 동물이 스스로 화면 끝을 넘어 친구 화면으로 놀러 갔다 돌아온다.

**Architecture:** 새 크레이트 `crates/screenpet-net`(std 스레드 + UDP 브로드캐스트 발견 + TCP 줄 단위 JSON)이 친구 연결을 관리하고 채널로 이벤트를 낸다. core `brain`은 화면 끝 나가기(Exit)·들어오기(Enter)·손님 귀가 규칙만 안다. 앱은 게임 루프를 여러 펫(내 동물 + 손님들) × 여러 창으로 바꾸고, 네트워크 이벤트를 펫 생성·제거로 잇는다.

**Spec:** `docs/superpowers/specs/2026-10-29-screenpet-phase3-visiting-design.md`

## Global Constraints

- UDP 발견 포트 47321, 2초마다 알림, 6초 소식 없으면 제거. TCP는 OS 지정 포트. ping 2초, 6초 무응답 → 끊김. 줄 최대 64KB.
- 친구(저장된 id)·허락 대기 중인 상대만 메시지 수신. 알 수 없는 type → 연결 끊기.
- 나가기 확률 30% (연결된 끝의 바닥 Wall에서). 손님: 집 방향 끝 50% 귀가, 3~6분 뒤 그리움(반드시 귀가), come_home 즉시. 손님은 배고픔·밥·성장 없음.
- 첫 친구 오른쪽 끝, 두 번째 왼쪽, 이후 번갈아. 트레이에서 바꿀 수 있음.
- 새 외부 의존성은 최소: `screenpet-net`은 std + serde/serde_json + `fastrand`(id 생성)만. 비동기 런타임 쓰지 않음.
- 리눅스에서 src-tauri 컴파일 불가 (CI 검증). 커밋 끝 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. 브랜치 `feat/phase3` (feat/phase2b 위).

---

### Task 1: `screenpet-net` 크레이트 — 발견·초대·연결·메시지

**Files:** Create `crates/screenpet-net/{Cargo.toml,src/lib.rs,src/protocol.rs,src/discovery.rs,src/peers.rs}`; Modify root `Cargo.toml` (members, default-members에 추가).

**Interfaces — Produces:**
```rust
pub struct NetConfig { pub id: String, pub name: String, pub discovery_port: u16, pub broadcast_to: SocketAddr, pub friends: Vec<String> }
pub enum NetEvent { PeerSeen { id: String, name: String }, PeerGone { id: String },
                    Invited { from: String, name: String, pet: String }, InviteAnswered { from: String, accepted: bool },
                    Connected { id: String, name: String }, Disconnected { id: String }, Received { from: String, msg: Visit } }
pub enum Visit { Visit { visit_id: u64, species: String, stage: String, scale: f64, personality: String, owner: String, enter_side: Side },
                 Leave { visit_id: u64 }, ComeHome { visit_id: u64 } }   // serde tag = "type", snake_case
pub enum Side { Left, Right }
pub struct Net { ... }
impl Net {
    pub fn start(cfg: NetConfig) -> std::io::Result<(Net, std::sync::mpsc::Receiver<NetEvent>)>;
    pub fn invite(&self, peer_id: &str, pet: &str);
    pub fn answer_invite(&self, peer_id: &str, accept: bool);
    pub fn send(&self, peer_id: &str, msg: Visit);
    pub fn unfriend(&self, peer_id: &str);
    pub fn friends(&self) -> Vec<String>;      // 수락으로 추가된 친구 포함 (앱이 config에 저장)
    pub fn shutdown(self);
}
pub fn new_install_id() -> String;               // 32 hex
```
- 내부 프로토콜(`protocol.rs`): `hello`, `invite`, `invite_reply`, `unfriend`, `ping` + `Visit`의 세 종류. serde `#[serde(tag = "type", rename_all = "snake_case")]`.
- 발견: 자기 id의 알림은 무시. 친구인 id를 보면 자동 연결(id가 작은 쪽이 연결을 건다 — 양쪽이 동시에 걸지 않도록).
- 초대 흐름: invite → 상대 `Invited` 이벤트 → `answer_invite` → `InviteAnswered` → 수락이면 양쪽 friends에 추가하고 `Connected`.
- 테스트(localhost, 두 Net 인스턴스, `broadcast_to = 127.0.0.1:<서로의 discovery 포트>`, 테스트마다 다른 포트): 발견, 초대 수락/거절, 친구 자동 재연결(Net 재시작 후), 메시지 왕복, 상대 shutdown → 6초 안에 Disconnected, 64KB 넘는 줄 → 끊김, 알 수 없는 type → 끊김, 친구 아닌 id의 visit → 무시.
- 커밋 `feat(net): LAN discovery, one-time invite, friend connections`.

### Task 2: core — 화면 끝 나가기·들어오기·손님 귀가

**Files:** Modify `crates/screenpet-core/src/{brain,world,behavior}.rs`.
- `Pet::set_exit_edges(left: bool, right: bool)` — 연결된 친구가 있는 화면 끝. 바닥(Ground) 이동 중 그 끝의 Wall에 닿으면 30% → `State::Exit { remaining: 1.0, side }`: 그 방향으로 계속 걸어 화면 밖으로(빈 몸이 창 밖으로 나감 — 위치는 경계 넘어 진행), 끝나면 `take_exit() -> Option<Side>`가 한 번 Some.
- `Pet::new_entering(cfg, world, side, seed)` — 해당 끝 바깥 바닥에서 시작해 1초간 안쪽으로 걸어 들어옴 (`State::Enter`), 끝나면 평소 행동.
- 손님 모드: `Pet::set_guest(home: Side, homesick_after: f64)` — 배고픔 무시(set_hungry 무시), 집 방향 Wall 도달 시 50% Exit(그리움 이후 100%), 반대 끝은 Turn. `go_home_now()` → 손 흔들기 1초(`Trick::Wave`) 후 `take_exit()` Some(home)으로 사라짐(걸어 나가지 않고).
- 애니메이션 추가 없음 (Exit/Enter = walk, 귀가 인사 = wave).
- 테스트: 나가기 30%(시드 루프), 연결 없는 끝은 Turn, Exit 후 take_exit 1회, Enter 후 정상 행동, 손님 집 방향 50%·그리움 후 100%, go_home_now, 손님 배고픔 무시, 10분 시뮬레이션 기존 그대로.
- 커밋 `feat(core): walk off screen edges to visit, enter from edges, guest homing`.

### Task 3: 앱 — 여러 펫·여러 창

**Files:** Modify `src-tauri/src/{game_loop,main,tray}.rs`, `ui/main.js` (+ index/style 필요 시).
- 게임 루프가 `Vec<Slot>`을 돈다: `Slot { label: String, pet: Pet, window: WebviewWindow, role: Own | Guest { visit_id, owner_id, owner_name, species }, stage_scale, ... }`. 내 동물은 `pet` 창(기존), 손님은 `WebviewWindowBuilder`로 `guest-<visit_id>` 창 생성(투명·테두리 없음·항상 위·작업표시줄 숨김, 크기 = 손님 단계 크기 + 이름표 띠), 제거 시 close.
- 웹뷰 초기화: 창 라벨로 `invoke("slot_info")` → `{ pet, stage, scale, role, owner }`. 이벤트는 `emit_to(label, ...)`. 손님 웹뷰: 이름표 "🏠 <owner>" (띠 위치), 음식 고르기 없음.
- 커서·간질이기·드래그: 커서 아래 있는 슬롯(맨 위)에만 전달. `pointer_down` 커맨드는 호출한 창의 라벨로 슬롯을 찾는다 (`tauri::Window` 인자).
- 내 동물이 놀러 가 있는 동안 `pet` 창은 숨김.
- 커밋 `feat(app): multiple pets and guest windows`.

### Task 4: 앱 — 네트워크 연결 UI

**Files:** Modify `src-tauri/src/{main,game_loop,tray}.rs`, `src-tauri/Cargo.toml` (screenpet-net 의존), `crates/screenpet-core/src/config.rs` (install_id, friends: Vec<{id,name,side}>), `ui/*`, `README.md`.
- 시작 시 `Net::start` (실패해도 앱은 동작 — 로그만), 네트워크 이벤트를 게임 루프 입력으로.
- 트레이 `친구 화면 ▸`: 발견된 비친구(클릭 → 초대), 친구별 하위메뉴(왼쪽에/오른쪽에, 연결 끊기, 연결 상태 표시 ●/○), `집에 와`.
- 초대 받기: 펫 머리 위 띠에 "<이름>의 <동물>이 놀러 와도 될까요?" + ✓ ✗ (20초 → 거절).
- 내 동물 Exit → 연결된 친구 중 그 끝의 친구에게 `Visit` 전송, 내 창 숨김. 실패(끊김)면 즉시 반대로 Enter.
- `Received Visit` → 손님 슬롯 생성(Enter). 손님 Exit/go_home → `Leave` 전송, 슬롯 제거. `Received Leave` → 내 동물 Enter(나갔던 끝). `ComeHome` 수신 → 해당 손님 go_home_now. `Disconnected` → 그 친구의 손님 제거 + 내 동물이 그 친구에게 가 있으면 즉시 Enter.
- config에 install_id(처음 생성), friends 저장(단일 writer = 게임 루프 규칙 유지).
- README 체크리스트 (두 PC 필요): 발견, 초대·수락, 재시작 후 자동 재연결, 스스로 넘어가기·돌아오기, 집에 와, PC 끄면 즉시 귀환, 방화벽 허용 창.
- 커밋 `feat(app): visit friends on the same network`.

### Task 5: 중계 서버 `screenpet-relay` + 앱 중계 전송

**Files:** Create `crates/screenpet-relay/` (bin), `deploy/relay/{README.md,screenpet-relay.service}`; Modify `crates/screenpet-net` (relay transport), app (친구 코드 UI), CI (relay 리눅스 바이너리 빌드 아티팩트).
- 서버: 동기 WebSocket(`tungstenite`) + std 스레드, 포트 기본 9411, 데이터 파일(JSON)에 id·토큰 해시(SHA-256)·친구 관계. 메시지 라우팅·제한은 spec §1b.
- 클라이언트 전송: `wss://` (rustls), 끊기면 5초→최대 60초 지수 백오프 재접속, 같은 친구가 LAN으로도 보이면 LAN 우선.
- 친구 코드: 서버 발급·24시간 회전, 입력 → invite 흐름 재사용.
- 배포 문서: NPM에서 Proxy Host(도메인 → 중계 서버 IP:9411, **Websockets Support 켜기**, SSL Let's Encrypt), 서버 실행(systemd user 서비스 예시).
- 테스트: 서버를 테스트 안에서 띄워 두 클라이언트 등록·친구 코드·전달·친구 아닌 전달 거부·토큰 위조 거부·제한.

### Task 6: 최종 검토 → 빌드 → v0.3.0 릴리스
