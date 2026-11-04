//! 물건 창 (`object-<n>`, object.html) — 물리와 규칙은 코어(screenpet_core::object), 여기는 창·메뉴·친구에게 던지기.
//! 물건은 대부분 잠들어 있다: 잠든 물건은 창도 웹뷰도 건드리지 않는다 (틱마다 바람 시계만 줄어든다).
//! 창 목록이 그대로면 세계도 다시 만들지 않고, 창 크기·화면 배율도 깨어 있는 물건만 다시 읽는다.
//! 받은 물건은 인증된 친구 링크에서 온 것만, 종류·움직임·글자를 코어가 다듬은 뒤에 쓴다 (글자는 웹뷰에서 textContent로만).
//! 💩 똥도 물건이다 (끌기·던지기·치우기·친구에게 넘기기·집중 중 되돌리기가 그대로): 메뉴로 못 꺼내고, 수는 따로 10개까지, 30분이면 스르륵 사라지고, 빗자루에 쓸린다.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use screenpet_core::config::Config;
use screenpet_core::geom::{Rect, WindowInfo};
use screenpet_core::object::{accept_toss, AcceptedToss, Exit, Object, ObjectKind, Origin, Postit, Release, Roster, TossBudget, TossInbox, MAX_OWN_OBJECTS, MAX_POOPS, OBJECT_LOGICAL_SIZE};
use screenpet_core::brain::Pet;
use screenpet_core::broom::Broom;
use screenpet_core::social::Social;
use screenpet_core::poop::PoopSpot;
use screenpet_core::tumble::shape_of;
use screenpet_core::platform::WindowSource;
use screenpet_core::visit::{self, Side, MAX_NOTE_CHARS};
use screenpet_core::world::World;
use screenpet_net::Toss;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry};

use crate::game_loop::Input;
use crate::network::{from_net_fling, from_net_side, to_net_fling, to_net_side, Network};
use crate::AppState;

const PREFIX: &str = "object-";
/// 포스트잇 쓰기 창 라벨
pub const POSTIT_LABEL: &str = "postit";
const POSTIT_SIZE: (f64, f64) = (300.0, 176.0);
/// 포스트잇을 읽는 동안의 창: 너비, 위로 더하는 자리 (논리 px)
const NOTE_WIDTH: f64 = 232.0;
const NOTE_SPACE: f64 = 84.0;
/// 뿅 사라지는 동안 기다렸다 닫는다 (object.html poof 0.3초)
const POOF: Duration = Duration::from_millis(400);
/// 똥이 스르륵 사라지는 동안 기다렸다 닫는다 (object.html fade 1.5초)
const FADE: Duration = Duration::from_millis(1600);
/// 앉은 창이 움직인 뒤 이만큼은 매 틱 따라간다 (사이엔 창 목록을 100 ms마다 읽을 때만 본다)
const RIDE_FOLLOW: Duration = Duration::from_secs(1);

const MENU_SPAWN: &str = "object:spawn:";
const MENU_CLEAR: &str = "object:clear";
const MENU_NOTE: &str = "object:note:";
const MENU_PEEL: &str = "object:peel:";
const MENU_REMOVE: &str = "object:remove:";

/// `물건 ▸` 하위 메뉴 (글자에 내 물건 수)
static MENU: OnceLock<Submenu<Wry>> = OnceLock::new();
/// 창 라벨 → 포스트잇 상태 (우클릭 메뉴는 게임 루프 밖에서 만든다). 글자는 없다
static POSTITS: Mutex<Option<HashMap<String, Postit>>> = Mutex::new(None);

fn set_postit_state(label: &str, postit: Option<Postit>) {
    let mut map = POSTITS.lock().unwrap();
    let map = map.get_or_insert_with(HashMap::new);
    match postit {
        Some(p) => {
            map.insert(label.to_string(), p);
        }
        None => {
            map.remove(label);
        }
    }
}

fn menu_label(own: usize) -> String {
    format!("물건 ({own}/{MAX_OWN_OBJECTS})")
}

/// 게임 루프로 들어오는 물건 입력
pub enum ObjectInput {
    Spawn(ObjectKind),
    ClearAll,
    /// 창의 물건을 눌렀다
    Down { label: String },
    Remove { label: String },
    /// 우클릭 "포스트잇 붙이기…"
    EditPostit { label: String },
    /// 우클릭 "포스트잇 떼기"
    PeelPostit { label: String },
    /// 포스트잇 창 "붙이기"
    SubmitPostit(String),
}

/// 웹뷰가 그릴 모습 (`object-pose`)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct PosePayload {
    angle: f64,
    /// 포스트잇을 읽느라 넓어진 창이 화면 끝에서 밀렸을 때 그림을 옮기는 거리 (논리 px)
    dx: f64,
}

struct Live {
    id: u64,
    label: String,
    object: Object,
    window: WebviewWindow,
    last_pos: Option<(i32, i32)>,
    last_pose: PosePayload,
    note_open: bool,
    need_show: bool,
}

impl Live {
    /// 창은 도는 축을 가운데에 두는 정사각형 (어떤 각도로 돌아도 그림이 잘리지 않는다, 종류마다 크기가 다르다).
    /// 쪽지를 읽는 동안엔 넓어지고 위로 자리가 더 생긴다 — 축은 아래쪽 정사각형의 가운데 그대로
    fn logical_size(&self) -> (f64, f64) {
        let side = shape_of(self.object.kind).window_side();
        if self.note_open {
            (NOTE_WIDTH, side + NOTE_SPACE)
        } else {
            (side, side)
        }
    }

    /// 창을 몸 자리로, 처음이면 보인다. 달라졌을 때만 OS에 알린다
    fn place(&mut self, world: &World, hidden: bool) {
        let dpi = self.object.dpi();
        let (w, h) = self.logical_size();
        let (cx, cy) = self.object.pivot();
        let mut x = cx - w * dpi / 2.0;
        if self.note_open {
            // 읽는 동안: 쪽지가 화면 밖으로 잘리지 않게 창만 안쪽으로 민다
            let wa = world.work_area_at(cx, self.object.body.y);
            x = x.clamp(wa.left, (wa.right - w * dpi).max(wa.left));
        }
        let side = shape_of(self.object.kind).window_side();
        let pos = (x.round() as i32, (cy + (side / 2.0 - h) * dpi).round() as i32);
        if self.last_pos != Some(pos) {
            let _ = self.window.set_position(PhysicalPosition::new(pos.0, pos.1));
            self.last_pos = Some(pos);
        }
        if self.need_show && !hidden {
            let _ = self.window.show();
            self.need_show = false;
        }
    }

    fn pose(&self) -> PosePayload {
        let dpi = self.object.dpi();
        let (w, _) = self.logical_size();
        let dx = self.last_pos.map_or(0.0, |(x, _)| (self.object.pivot().0 - (x as f64 + w * dpi / 2.0)) / dpi);
        let dx = if self.note_open { dx } else { 0.0 };
        PosePayload { angle: self.object.angle(), dx }
    }

    fn set_note_open(&mut self, open: bool) {
        if self.note_open == open {
            return;
        }
        self.note_open = open;
        let (w, h) = self.logical_size();
        let _ = self.window.set_size(LogicalSize::new(w, h));
        self.last_pos = None;
    }
}

/// 창을 기다리는 똥 (틱마다 하나씩 — 창 만들기는 메인 스레드를 기다린다). 받은 쓰레기 줄과 따로 세어 서로 밀어내지 않는다
enum WaitingPoop {
    /// 동물이 쌌다
    Dropped { kind: ObjectKind, x: f64, y: f64, scale: f64 },
    /// 친구가 던졌다
    Tossed(AcceptedToss, Side, f64),
}

pub struct Objects {
    next: u64,
    live: Vec<Live>,
    roster: Roster,
    /// 받았지만 아직 창을 만들지 않은 물건 (틱마다 하나씩)
    inbox: TossInbox<(AcceptedToss, Side)>,
    poop_queue: VecDeque<WaitingPoop>,
    fading: Vec<(WebviewWindow, Instant)>,
    /// 물건이 있을 때만 맞춘다 (물건 키에 맞춘 세계)
    world: Option<World>,
    /// 친구별 던지기 예산 (받는 쪽 net의 친구별 제한과 같다 — 넘치면 받는 쪽이 버리므로 미리 튕긴다)
    budgets: HashMap<String, TossBudget>,
    /// 친구별 마지막으로 던진 때 (되돌아온 물건이 정말 내 것인지 본다)
    threw_at: HashMap<String, Instant>,
    started: Instant,
    ride_until: Instant,
    frame: u64,
    /// 포스트잇 창이 다루는 물건
    posting: Option<String>,
    shown_own: usize,
}

impl Objects {
    pub fn new() -> Objects {
        let now = Instant::now();
        Objects {
            next: 0,
            live: Vec::new(),
            roster: Roster::default(),
            inbox: TossInbox::default(),
            poop_queue: VecDeque::new(),
            fading: Vec::new(),
            world: None,
            budgets: HashMap::new(),
            threw_at: HashMap::new(),
            started: now,
            ride_until: now,
            frame: 0,
            posting: None,
            shown_own: 0,
        }
    }

    /// 끌고 있는 물건이 있다 (동물을 동시에 잡거나 간질이지 않는다)
    pub fn is_held(&self) -> bool {
        self.live.iter().any(|l| l.object.is_held())
    }

    fn find(&mut self, label: &str) -> Option<&mut Live> {
        self.live.iter_mut().find(|l| l.label == label)
    }

    /// busy = 동물을 잡고 있거나 음식을 들고 있음 (그동안의 누름은 무시). base = 지금 세계, cursor = 커서
    pub fn handle(&mut self, app: &AppHandle, input: ObjectInput, base: &World, cursor: (f64, f64), busy: bool) {
        match input {
            ObjectInput::Spawn(kind) => self.spawn_own(app, kind, base, cursor),
            ObjectInput::ClearAll => {
                self.roster.clear();
                self.inbox.clear();
                self.poop_queue.clear();
                for l in std::mem::take(&mut self.live) {
                    self.poof(l);
                }
            }
            ObjectInput::Remove { label } => self.remove(&label),
            ObjectInput::Down { label } => {
                if !busy && !self.is_held() {
                    if let Some(l) = self.find(&label) {
                        l.object.pointer_down(cursor.0, cursor.1);
                    }
                }
            }
            ObjectInput::EditPostit { label } => {
                if self.find(&label).is_some_and(|l| l.object.postit() == Postit::None) {
                    self.posting = Some(label);
                    show_postit_window(app);
                }
            }
            ObjectInput::PeelPostit { label } => {
                if let Some(l) = self.find(&label) {
                    if l.object.peel_note() {
                        set_postit_state(&l.label, Some(l.object.postit()));
                        let _ = app.emit_to(l.label.as_str(), "object-sticker", false);
                    }
                }
            }
            ObjectInput::SubmitPostit(text) => {
                let Some(label) = self.posting.take() else { return };
                if let Some(l) = self.find(&label) {
                    if l.object.attach_note(&text) {
                        set_postit_state(&l.label, Some(l.object.postit()));
                        let _ = app.emit_to(l.label.as_str(), "object-sticker", true);
                    }
                }
            }
        }
        self.sync_menu();
    }

    /// 메뉴에서 만들기: 커서 근처(트레이에서 눌렀으면 그 위)에서 떨어진다. 넘치면 가장 오래된 내 물건이 뿅
    fn spawn_own(&mut self, app: &AppHandle, kind: ObjectKind, base: &World, cursor: (f64, f64)) {
        let id = self.next_id();
        let Some(window) = build_window(app, id, kind, false, 1.0) else { return };
        let dpi = window.scale_factor().unwrap_or(1.0);
        let world = self.world_for(base, dpi);
        let b = world.bounds;
        let size = OBJECT_LOGICAL_SIZE * dpi;
        let x = cursor.0.clamp(b.left + size / 2.0, (b.right - size / 2.0).max(b.left + size / 2.0));
        let y = (cursor.1 - size / 2.0).clamp(b.top + size, b.bottom.max(b.top + size));
        let object = Object::new(kind, x, y, dpi, fastrand::u64(..));
        self.admit(id, window, object, Origin::Own);
    }

    /// 동물이 쌌다: (x, y)에서 떨어지기 시작한다. 창은 tick이 만든다
    pub fn drop_poop(&mut self, kind: ObjectKind, x: f64, y: f64, scale: f64) {
        self.queue_poop(WaitingPoop::Dropped { kind, x, y, scale });
    }

    fn queue_poop(&mut self, poop: WaitingPoop) {
        if self.poop_queue.len() >= MAX_POOPS {
            self.poop_queue.pop_front();
        }
        self.poop_queue.push_back(poop);
    }

    /// 바닥에 놓인 똥 자리를 out에 다시 채운다 (동물이 알아챈다, 틱마다 할당하지 않는다)
    pub fn poop_spots(&self, out: &mut Vec<PoopSpot>) {
        out.clear();
        out.extend(self.live.iter().filter_map(|l| l.object.poop_spot(l.id)));
    }

    /// 🧹 빗자루가 지나간 똥은 빗자루 쪽으로 밀려 날아간다 (평소의 던지기 — 친구가 연결된 끝을 넘으면 tick이 친구에게 보낸다)
    pub fn sweep_poops(&mut self, broom: &Broom) {
        let (vx, vy) = broom.velocity();
        for l in self.live.iter_mut().filter(|l| l.object.kind.is_poop()) {
            if broom.passes_over(l.object.body.x, l.object.body.y, l.object.dpi()) {
                l.object.sweep(vx, vy);
            }
        }
    }

    /// 던져서 날아가는 물건이 동물에 부딪힌다 (같은 화면만 — 판정과 쿨다운은 코어 social.rs, 통신할 것이 없다)
    pub fn hit_pets<T: AsRef<Pet> + AsMut<Pet>>(&mut self, social: &mut Social, dt: f64, pets: &mut [T]) {
        for l in &mut self.live {
            social.object_hits(l.id, &mut l.object, dt, pets);
        }
    }

    /// 친구가 던진 물건 (인증된 친구 링크 — net이 확인 — 에서 온 내 친구의 것만). 받은 물건은 기다리는 것까지 20개, 넘치면 가장 오래된 것부터 치운다.
    /// 여기서는 줄만 세운다 — 창은 tick이 하나씩 만든다 (한꺼번에 오면 게임 루프가 멈추지 않게)
    pub fn receive(&mut self, config: &Config, from: &str, toss: Toss) {
        if config.friend(from).is_none() {
            return;
        }
        let fling = toss.fling.map(from_net_fling);
        let Some(accepted) = accept_toss(&toss.kind, fling, toss.note.as_deref()) else {
            eprintln!("ignored a thrown object of unknown kind from {from}");
            return;
        };
        let side = from_net_side(toss.side, toss.vside);
        if accepted.kind.is_poop() {
            // 메뉴로 꺼도 친구가 던진 똥은 받는다 (끄는 건 동물이 싸는 것뿐, 받기 싫으면 집중·하우스)
            self.queue_poop(WaitingPoop::Tossed(accepted, side, screenpet_core::poop::sanitize_size(toss.scale)));
            return;
        }
        if let Some(old) = self.inbox.push((accepted, side), &mut self.roster) {
            if let Some(i) = self.live.iter().position(|l| l.id == old) {
                let l = self.live.remove(i);
                self.poof(l);
            }
        }
    }

    /// 방금 그 친구에게 물건을 던졌다
    pub fn threw_lately(&self, friend: &str) -> bool {
        self.threw_at.get(friend).is_some_and(|t| t.elapsed() < BOUNCE_WINDOW)
    }

    /// 줄의 맨 앞 물건 하나의 창을 만든다 (틱마다)
    fn admit_next_toss(&mut self, app: &AppHandle, base: &World) {
        if let Some(poop) = self.poop_queue.pop_front() {
            self.admit_poop(app, base, poop);
            return;
        }
        let Some((accepted, side)) = self.inbox.pop() else { return };
        let id = self.next_id();
        let Some(window) = build_window(app, id, accepted.kind, accepted.note.is_some(), 1.0) else { return };
        let dpi = window.scale_factor().unwrap_or(1.0);
        let world = self.world_for(base, dpi);
        let mut object = Object::flown_in(accepted.kind, world, side, accepted.fling, dpi, fastrand::u64(..));
        object.receive_note(accepted.note);
        self.admit(id, window, object, Origin::Foreign);
    }

    fn admit_poop(&mut self, app: &AppHandle, base: &World, poop: WaitingPoop) {
        let id = self.next_id();
        let kind = match &poop {
            WaitingPoop::Dropped { kind, .. } => *kind,
            WaitingPoop::Tossed(accepted, ..) => accepted.kind,
        };
        let scale = match &poop {
            WaitingPoop::Dropped { scale, .. } | WaitingPoop::Tossed(_, _, scale) => *scale,
        };
        let Some(window) = build_window(app, id, kind, false, scale) else { return };
        let dpi = window.scale_factor().unwrap_or(1.0);
        let world = self.world_for(base, dpi);
        let object = match poop {
            WaitingPoop::Dropped { kind, x, y, .. } => {
                let b = world.bounds;
                let margin = OBJECT_LOGICAL_SIZE * dpi / 2.0;
                Object::new(kind, x.clamp(b.left + margin, (b.right - margin).max(b.left + margin)), y, dpi, fastrand::u64(..))
            }
            WaitingPoop::Tossed(accepted, side, _) => Object::flown_in(accepted.kind, world, side, accepted.fling, dpi, fastrand::u64(..)),
        };
        let mut object = object;
        object.set_scale(scale);
        self.admit(id, window, object, Origin::Poop);
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    fn world_for(&mut self, base: &World, dpi: f64) -> &World {
        self.world.get_or_insert_with(|| World::new(base.windows.clone(), base.work_areas.clone(), OBJECT_LOGICAL_SIZE * dpi))
    }

    /// 물건 수를 지키며 들인다: 넘치면 그 쪽(내 것 10 / 받은 것 20 / 똥 10)의 가장 오래된 것이 뿅 사라진다
    fn admit(&mut self, id: u64, window: WebviewWindow, object: Object, origin: Origin) {
        if let Some(old) = self.roster.add(id, origin) {
            if let Some(i) = self.live.iter().position(|l| l.id == old) {
                let l = self.live.remove(i);
                self.poof(l);
            }
        }
        let label = window.label().to_string();
        set_postit_state(&label, Some(object.postit()));
        self.live.push(Live {
            id,
            label,
            object,
            window,
            last_pos: None,
            last_pose: PosePayload { angle: 0.0, dx: 0.0 },
            note_open: false,
            need_show: true,
        });
    }

    fn remove(&mut self, label: &str) {
        if let Some(i) = self.live.iter().position(|l| l.label == label) {
            let l = self.live.remove(i);
            self.roster.remove(l.id);
            self.poof(l);
        }
    }

    fn poof(&mut self, l: Live) {
        set_postit_state(&l.label, None);
        let _ = l.window.eval("document.body.classList.add('poof')");
        self.fading.push((l.window, Instant::now() + POOF));
    }

    /// 다 된 똥이 스르륵 사라진다
    fn fade(&mut self, l: Live) {
        set_postit_state(&l.label, None);
        let _ = l.window.eval("document.body.classList.add('fade')");
        self.fading.push((l.window, Instant::now() + FADE));
    }

    /// `물건 ▸` 글자의 내 물건 수 (바뀔 때만)
    fn sync_menu(&mut self) {
        let own = self.roster.own_count();
        if own != self.shown_own {
            self.shown_own = own;
            if let Some(m) = MENU.get() {
                let _ = m.set_text(menu_label(own));
            }
        }
    }

    /// 창 목록을 새로 읽었을 때 (100 ms마다). 달라진 게 없으면 할 일이 없다 — 물건은 계속 잔다
    pub fn refresh(&mut self, windows: &[WindowInfo], work_areas: &[Rect], now: Instant) {
        if self.live.is_empty() {
            self.world = None;
            return;
        }
        // 깨어 있는 물건만 화면 배율을 다시 읽는다 (창에 묻는 일은 메인 스레드를 거친다)
        for l in self.live.iter_mut().filter(|l| !l.object.is_asleep()) {
            if let Ok(dpi) = l.window.scale_factor() {
                l.object.set_dpi(dpi);
            }
        }
        let same = self.world.as_ref().is_some_and(|w| w.windows == windows && w.work_areas == work_areas);
        if same {
            return;
        }
        let dpi = self.live.first().map_or(1.0, |l| l.object.dpi());
        self.world = Some(World::new(windows.to_vec(), work_areas.to_vec(), OBJECT_LOGICAL_SIZE * dpi));
        self.reseat_all(now);
    }

    /// 세계가 바뀌었다: 앉은 창을 따라가거나 떨어지기 시작한다. 창을 따라 움직였으면 잠시 매 틱 따라간다
    fn reseat_all(&mut self, now: Instant) {
        let Some(world) = self.world.as_ref() else { return };
        for l in &mut self.live {
            if l.object.reseat(world) && l.object.ridden_window().is_some() {
                self.ride_until = now + RIDE_FOLLOW;
            }
        }
    }

    /// 앉은 창이 움직이는 중이면 그 창의 경계만 새로 읽는다 (창을 끌어 옮길 때 100 ms 계단이 아니게)
    fn follow_rides(&mut self, source: &mut dyn WindowSource, now: Instant) {
        if now >= self.ride_until {
            return;
        }
        let mut ids: Vec<u64> = self.live.iter().filter_map(|l| l.object.ridden_window()).collect();
        ids.sort_unstable();
        ids.dedup();
        let rects: Vec<(u64, Rect)> = ids.into_iter().filter_map(|id| source.rect_of(id).map(|r| (id, r))).collect();
        if self.world.as_mut().is_some_and(|w| w.patch_windows(&rects)) {
            self.reseat_all(now);
        }
    }

    /// 매 틱 (게임 루프)
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, app: &AppHandle, base: &World, dt: f64, now: Instant, cursor: (f64, f64), left_down: bool, away: bool, source: &mut dyn WindowSource, network: &Network, config: &Config) {
        self.fading.retain(|(w, at)| {
            if now >= *at {
                let _ = w.destroy();
                false
            } else {
                true
            }
        });
        self.admit_next_toss(app, base);
        if self.live.is_empty() {
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        self.follow_rides(source, now);
        let hidden = crate::windows_hidden(app);

        // 끌기·놓기
        if let Some(l) = self.live.iter_mut().find(|l| l.object.is_held()) {
            if left_down {
                l.object.pointer_move(cursor.0, cursor.1);
            } else if l.object.pointer_up() == Release::Click {
                // 받은 포스트잇을 눌렀다: 글자는 이때 처음 웹뷰로 가고, 코어에서는 지워진다
                if let Some(text) = l.object.read_note() {
                    l.set_note_open(true);
                    set_postit_state(&l.label, Some(l.object.postit()));
                    let _ = app.emit_to(l.label.as_str(), "object-sticker", false);
                    let _ = app.emit_to(l.label.as_str(), "object-note", text);
                }
            }
        }

        // 던져 넘길 수 있는 끝은 날아가는 물건이 있을 때만 계산한다
        let secs = now.duration_since(self.started).as_secs_f64();
        let open = {
            let budgets = &self.budgets;
            move |id: &str| network.is_linked(id) && budgets.get(id).is_none_or(|b| b.can_spend(secs))
        };
        let edges = if self.live.iter().all(|l| l.object.is_asleep()) { Default::default() } else { config.exit_edges(&open) };

        let Some(world) = self.world.as_ref() else { return };
        let mut exits: Vec<(usize, Exit)> = Vec::new();
        for (i, l) in self.live.iter_mut().enumerate() {
            let was_asleep = l.object.is_asleep();
            if let Some(exit) = l.object.tick(dt, world, edges, screenpet_core::object::gusts_ok(hidden, away)) {
                exits.push((i, exit));
                continue;
            }
            if l.object.take_note_ended() {
                // 10초 끝: 글자를 지우고 창을 줄인다 — 다시 볼 수 없다
                let _ = app.emit_to(l.label.as_str(), "object-note-end", ());
                l.set_note_open(false);
                set_postit_state(&l.label, Some(l.object.postit()));
            }
            let asleep = l.object.is_asleep();
            // 잠든 물건은 창이 제자리에 있으면 아무것도 하지 않는다 (reseat가 옮겼으면 last_pos와 달라진다)
            l.place(world, hidden);
            if asleep && was_asleep && l.last_pose == l.pose() {
                continue;
            }
            // 모습은 30 Hz면 충분하다 (잠들 때의 마지막 모습은 꼭 보낸다)
            let pose = l.pose();
            if pose != l.last_pose && (self.frame % 2 == 0 || asleep) {
                l.last_pose = pose;
                let _ = app.emit_to(l.label.as_str(), "object-pose", pose);
            }
        }
        for (i, exit) in exits.into_iter().rev() {
            let l = self.live.remove(i);
            self.roster.remove(l.id);
            self.toss_away(&l, exit, secs, network, config);
            set_postit_state(&l.label, None);
            let _ = l.window.destroy();
        }
        let mut i = 0;
        while i < self.live.len() {
            if self.live[i].object.expired() {
                let l = self.live.remove(i);
                self.roster.remove(l.id);
                self.fade(l);
            } else {
                i += 1;
            }
        }
        self.sync_menu();
    }

    /// 물건이 side 끝을 넘어갔다: 그 끝의 연결된 친구 하나에게 날려 보낸다 (친구 쪽에서 반대편 끝으로 들어온다). 돌아오지 않는다
    fn toss_away(&mut self, l: &Live, exit: Exit, secs: f64, network: &Network, config: &Config) {
        let friends = {
            let budgets = &self.budgets;
            config.friends_at(exit.side, |id| network.is_linked(id) && budgets.get(id).is_none_or(|b| b.can_spend(secs)))
        };
        if friends.is_empty() {
            return;
        }
        let friend = friends[fastrand::usize(..friends.len())].id.clone();
        self.budgets.entry(friend.clone()).or_default().spend(secs);
        self.threw_at.insert(friend.clone(), Instant::now());
        let (side, vside) = to_net_side(visit::opposite(exit.side));
        let toss = Toss { kind: l.object.kind.id().to_string(), side, vside, fling: Some(to_net_fling(exit.fling)), note: l.object.note_to_send(), bounced: false, scale: l.object.kind.is_poop().then(|| l.object.scale()) };
        eprintln!("threw a {} to {friend}{}", l.object.kind.id(), if toss.note.is_some() { " with a post-it" } else { "" });
        network.toss_to(&friend, toss);
    }
}

/// 던지고 이만큼 안에 되돌아온 물건만 내 것으로 본다
const BOUNCE_WINDOW: Duration = Duration::from_secs(30);

fn build_window(app: &AppHandle, id: u64, kind: ObjectKind, sticker: bool, scale: f64) -> Option<WebviewWindow> {
    let label = format!("{PREFIX}{id}");
    let shape = shape_of(kind);
    let (w, h) = (shape.window_side(), shape.window_side());
    let (px, py) = shape.pivot_logical();
    // kind.id()와 숫자는 코어 목록의 값뿐이다 (친구가 보낸 글자가 아니다)
    let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("object.html".into()))
        .title("ScreenPet Object")
        .initialization_script(&format!("window.OBJECT = {{ kind: {:?}, sticker: {sticker}, side: {w}, pivot: [{px}, {py}], scale: {scale} }}; {}", kind.id(), crate::art::style_script()))
        .inner_size(w, h)
        .accept_first_mouse(true)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .build()
        .inspect(crate::overlay::float_everywhere);
    match built {
        Ok(window) => {
            // 다른 배율의 모니터에서도 논리 크기를 맞춘다
            let _ = window.set_size(LogicalSize::new(w, h));
            Some(window)
        }
        Err(e) => {
            eprintln!("failed to create object window: {e}");
            None
        }
    }
}

/// `물건 ▸` 하위 메뉴 (tray::build가 부른다)
pub fn build_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let sub = Submenu::new(app, menu_label(0), true)?;
    // 똥은 메뉴로 꺼내지 않는다 (동물이 싼다)
    for kind in ObjectKind::TRASH {
        sub.append(&MenuItem::with_id(app, format!("{MENU_SPAWN}{}", kind.id()), kind.label(), true, None::<&str>)?)?;
    }
    sub.append(&PredefinedMenuItem::separator(app)?)?;
    sub.append(&MenuItem::with_id(app, MENU_CLEAR, "🧹 쓰레기 다 치우기", true, None::<&str>)?)?;
    let _ = MENU.set(sub.clone());
    Ok(sub)
}

/// 물건 메뉴 항목이면 처리하고 true (트레이의 메뉴 처리기에서 먼저 부른다)
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    let input = if id == MENU_CLEAR {
        ObjectInput::ClearAll
    } else if let Some(kind) = id.strip_prefix(MENU_SPAWN) {
        let Some(kind) = ObjectKind::from_id(kind) else { return true };
        ObjectInput::Spawn(kind)
    } else if let Some(label) = id.strip_prefix(MENU_NOTE) {
        ObjectInput::EditPostit { label: label.to_string() }
    } else if let Some(label) = id.strip_prefix(MENU_PEEL) {
        ObjectInput::PeelPostit { label: label.to_string() }
    } else if let Some(label) = id.strip_prefix(MENU_REMOVE) {
        ObjectInput::Remove { label: label.to_string() }
    } else {
        return false;
    };
    let _ = app.state::<AppState>().tx.lock().unwrap().send(Input::Object(input));
    true
}

fn is_object_label(label: &str) -> bool {
    label.starts_with(PREFIX)
}

/// 물건을 눌렀다 (왼쪽 버튼) — 부른 창의 라벨로 어느 물건인지 찾는다
#[tauri::command]
pub fn object_down(window: WebviewWindow, state: State<AppState>) {
    if is_object_label(window.label()) {
        let _ = state.tx.lock().unwrap().send(Input::Object(ObjectInput::Down { label: window.label().to_string() }));
    }
}

/// 물건 우클릭 메뉴. show_menu처럼 async — 모달 메뉴 루프가 WebView2 IPC 콜백 안에서 돌지 않게
#[tauri::command]
pub async fn object_menu(window: tauri::Window) -> Result<(), String> {
    let label = window.label().to_string();
    if !is_object_label(&label) {
        return Ok(());
    }
    let postit = POSTITS.lock().unwrap().as_ref().and_then(|m| m.get(&label).copied()).unwrap_or_default();
    let app = window.app_handle().clone();
    let build = || -> tauri::Result<Menu<Wry>> {
        let note = match postit {
            // 똥엔 포스트잇이 없다
            Postit::Never => None,
            Postit::Mine => Some(MenuItem::with_id(&app, format!("{MENU_PEEL}{label}"), "📝 포스트잇 떼기", true, None::<&str>)?),
            // 친구가 붙인 포스트잇은 덮어쓰거나 뗄 수 없다 (읽는 중에도)
            other => Some(MenuItem::with_id(&app, format!("{MENU_NOTE}{label}"), "📝 포스트잇 붙이기…", other == Postit::None, None::<&str>)?),
        };
        let remove = MenuItem::with_id(&app, format!("{MENU_REMOVE}{label}"), "🗑️ 이것만 치우기", true, None::<&str>)?;
        let clear = MenuItem::with_id(&app, MENU_CLEAR, "🧹 쓰레기 다 치우기", true, None::<&str>)?;
        match &note {
            Some(note) => Menu::with_items(&app, &[note, &remove, &clear]),
            None => Menu::with_items(&app, &[&remove, &clear]),
        }
    };
    let menu = build().map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

/// 포스트잇 쓰기 창을 보이고 앞으로 (처음이면 만든다 — 게임 루프 스레드에서만). 닫기(X)는 숨기기로 바꾼다
fn show_postit_window(app: &AppHandle) {
    let window = match app.get_webview_window(POSTIT_LABEL) {
        Some(w) => w,
        None => {
            let built = WebviewWindowBuilder::new(app, POSTIT_LABEL, WebviewUrl::App("postit.html".into()))
                .title("ScreenPet 포스트잇")
                .inner_size(POSTIT_SIZE.0, POSTIT_SIZE.1)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .always_on_top(true)
                .skip_taskbar(false)
                .visible(false)
                .build();
            match built {
                Ok(w) => {
                    let handle = w.clone();
                    w.on_window_event(move |ev| {
                        if let WindowEvent::CloseRequested { api, .. } = ev {
                            api.prevent_close();
                            let _ = handle.hide();
                        }
                    });
                    w
                }
                Err(e) => {
                    eprintln!("failed to create post-it window: {e}");
                    return;
                }
            }
        }
    };
    let _ = app.emit_to(POSTIT_LABEL, "postit-open", MAX_NOTE_CHARS);
    if !window.is_visible().unwrap_or(false) {
        let _ = window.center();
        let _ = window.show();
    }
    let _ = window.set_focus();
}

/// 포스트잇 창 최대 글자 수 (첫 로드용)
#[tauri::command]
pub fn postit_max() -> usize {
    MAX_NOTE_CHARS
}

/// 포스트잇 창 "붙이기": 게임 루프로 보내고 창을 숨긴다 (다듬기·80자 제한은 코어가 한다)
#[tauri::command]
pub fn postit_submit(window: WebviewWindow, state: State<AppState>, text: String) {
    if window.label() != POSTIT_LABEL {
        return;
    }
    let text: String = text.chars().take(256).collect();
    let _ = state.tx.lock().unwrap().send(Input::Object(ObjectInput::SubmitPostit(text)));
    let _ = window.hide();
}

/// 포스트잇 창 "닫기"
#[tauri::command]
pub fn postit_close(window: WebviewWindow) {
    if window.label() == POSTIT_LABEL {
        let _ = window.hide();
    }
}
