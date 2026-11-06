//! 사다리타기 (spec `2026-11-04-screenpet-ladder-design.md`) — net의 방장·참가자 상태 기계를 네트워크·창·트레이에 잇는다.
//!
//! 공정성(커밋·공개·시드)과 형식 검사는 모두 `screenpet_net::ladder`가 하고, 사다리 그림(순열·가로줄)은
//! `screenpet_core::ladder`가 시드에서 만든다. 여기는 누구에게 무엇을 보낼지, 어떤 화면을 띄울지만 정한다.
//! 창은 `ladder` 하나를 다시 쓰고 화면(view)만 바꾼다. 초대로 뜰 때는 **포커스를 가져가지 않는다** (작업 중인 키 입력을 뺏지 않게).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use screenpet_core::config::{Config, LastLadder, LastLadderRow, MAX_NAME_CHARS};
use screenpet_core::ladder::{self as draw, Mood};
use screenpet_net::ladder::{Answer, CancelReason, DeclineReason, GuestGame, HostGame, LockedResult, Outcome, Phase, Seat};
use screenpet_net::{LadderMsg, Net, LADDER_MIN_VERSION, SAS_EMOJI};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, WebviewWindow, WindowEvent};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::slots::{Slot, OWN_LABEL};
use crate::AppState;

/// 사다리 창 라벨 (capabilities/default.json의 windows에도 있어야 IPC가 된다)
pub const LABEL: &str = "ladder";
/// 화면 다시 그리기 (상태가 안 바뀌어도 — 설정 화면의 연결된 친구 목록 등) 간격
const REFRESH_EVERY: Duration = Duration::from_millis(500);
/// 끝난 판의 진행을 보는 중이라 새 초대를 막는 최대 시간 (10명이 한 명씩 내려가도 넉넉히 — 웹뷰가 "도착"을 못 알려도 막힘이 끝나게)
const WATCH_AT_MOST: Duration = Duration::from_secs(90);
/// 같은 친구의 사다리 초대는 이 시간 안에 이만큼까지 (넘으면 조용히 거절)
const INVITE_WINDOW: Duration = Duration::from_secs(60);
const INVITES_PER_WINDOW: usize = 3;
/// 화면별 창 크기 (논리 px). 게임 화면은 칸 수에 맞춘다
const SETUP_SIZE: (f64, f64) = (380.0, 470.0);
const INVITE_SIZE: (f64, f64) = (340.0, 250.0);
const SMALL_SIZE: (f64, f64) = (320.0, 170.0);
/// 10명 결과 표 + 지문 + 버튼이 다 보이는 크기 (더 작은 화면에서는 표가 스크롤된다)
const LAST_SIZE: (f64, f64) = (360.0, 470.0);
const GAME_HEIGHT: f64 = 540.0;
/// 주 모니터를 모를 때 쓰는 작업 영역 크기 (논리 px)
const DEFAULT_WORK_AREA: (f64, f64) = (1280.0, 720.0);

/// 웹뷰 첫 로드용 지금 화면 (그 뒤로는 `ladder-state` 이벤트)
static VIEW: Mutex<Option<LadderView>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotRow {
    pub text: String,
    /// "good" | "bad" | "none"
    pub mood: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FriendRow {
    pub id: String,
    pub name: String,
    /// 연결됨 + 사다리를 아는 판 (아니면 "업데이트 필요" — 사다리 메시지를 보낼 수 없다)
    pub can_invite: bool,
    /// 초대 뒤 답: "" | "waiting" | "joined" | "no" | "busy" | "playing" | "timeout" | "left"
    pub answer: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayerRow {
    pub name: String,
    pub pet: String,
    /// "" | "pixel"
    pub style: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GameView {
    pub game_id: String,
    pub players: Vec<PlayerRow>,
    pub results: Vec<SlotRow>,
    pub rows: Vec<Vec<usize>>,
    /// 위 칸마다 각 줄을 지난 뒤의 세로줄 (마지막 = 결과 칸)
    pub paths: Vec<Vec<usize>>,
    pub me: usize,
    pub host: usize,
    /// 공정성 지문 이모지 4개
    pub print: String,
    pub column: f64,
    /// 진행 화면 없이 결과 표만 (집중 중·하우스 안)
    pub skip: bool,
    pub host_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CancelView {
    /// "host" | "left" | "timeout" | "invalid"
    pub reason: &'static str,
    pub who: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct LadderView {
    /// "setup" | "inviting" | "invite" | "joined" | "confirm" | "locked" | "game" | "cancelled" | "last"
    pub view: &'static str,
    pub host_name: String,
    pub invited: u8,
    pub friends: Vec<FriendRow>,
    /// 초대 화면: 결과 칸 미리보기. 확인 화면("confirm"): 바뀐 최종 결과 칸
    pub draft: Vec<SlotRow>,
    pub joined: usize,
    pub all_answered: bool,
    pub game: Option<GameView>,
    pub cancel: Option<CancelView>,
    pub last: Option<LastLadder>,
    /// 설정 화면 안내: "" | "busy" (집중 중·하우스 안이라 초대할 수 없음) | "nobody" (고른 친구가 모두 끊김)
    pub notice: &'static str,
}

enum Game {
    Host(HostGame),
    Guest(GuestGame),
}

/// 게임 하나와 화면 상태 (게임 루프 스레드의 `Network`가 가진다)
#[derive(Default)]
pub struct Ladders {
    game: Option<Game>,
    /// 결과 칸이 초대 때와 달라 내 답을 기다리는 잠그기와 그 최종 칸 (답하기 전에는 내 난수를 공개하지 않는다)
    held_lock: Option<(LadderMsg, Vec<LockedResult>)>,
    /// 설정 화면이 열려 있음 (아직 초대 전)
    setup: bool,
    /// 지난 결과 화면이 열려 있음
    showing_last: bool,
    /// 이 판의 결과 그림 (끝난 순간 한 번 만든다)
    layout: Option<GameView>,
    /// 마지막으로 보낸 화면 (바뀔 때만 다시 보낸다)
    sent: Option<LadderView>,
    /// 이 판에서 내 진짜 펫이 이미 반응했음 (= 내 결과를 봤음)
    reacted: bool,
    /// 설정 화면에 띄울 안내 (`LadderView::notice`)
    notice: &'static str,
    /// 이 판의 결과가 나온 시각 (새 초대를 막는 시간의 상한)
    done_at: Option<Instant>,
    invite_log: HashMap<String, VecDeque<Instant>>,
    /// 마지막으로 화면을 맞춘 시각 (틱마다 맞추지 않는다)
    refreshed: Option<Instant>,
}

/// 사다리가 들여다보는 네트워크 쪽 사정
pub struct Ctx<'a> {
    pub net: Option<&'a Net>,
    pub config: &'a Config,
    pub connected: &'a HashSet<String>,
    pub my_name: &'a str,
}

fn seat(ctx: &Ctx, own: &Slot) -> Seat {
    Seat { name: ctx.my_name.to_string(), pet: own.entry.id.clone(), style: ctx.config.style.to_wire().unwrap_or_default() }
}

fn send(ctx: &Ctx, out: Vec<(String, LadderMsg)>) {
    if let Some(net) = ctx.net {
        for (to, m) in out {
            net.send_ladder(&to, m);
        }
    }
}

fn answer_label(a: &Answer) -> &'static str {
    match a {
        Answer::Waiting => "waiting",
        Answer::Joined => "joined",
        Answer::Declined(DeclineReason::No) => "no",
        Answer::Declined(DeclineReason::Busy) => "busy",
        Answer::Declined(DeclineReason::Playing) => "playing",
        Answer::TimedOut => "timeout",
        Answer::Left => "left",
    }
}

/// 친구가 알린 이름을 화면용으로 (상태 기계는 받은 글자 그대로 해시한다)
fn shown_name(raw: &str) -> String {
    draw::shown_text(raw, MAX_NAME_CHARS)
}

fn slot_rows(r: &[LockedResult]) -> Vec<SlotRow> {
    r.iter().map(|r| SlotRow { text: draw::shown_text(&r.text, draw::RESULT_MAX_CHARS), mood: r.mood.clone() }).collect()
}

/// 웹뷰가 보낸 결과 칸 → 상태 기계용 (글자는 core 규칙으로 다듬는다 — 다듬은 뒤엔 net이 그대로 검사한다)
fn locked(rows: &[SlotRow]) -> Option<Vec<LockedResult>> {
    rows.iter().map(|r| Some(LockedResult { text: draw::clean_result(&r.text)?, mood: Mood::from_wire(&r.mood).to_wire().to_string() })).collect()
}

/// 위 칸 `i`가 닿는 결과 칸
fn end_column(view: &GameView, i: usize) -> usize {
    *view.paths[i].last().expect("path")
}

fn print_emoji(print: u32) -> String {
    draw::print_indices(print).iter().map(|&i| SAS_EMOJI[i]).collect()
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// 주 모니터 작업 영역 크기 (논리 px)
fn work_area(app: &AppHandle) -> (f64, f64) {
    app.primary_monitor().ok().flatten().map_or(DEFAULT_WORK_AREA, |m| {
        let (area, dpi) = (m.work_area().size, m.scale_factor());
        (area.width as f64 / dpi, area.height as f64 / dpi)
    })
}

impl Ladders {
    pub fn busy(&self) -> bool {
        self.game.as_ref().is_some_and(|g| !phase(g).is_over())
    }

    /// 메뉴 "🎲 사다리타기 ▸ 새 사다리…": 판이 있으면 그 창을, 없으면 설정 화면을
    pub fn open(&mut self, app: &AppHandle, ctx: &Ctx) {
        if !self.busy() {
            self.game = None;
            self.layout = None;
            self.setup = true;
            self.notice = "";
        }
        self.showing_last = false;
        self.refresh(app, ctx, true);
    }

    /// 메뉴 "🎲 사다리타기 ▸ 지난 결과"
    pub fn show_last(&mut self, app: &AppHandle, ctx: &Ctx) {
        if ctx.config.last_ladder.is_none() || self.busy() {
            return;
        }
        self.game = None;
        self.setup = false;
        self.showing_last = true;
        self.refresh(app, ctx, true);
    }

    /// 설정 화면 "초대": 고른 친구에게 초대를 보낸다
    pub fn host(&mut self, app: &AppHandle, ctx: &Ctx, own: &Slot, friends: Vec<String>, draft: Vec<SlotRow>) {
        if self.busy() {
            return;
        }
        // 초대 버튼이 아무 반응 없이 끝나지 않게, 못 하는 이유를 설정 화면에 띄운다
        if own.pet.refuses_arrivals() {
            self.notice = "busy";
            self.refresh(app, ctx, false);
            return;
        }
        // 지금 연결돼 있고 사다리를 아는 친구만 (웹뷰가 보낸 목록을 믿지 않는다)
        let invitable: Vec<String> =
            friends.into_iter().filter(|id| ctx.config.friend(id).is_some() && ctx.net.and_then(|n| n.peer_version(id)).is_some_and(|v| v >= LADDER_MIN_VERSION)).collect();
        let fail = |s: &mut Self, why: &'static str| {
            s.notice = why;
            s.refresh(app, ctx, false);
        };
        let Some(draft) = locked(&draft) else { return fail(self, "slots") };
        if invitable.is_empty() {
            return fail(self, "nobody");
        }
        let Some((game, out)) = HostGame::new(seat(ctx, own), &draft, &invitable, Instant::now()) else {
            eprintln!("ladder: cannot start a game with {} friends", invitable.len());
            return fail(self, "invalid");
        };
        self.notice = "";
        send(ctx, out);
        self.game = Some(Game::Host(game));
        self.setup = false;
        self.layout = None;
        self.reacted = false;
        self.refresh(app, ctx, true);
    }

    /// 방장 "시작": 최종 결과 칸으로 잠근다 (칸 수가 안 맞으면 아무것도 안 한다 — 웹뷰가 막는다)
    pub fn begin(&mut self, app: &AppHandle, ctx: &Ctx, results: Vec<SlotRow>) {
        let Some(Game::Host(h)) = self.game.as_mut() else { return };
        let Some(results) = locked(&results) else { return };
        if let Some(out) = h.start(&results, Instant::now()) {
            send(ctx, out);
        }
        self.refresh(app, ctx, false);
    }

    /// 초대 화면 "참가" / "안 할래요"
    pub fn answer(&mut self, app: &AppHandle, ctx: &Ctx, own: &Slot, join: bool) {
        let Some(Game::Guest(g)) = self.game.as_mut() else { return };
        let host = g.host_id().to_string();
        let msg = match self.held_lock.take() {
            Some((lock, _)) if join => g.on_message(lock, Instant::now()),
            Some(_) => g.refuse_lock(),
            None if join => g.join(seat(ctx, own), Instant::now()),
            None => g.decline(DeclineReason::No),
        };
        if let Some(m) = msg {
            send(ctx, vec![(host, m)]);
        }
        if !join {
            self.game = None;
            self.hide_window(app);
            return;
        }
        self.refresh(app, ctx, false);
    }

    /// 창 닫기: 진행 중인 판이면 그만둔다고 알린다
    pub fn close(&mut self, app: &AppHandle, ctx: &Ctx) {
        match self.game.as_mut() {
            Some(Game::Host(h)) => send(ctx, h.abort()),
            Some(Game::Guest(g)) => {
                let host = g.host_id().to_string();
                if let Some(m) = g.quit() {
                    send(ctx, vec![(host, m)]);
                }
            }
            None => {}
        }
        self.game = None;
        self.held_lock = None;
        self.setup = false;
        self.showing_last = false;
        self.layout = None;
        self.hide_window(app);
    }

    /// 창을 숨기고 "마지막으로 보낸 화면"도 잊는다 — 잊지 않으면 다음에 똑같은 초대가 와도 같은 화면이라 창을 다시 띄우지 않는다
    fn hide_window(&mut self, app: &AppHandle) {
        self.sent = None;
        hide(app);
    }

    /// 친구의 사다리 메시지 (인증된 친구 링크 — net이 확인한다). 판이 끝나 결과가 나오면 설정에 남길 것을 돌려준다
    pub fn on_message(&mut self, app: &AppHandle, ctx: &Ctx, own: &Slot, from: &str, msg: LadderMsg) -> Option<LastLadder> {
        let now = Instant::now();
        if let LadderMsg::LadderInvite { .. } = msg {
            self.on_invite(app, ctx, own, from, msg, now);
            return None;
        }
        match self.game.as_mut() {
            Some(Game::Host(h)) => send(ctx, h.on_message(from, msg, now)),
            Some(Game::Guest(g)) if g.host_id() == from => {
                if let LadderMsg::LadderStart { game_id, .. } = &msg {
                    // 다른 판(앞 판)의 늦은 신호는 무시한다
                    if *game_id == g.game_id() {
                        let _ = app.emit_to(LABEL, "ladder-start", ());
                    }
                    return None;
                }
                // 초대 때와 다른 결과 칸: 공개하기 전에 묻는다 (방장의 공개 기한이 지나면 방장이 판을 취소한다)
                if let Some(changed) = g.lock_changed_results(&msg) {
                    self.held_lock = Some((msg, changed));
                    self.refresh(app, ctx, false);
                    return None;
                }
                if let Some(reply) = g.on_message(msg, now) {
                    send(ctx, vec![(from.to_string(), reply)]);
                }
            }
            _ => return None,
        }
        self.after_change(app, ctx, own)
    }

    fn on_invite(&mut self, app: &AppHandle, ctx: &Ctx, own: &Slot, from: &str, msg: LadderMsg, now: Instant) {
        if ctx.config.friend(from).is_none() {
            return;
        }
        let Some(mut g) = GuestGame::on_invite(from, msg, now) else { return };
        let log = self.invite_log.entry(from.to_string()).or_default();
        log.retain(|t| now.duration_since(*t) < INVITE_WINDOW);
        // 넘친 초대는 세지 않는다 (받은 만큼 기록이 자라지 않게)
        let flooding = log.len() >= INVITES_PER_WINDOW;
        if !flooding {
            log.push_back(now);
        }
        // 판 중(내가 방장 설정 화면을 열어 둔 것도, 끝난 판의 진행·결과 화면을 보고 있는 것도) · 너무 잦음 · 초대 받기 끔 ·
        // 집중 중이거나 하우스 안 (기존 거절 규칙) → 창 없이 답한다
        // 끝난 판은 내 펫이 결과 칸에 닿기 전(아직 내려가는 중)에만 막는다 — 취소 안내나 이미 본 결과는 새 초대로 바꾼다
        let watching = self.game.as_ref().is_some_and(|g| matches!(phase(g), Phase::Done(_)))
            && !self.reacted
            && self.sent.is_some()
            && self.done_at.is_some_and(|t| now.duration_since(t) < WATCH_AT_MOST);
        let auto = if self.busy() || self.setup || watching || self.showing_last {
            Some(DeclineReason::Playing)
        } else if flooding || ctx.config.ladder_invites_off {
            Some(DeclineReason::No)
        } else if own.pet.refuses_arrivals() {
            Some(DeclineReason::Busy)
        } else {
            None
        };
        if let Some(reason) = auto {
            if let Some(m) = g.decline(reason) {
                send(ctx, vec![(from.to_string(), m)]);
            }
            return;
        }
        self.game = Some(Game::Guest(g));
        self.held_lock = None;
        self.setup = false;
        self.showing_last = false;
        self.layout = None;
        self.reacted = false;
        self.refresh(app, ctx, false);
    }

    pub fn on_disconnected(&mut self, app: &AppHandle, ctx: &Ctx, own: &Slot, id: &str) -> Option<LastLadder> {
        match self.game.as_mut() {
            Some(Game::Host(h)) => send(ctx, h.on_disconnected(id, Instant::now())),
            Some(Game::Guest(g)) if g.host_id() == id => g.on_host_disconnected(),
            _ => return None,
        }
        self.after_change(app, ctx, own)
    }

    /// 시간 초과 (게임 루프 틱마다). 화면은 상태가 바뀌었을 때, 아니면 0.5초마다만 맞춘다
    pub fn tick(&mut self, app: &AppHandle, ctx: &Ctx, own: &Slot) {
        let now = Instant::now();
        let due = self.refreshed.is_none_or(|t| now.duration_since(t) >= REFRESH_EVERY);
        let Some(game) = self.game.as_mut() else {
            // 설정 화면: 연결된 친구 목록이 바뀔 수 있다
            if self.setup && due {
                self.refresh(app, ctx, false);
            }
            return;
        };
        // 끝난 판에는 기한이 없다 (결과 화면은 그대로)
        if phase(game).is_over() {
            return;
        }
        let before = state_key(game);
        match game {
            Game::Host(h) => send(ctx, h.tick(now)),
            Game::Guest(g) => {
                let host = g.host_id().to_string();
                if let Some(m) = g.tick(now) {
                    send(ctx, vec![(host, m)]);
                }
            }
        }
        let changed = self.game.as_ref().map(state_key) != Some(before);
        if changed {
            // 시간 초과로는 취소만 되므로 결과를 남길 일은 없다
            let _ = self.after_change(app, ctx, own);
        } else if due {
            self.refresh(app, ctx, false);
        }
    }

    /// 상태가 바뀐 뒤: 방금 끝났으면 결과 그림을 만들고 (방장은 진행 시작 신호도 보낸다), 화면을 맞춘다
    fn after_change(&mut self, app: &AppHandle, ctx: &Ctx, own: &Slot) -> Option<LastLadder> {
        // 답하지 않은 초대가 끝났다 (시간 초과·방장이 닫음·방장이 끊김) — 안내 카드로 남기지 않고 창을 닫는다
        if matches!(&self.game, Some(Game::Guest(g)) if g.ended_unanswered()) {
            self.game = None;
            self.hide_window(app);
            return None;
        }
        let mut last = None;
        let mut start_now = false;
        if self.layout.is_none() {
            if let Some(o) = self.game.as_ref().and_then(|g| match phase(g) {
                Phase::Done(o) => Some(o.clone()),
                _ => None,
            }) {
                let view = self.layout_for(app, ctx, &o, own.pet.refuses_arrivals());
                self.done_at = Some(Instant::now());
                // 진행 없이 결과 표만 보는 경우는 이미 결과를 본 것으로 친다
                if view.skip {
                    self.reacted = true;
                }
                last = Some(LastLadder {
                    host: view.host_name.clone(),
                    at: unix_now(),
                    print: view.print.clone(),
                    rows: view
                        .players
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let end = end_column(&view, i);
                            LastLadderRow { name: p.name.clone(), result: view.results[end].text.clone(), mood: view.results[end].mood.clone(), me: i == view.me }
                        })
                        .collect(),
                });
                if let Some(Game::Host(h)) = &self.game {
                    let msg = LadderMsg::LadderStart { game_id: h.game_id(), mode: "one_by_one".into() };
                    let to: Vec<(String, LadderMsg)> = o.players.iter().filter_map(|p| h.friend_of(&p.token)).map(|id| (id, msg.clone())).collect();
                    send(ctx, to);
                    // 방장 화면도 같은 순간에 시작한다 (웹뷰는 화면이 뜬 뒤 들어온 신호를 기억했다가 시작한다)
                    start_now = true;
                }
                self.layout = Some(view);
            }
        }
        self.refresh(app, ctx, false);
        if start_now {
            let _ = app.emit_to(LABEL, "ladder-start", ());
        }
        last
    }

    /// 결과 그림: 시드 → 순열 → 사다리 → 칸마다 길. 이름은 내 화면에 보일 이름 (나 / 별명 / 스스로 알린 이름 + 번호)
    fn layout_for(&self, app: &AppHandle, ctx: &Ctx, o: &Outcome, skip: bool) -> GameView {
        let mut stream = o.stream();
        let mut next = || stream.next_u32();
        let perm = draw::permutation(o.players.len(), &mut next);
        let ladder = draw::build(&perm, &mut next);
        let paths: Vec<Vec<usize>> = (0..o.players.len()).map(|i| draw::trace(&ladder, i)).collect();
        let (host_friend, me_host) = match &self.game {
            Some(Game::Guest(g)) => (Some(g.host_id().to_string()), false),
            _ => (None, true),
        };
        let names: Vec<String> = o
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if i == o.me {
                    return "나".to_string();
                }
                // 별명은 누가 어느 친구인지 아는 경우에만: 방장 화면은 모두, 참가자 화면은 방장만
                let friend = match &self.game {
                    Some(Game::Host(h)) => h.friend_of(&p.token),
                    _ if i == o.host => host_friend.clone(),
                    _ => None,
                };
                friend.and_then(|id| ctx.config.person_name(&id).map(str::to_string)).unwrap_or_else(|| shown_name(&p.name))
            })
            .collect();
        let host_name = if me_host { ctx.my_name.to_string() } else { names[o.host].clone() };
        GameView {
            game_id: match &self.game {
                Some(Game::Host(h)) => h.game_id(),
                Some(Game::Guest(g)) => g.game_id(),
                None => String::new(),
            },
            players: draw::disambiguate(&names).into_iter().zip(&o.players).map(|(name, p)| PlayerRow { name, pet: p.pet.clone(), style: p.style.clone() }).collect(),
            results: slot_rows(&o.results),
            rows: ladder.rows,
            paths,
            me: o.me,
            host: o.host,
            print: print_emoji(o.print),
            column: draw::column_width(o.players.len(), work_area(app).0),
            skip,
            host_name,
        }
    }

    /// 웹뷰: 내 펫이 결과 칸에 닿았다 — 내 진짜 펫도 한 번 반응한다 (만세 / 갸웃 + 결과 말풍선)
    pub fn react(&mut self, app: &AppHandle, own: &mut Slot) {
        let Some(view) = self.layout.as_ref() else { return };
        if self.reacted {
            return;
        }
        self.reacted = true;
        let slot = &view.results[end_column(view, view.me)];
        let good = match Mood::from_wire(&slot.mood) {
            Mood::Good => true,
            Mood::Bad => false,
            Mood::Neutral => return,
        };
        if own.pet.react_to_result(good) {
            let _ = app.emit_to(OWN_LABEL, "pet-say-text", format!("🎲 {}", slot.text));
        }
    }

    fn build_view(&self, ctx: &Ctx) -> Option<LadderView> {
        if self.showing_last {
            return Some(LadderView { view: "last", last: ctx.config.last_ladder.clone(), ..Default::default() });
        }
        let friends = |answers: &[(String, Answer)]| -> Vec<FriendRow> {
            ctx.config
                .friends
                .iter()
                .filter(|f| ctx.connected.contains(&f.id))
                .map(|f| {
                    let v = ctx.net.and_then(|n| n.peer_version(&f.id)).unwrap_or(0);
                    let answer = answers.iter().find(|(id, _)| *id == f.id).map_or("", |(_, a)| answer_label(a));
                    FriendRow { id: f.id.clone(), name: ctx.config.person_name(&f.id).unwrap_or(&f.name).to_string(), can_invite: v >= LADDER_MIN_VERSION, answer }
                })
                .collect()
        };
        let Some(game) = &self.game else {
            return self.setup.then(|| LadderView { view: "setup", host_name: ctx.my_name.to_string(), friends: friends(&[]), notice: self.notice, ..Default::default() });
        };
        let cancel = |reason: CancelReason, who: &Option<String>| CancelView { reason: reason.wire(), who: who.as_deref().map(shown_name) };
        Some(match (game, phase(game)) {
            (_, Phase::Done(_)) => LadderView { view: "game", game: self.layout.clone(), ..Default::default() },
            (_, Phase::Cancelled { reason, who }) => LadderView { view: "cancelled", cancel: Some(cancel(*reason, who)), ..Default::default() },
            (Game::Host(h), p) => {
                let answers = h.answers();
                let mut rows = friends(&answers);
                // 초대한 친구만 (연결이 끊긴 친구도 답과 함께 남긴다)
                rows.retain(|r| answers.iter().any(|(id, _)| *id == r.id));
                for (id, a) in &answers {
                    if !rows.iter().any(|r| r.id == *id) {
                        let name = ctx.config.person_name(id).map_or_else(|| id.clone(), str::to_string);
                        rows.push(FriendRow { id: id.clone(), name, can_invite: false, answer: answer_label(a) });
                    }
                }
                let view = if *p == Phase::Locked { "locked" } else { "inviting" };
                LadderView { view, host_name: ctx.my_name.to_string(), friends: rows, joined: h.joined(), all_answered: h.all_answered(), ..Default::default() }
            }
            (Game::Guest(g), p) => {
                let held = self.held_lock.as_ref().filter(|_| *p == Phase::Joined);
                let view = match p {
                    Phase::Invited => "invite",
                    Phase::Joined if held.is_some() => "confirm",
                    Phase::Joined => "joined",
                    _ => "locked",
                };
                let host_name = ctx.config.person_name(g.host_id()).map_or_else(|| shown_name(g.host_name()), str::to_string);
                LadderView { view, host_name, invited: g.invited(), draft: slot_rows(held.map_or(g.draft(), |(_, changed)| changed)), ..Default::default() }
            }
        })
    }

    /// 화면을 지금 상태에 맞춘다 (바뀌었을 때만). `focus` = 사용자가 직접 연 화면 (초대로 뜬 화면은 포커스를 뺏지 않는다)
    pub fn refresh(&mut self, app: &AppHandle, ctx: &Ctx, focus: bool) {
        self.refreshed = Some(Instant::now());
        let Some(view) = self.build_view(ctx) else {
            self.sent = None;
            return;
        };
        if !focus && self.sent.as_ref() == Some(&view) {
            return;
        }
        let (width, height) = match view.view {
            "setup" | "inviting" => SETUP_SIZE,
            "invite" | "confirm" => INVITE_SIZE,
            "last" => LAST_SIZE,
            "game" => {
                let g = view.game.as_ref().expect("game view");
                (g.column * g.players.len().max(4) as f64 + draw::WINDOW_MARGIN, GAME_HEIGHT)
            }
            _ => SMALL_SIZE,
        };
        let size = (width, draw::window_height(height, work_area(app).1));
        let resized = self.sent.as_ref().is_none_or(|s| s.view != view.view);
        *VIEW.lock().unwrap() = Some(view.clone());
        show(app, &view, size, focus, resized);
        self.sent = Some(view);
    }
}

/// 틱 사이에 바뀌었는지 볼 것: 단계와 (방장이면) 친구들의 답
fn state_key(g: &Game) -> (u8, [u8; 7]) {
    let step = match phase(g) {
        Phase::Inviting => 0,
        Phase::Invited => 1,
        Phase::Joined => 2,
        Phase::Locked => 3,
        Phase::Done(_) => 4,
        Phase::Cancelled { .. } => 5,
    };
    let answers = match g {
        Game::Host(h) => h.answer_counts(),
        Game::Guest(_) => [0; 7],
    };
    (step, answers)
}

fn phase(g: &Game) -> &Phase {
    match g {
        Game::Host(h) => h.phase(),
        Game::Guest(g) => g.phase(),
    }
}

/// 사다리 창을 보인다 (처음이면 만든다 — 게임 루프 스레드에서만). 포커스는 `focus`일 때만
fn show(app: &AppHandle, view: &LadderView, size: (f64, f64), focus: bool, resized: bool) {
    let window = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => match build_window(app) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create ladder window: {e}");
                return;
            }
        },
    };
    let _ = app.emit_to(LABEL, "ladder-state", view.clone());
    if resized {
        let _ = window.set_size(LogicalSize::new(size.0, size.1));
    }
    let hidden = !window.is_visible().unwrap_or(false);
    if hidden {
        crate::overlay::place_on_primary(app, &window, size);
    }
    // 작은 초대 창이 있던 자리에서 게임 창으로 커지면 아래·오른쪽이 화면 밖으로 나갈 수 있다
    if hidden || resized {
        keep_in_work_area(app, &window, size);
    }
    if hidden {
        let _ = window.show();
    }
    if focus {
        let _ = window.set_focus();
    }
}

/// 창이 지금 모니터의 작업 영역 안에 들어오게 옮긴다 (들어 있으면 사용자가 둔 자리 그대로). `inner` = 방금 정한 안쪽 크기
fn keep_in_work_area(app: &AppHandle, window: &WebviewWindow, inner: (f64, f64)) {
    let monitor = window.current_monitor().ok().flatten().or_else(|| app.primary_monitor().ok().flatten());
    let (Some(m), Ok(pos)) = (monitor, window.outer_position()) else { return };
    let (area, dpi) = (m.work_area(), m.scale_factor());
    // 숨긴 창이거나 크기 변경이 아직 안 끝났으면 바깥 크기가 작게 나올 수 있다 — 안쪽 크기보다 작게 잡지 않는다
    let outer = window.outer_size().map_or((0.0, 0.0), |s| (s.width as f64, s.height as f64));
    let x = draw::keep_on_screen(pos.x as f64, outer.0.max(inner.0 * dpi), area.position.x as f64, area.size.width as f64);
    let y = draw::keep_on_screen(pos.y as f64, outer.1.max(inner.1 * dpi), area.position.y as f64, area.size.height as f64);
    if (x as i32, y as i32) != (pos.x, pos.y) {
        let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
    }
}

fn hide(app: &AppHandle) {
    *VIEW.lock().unwrap() = None;
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}

/// 사다리 창: 작고 테두리 있는 창, 항상 위, **처음부터 포커스 없음**. 닫기(X)는 그만두기로 바꾼다 (창은 다시 쓴다)
fn build_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let form = crate::overlay::Form { label: LABEL, page: "ladder.html", title: "ScreenPet 사다리타기", size: SETUP_SIZE };
    let window = crate::overlay::form_builder(app, &form).focused(false).build()?;
    let handle = app.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            handle.state::<AppState>().send(Input::Net(NetInput::LadderClose));
        }
    });
    Ok(window)
}

fn from_ladder(window: &WebviewWindow) -> bool {
    window.label() == LABEL
}

/// 사다리 창의 지금 화면 (웹뷰 첫 로드용 — 사다리 창만)
#[tauri::command]
pub fn ladder_state(window: WebviewWindow) -> Option<LadderView> {
    if !from_ladder(&window) {
        return None;
    }
    VIEW.lock().unwrap().clone()
}

/// 설정 화면 "초대"
#[tauri::command]
pub fn ladder_host(window: WebviewWindow, state: tauri::State<AppState>, friends: Vec<String>, results: Vec<SlotRow>) {
    if from_ladder(&window) && friends.len() < draw::MAX_PLAYERS && results.len() <= draw::MAX_PLAYERS {
        state.send(Input::Net(NetInput::LadderHost { friends, results }));
    }
}

/// 방장 "시작" (최종 결과 칸)
#[tauri::command]
pub fn ladder_begin(window: WebviewWindow, state: tauri::State<AppState>, results: Vec<SlotRow>) {
    if from_ladder(&window) && results.len() <= draw::MAX_PLAYERS {
        state.send(Input::Net(NetInput::LadderBegin(results)));
    }
}

/// 초대 화면 "참가" / "안 할래요"
#[tauri::command]
pub fn ladder_answer(window: WebviewWindow, state: tauri::State<AppState>, join: bool) {
    if from_ladder(&window) {
        state.send(Input::Net(NetInput::LadderAnswer(join)));
    }
}

/// 창의 "닫기" / "그만두기"
#[tauri::command]
pub fn ladder_close(window: WebviewWindow, state: tauri::State<AppState>) {
    if from_ladder(&window) {
        state.send(Input::Net(NetInput::LadderClose));
    }
}

/// 내 펫이 결과 칸에 닿았다 (진짜 펫 반응)
#[tauri::command]
pub fn ladder_seen(window: WebviewWindow, state: tauri::State<AppState>) {
    if from_ladder(&window) {
        state.send(Input::Net(NetInput::LadderReact));
    }
}
