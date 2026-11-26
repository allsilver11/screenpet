//! 🔎 "엥 왜 이래?" 탐지기 — 번들 동물 모두를 한 세상(모니터 한둘, 창 몇 개)에 풀어 놓고 게임 루프(src-tauri game_loop.rs)와
//! 같은 순서로 오래 돌림. 창 옮기기·닫기·열기, 물건·똥·거대한 것, 먹이, 잡아 끌기·던지기, 종 바꾸기, 조작 모드, 하우스, 집중,
//! 자리 비움을 무작위로 섞고, 틱마다 화면에서 이상해 보일 것을 살핌:
//! 사라지거나 화면 밖·작업표시줄 뒤에 오래 있음 · pet.json에 없는 그림 (웹뷰가 조용히 idle로 바꿔 "갑자기 다른 그림") ·
//! 너무 오래가는 상태 · 제자리 달리기 · 엉뚱한 동물의 마법 효과 (예: 디멘터였던 동물의 서리) · 말풍선이 없는 대사 · 뿅·가방 흐림이 오래 남음.
//! 실패하면 seed·틱·동물·최근 상태 기록을 찍음. 긴 판은 `cargo test -- --ignored qa_sim`

use std::collections::{HashMap, HashSet, VecDeque};

use crate::brain::control_moves::ControlInput;
use crate::brain::portkey_moves::PortkeyEvent;
use crate::brain::{hoist_moves, Pet, PetConfig, State};
use crate::care::{Care, Food, Stage};
use crate::geom::{Rect, WindowInfo};
use crate::juggle::{JuggleEvent, ObjectSpot, Target};
use crate::magic::MagicFx;
use crate::manifest::{parse_manifest, PetManifest};
use crate::object::{Object, ObjectKind};
use crate::play::ObjectPlay;
use crate::poop::PoopSpot;
use crate::portkey::{PortkeyKey, PortkeySpot};
use crate::presence::{DailyMarks, FocusLength, LocalMoment, Rhythm, RhythmInput, RhythmPet};
use crate::social::Social;
use crate::surfaces::Owner;
use crate::taskbar::Toys;
use crate::test_util::DT;
use crate::visit::ExitEdges;
use crate::web::WebSpot;
use crate::world::World;

/// 세계(창 목록)를 다시 읽는 틱 간격 (앱 WORLD_REFRESH 100ms)
const REFRESH_TICKS: u64 = 6;
/// 한 판에 동시에 놓인 물건 수 한도
const MAX_OBJECTS: usize = 8;
/// 기록해 두는 최근 상태 바뀜 수
const HISTORY: usize = 40;

/// 번들 동물 한 종: pet.json과 말풍선이 있는 대사 이름들
struct Species {
    id: String,
    manifest: PetManifest,
    /// 이 동물이 말할 수 있는 대사 (pet.json speech·stage_speech + main.js·common.js 기본 대사) — 단계 이름 → 대사 이름
    speech: HashMap<&'static str, HashSet<String>>,
}

/// ui/*.js 안 `const NAME = {` 객체의 키들 (안쪽 객체의 키도 모두 — 넉넉하게)
fn js_keys(src: &str, decl: &str) -> HashSet<String> {
    let start = src.find(decl).unwrap_or_else(|| panic!("{decl} not found"));
    let mut out = HashSet::new();
    for line in src[start + decl.len()..].lines() {
        if line.starts_with("};") {
            break;
        }
        let t = line.trim_start();
        if let Some((key, _)) = t.split_once(':') {
            let key = key.trim().trim_matches('"');
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                out.insert(key.to_string());
            }
        }
    }
    out
}

/// 모든 번들 동물 (이름순)
fn all_species() -> Vec<Species> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
    let main = std::fs::read_to_string(root.join("main.js")).unwrap();
    let common = std::fs::read_to_string(root.join("common.js")).unwrap();
    let mut defaults = js_keys(&main, "const EXTRA_SPEECH = {");
    defaults.extend(js_keys(&common, "const RHYTHM_SPEECH = {"));
    // 커서 장난 대사는 동물마다 CURSOR_LINES에 없으면 "!"
    defaults.insert("cursor_play".into());
    let stage_src = &main[main.find("const STAGE_SPEECH = {").expect("STAGE_SPEECH")..];
    crate::test_util::species()
        .into_iter()
        .map(|id| {
            let path = root.join("pets").join(&id).join("pet.json");
            let json = std::fs::read_to_string(&path).unwrap();
            let manifest = parse_manifest(&json).unwrap_or_else(|e| panic!("{id}: {e}"));
            let raw: serde_json::Value = serde_json::from_str(&json).unwrap();
            let keys = |v: Option<&serde_json::Value>| -> Vec<String> { v.and_then(|v| v.as_object()).map(|o| o.keys().cloned().collect()).unwrap_or_default() };
            let speech = Stage::ALL
                .iter()
                .map(|s| {
                    let mut set = defaults.clone();
                    set.extend(keys(raw.get("speech")));
                    set.extend(keys(raw.get("stage_speech").and_then(|v| v.get(s.name()))));
                    // STAGE_SPEECH의 그 단계 칸: `  baby: {` … `  },`
                    if let Some(i) = stage_src.find(&format!("  {}: {{", s.name())) {
                        let body = &stage_src[i..];
                        let body = &body[..body.find("\n  },").unwrap_or(body.len())];
                        for line in body.lines().skip(1) {
                            if let Some((k, _)) = line.trim().split_once(':') {
                                set.insert(k.trim().to_string());
                            }
                        }
                    }
                    (s.name(), set)
                })
                .collect();
            Species { id, manifest, speech }
        })
        .collect()
}

/// 웹뷰와 코어가 주고받는 이름 (serde)
fn wire_name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).unwrap().as_str().unwrap().to_string()
}

/// 상태 이름 (Debug의 앞부분)
fn state_name(s: &State) -> String {
    let d = format!("{s:?}");
    d.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_string()
}

/// 동물 한 마리의 자리 (앱 Slot과 같은 몫: 두뇌, 제 세계, 종·단계)와 탐지기 기록
struct Slot {
    pet: Pet,
    world: World,
    species: usize,
    stage: Stage,
    care: Care,
    /// 성장해서 새 단계를 적용해야 함 (앱 track.pending_stage)
    pending_stage: bool,
    /// 최근 상태 바뀜 (틱, 상태, 그림, x, y)
    hist: VecDeque<String>,
    /// 지금 상태 이름과 시작 틱
    state: (String, u64),
    /// 화면에 안 보이기 시작한 틱
    hidden_since: Option<u64>,
    /// 작업표시줄 뒤 (발이 작업 영역 아래)에 내려가기 시작한 틱
    below_since: Option<u64>,
    /// 움직이는 상태에서 (시작 틱, x, y) — 거기서 멀리 못 가면 제자리 달리기
    move_ref: Option<(u64, f64, f64)>,
    /// tuck·poof가 켜지기 시작한 틱
    faded_since: Option<u64>,
}

impl AsRef<Pet> for Slot {
    fn as_ref(&self) -> &Pet {
        &self.pet
    }
}

impl AsMut<Pet> for Slot {
    fn as_mut(&mut self) -> &mut Pet {
        &mut self.pet
    }
}

impl Slot {
    /// 화면 위에 있어야 하는 동물 (앱 Slot::on_screen)
    fn on_screen(&self) -> bool {
        !self.pet.is_housed() && !self.pet.is_away()
    }

    /// 커서가 스프라이트 (사방 20% 여유) 위 (앱 Slot::cursor_inside)
    fn cursor_inside(&self, cx: f64, cy: f64) -> bool {
        let (sx, sy) = self.pet.window_pos();
        let side = self.pet.cfg.size;
        let pad = side * 0.2;
        cx >= sx - pad && cx < sx + side + pad && cy >= sy - pad && cy < sy + side + pad
    }
}

/// 이 종·단계의 설정 (앱 pet_config — 화면 배율 1)
fn config_of(sp: &Species, stage: Stage) -> PetConfig {
    PetConfig::from_manifest(&sp.manifest, 1.0, sp.manifest.size_mult(stage), stage.name())
}

/// 놓인 물건 하나 (앱 objects.rs LiveObject의 코어 몫)
struct Live {
    id: u64,
    object: Object,
}

/// 한 판의 결과: 처음 걸린 이상한 것
#[derive(Debug)]
struct Weird {
    seed: u64,
    tick: u64,
    pet: String,
    what: String,
    history: Vec<String>,
}

impl std::fmt::Display for Weird {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "엥 왜 이래? seed {} tick {} ({:.1}s) pet {}: {}", self.seed, self.tick, self.tick as f64 * DT, self.pet, self.what)?;
        for h in &self.history {
            writeln!(f, "    {h}")?;
        }
        Ok(())
    }
}

/// 상태별 최대 지속 (초) — 넘으면 이상함. None이면 얼마든 (잠·쉼·집 안 등)
fn state_limit(name: &str) -> Option<f64> {
    match name {
        "Sleep" | "Idle" | "Housed" | "Dragged" | "Pressed" | "OnHead" | "Faint" | "Turn" => None,
        "Air" | "Slide" => Some(10.0),
        "Away" => Some(5.0),
        // 🎒 가방 속 해리는 결투 한 판 동안 숨어 있음 (brain_tuck.rs — 그 뒤 반드시 나옴)
        "Tucked" => Some(crate::wizard::TUCK_MAX_SECS + 1.0),
        _ => Some(60.0),
    }
}

/// 걸어서 자리를 옮겨야 하는 상태 (오래 제자리면 제자리 달리기)
fn is_moving(s: &State) -> bool {
    matches!(s, State::Move { .. } | State::Chase { .. } | State::Flee { .. } | State::GoHome | State::ToFood { .. } | State::Exit { .. } | State::Enter { .. })
}

/// 이 동물이 낼 수 있는 마법 효과인가 (예: 서리·영혼 줄기는 디멘터만)
fn fx_allowed(fx: &MagicFx, c: &PetConfig) -> bool {
    match fx {
        MagicFx::Frost { .. } | MagicFx::FrostWipe { .. } | MagicFx::Soul { .. } => c.dementor,
        MagicFx::Stag { .. } | MagicFx::Otter { .. } => c.patronus.is_some(),
        MagicFx::Snake { .. } => c.dark_lord,
        MagicFx::Egg { .. } | MagicFx::Hatchling { .. } | MagicFx::Campfire { .. } => c.gamekeeper,
        MagicFx::Bolt { .. } | MagicFx::Duel { .. } | MagicFx::Wand { .. } | MagicFx::Glow { .. } | MagicFx::Puff { .. } => !c.spells.is_empty() || c.dark_lord,
    }
}

/// 사각형 a와 b가 겹치는 넓이
fn overlap(a: &Rect, b: &Rect) -> f64 {
    let w = (a.right.min(b.right) - a.left.max(b.left)).max(0.0);
    let h = (a.bottom.min(b.bottom) - a.top.max(b.top)).max(0.0);
    w * h
}

/// 탐지기 한 판
struct Run {
    seed: u64,
    rng: fastrand::Rng,
    species: Vec<Species>,
    slots: Vec<Slot>,
    social: Social,
    work_areas: Vec<Rect>,
    windows: Vec<WindowInfo>,
    /// 끄는 중인 창 (id, vx, vy, 남은 초)
    drags: Vec<(u64, f64, f64, f64)>,
    next_window: u64,
    objects: Vec<Live>,
    next_object: u64,
    webs: Vec<(WebSpot, u64)>,
    next_web: u64,
    rhythm: Rhythm,
    marks: DailyMarks,
    cursor: (f64, f64),
    cursor_v: (f64, f64),
    left_down: bool,
    /// 잡고 있는 동물 (자리, 놓을 틱)
    held: Option<(usize, u64)>,
    /// 누른 채 끝날 틱 (빈 곳 클릭)
    click_until: u64,
    /// 직접 주기 (음식, 끝날 틱)
    hand: Option<(Food, u64)>,
    /// 조작 모드 (자리, 끝날 틱, 지금 키, 키를 바꿀 틱)
    control: Option<(usize, u64, ControlInput, u64)>,
    /// 하우스에 간 동물 (자리, 나올 틱)
    housed: Vec<(usize, u64)>,
    focus_until: Option<u64>,
    /// 사용자가 자리를 비운 동안 (끝날 틱)
    away_until: Option<u64>,
    idle_secs: f64,
    start_minute: u32,
    tick: u64,
    door_x: f64,
}

impl Run {
    /// seed로 세상을 만들고 번들 동물 모두 (단계는 무작위)를 떨어뜨림
    fn new(seed: u64) -> Run {
        let mut rng = fastrand::Rng::with_seed(seed);
        let species = all_species();
        let a = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        let work_areas = match rng.u8(0..3) {
            0 => vec![a],
            // 오른쪽에 낮고 아래로 내려온 모니터 (위아래로 틈)
            1 => vec![a, Rect::new(1920.0, 180.0, 3200.0, 980.0)],
            // 왼쪽에 작은 모니터
            _ => vec![a, Rect::new(-1280.0, 56.0, 0.0, 1000.0)],
        };
        let mut run = Run {
            seed,
            rng,
            species,
            slots: Vec::new(),
            social: Social::new(seed),
            work_areas,
            windows: Vec::new(),
            drags: Vec::new(),
            next_window: 100,
            objects: Vec::new(),
            next_object: 10_000,
            webs: Vec::new(),
            next_web: 1,
            rhythm: Rhythm::default(),
            marks: DailyMarks { morning: Some(1), lunch: Some(1) },
            cursor: (900.0, 500.0),
            cursor_v: (0.0, 0.0),
            left_down: false,
            held: None,
            click_until: 0,
            hand: None,
            control: None,
            housed: Vec::new(),
            focus_until: None,
            away_until: None,
            idle_secs: 0.0,
            start_minute: 0,
            tick: 0,
            door_x: 1700.0,
        };
        run.start_minute = run.rng.u32(0..24 * 60);
        for _ in 0..run.rng.usize(2..6) {
            run.open_window();
        }
        for i in 0..run.species.len() {
            let stage = Stage::ALL[run.rng.usize(0..Stage::ALL.len())];
            let cfg = config_of(&run.species[i], stage);
            let area = run.work_areas[run.rng.usize(0..run.work_areas.len())];
            let x = area.left + 80.0 + run.rng.f64() * (area.width() - 160.0);
            let mut pet = Pet::new(cfg, x, area.top + 0.4 * area.height(), seed.wrapping_mul(131).wrapping_add(i as u64));
            pet.set_cursor_species(&run.species[i].id);
            let world = World::new(run.windows.clone(), run.work_areas.clone(), pet.cfg.size);
            let care = Care { stage, ..Care::default() };
            run.slots.push(Slot {
                pet,
                world,
                species: i,
                stage,
                care,
                pending_stage: false,
                hist: VecDeque::new(),
                state: (String::new(), 0),
                hidden_since: None,
                below_since: None,
                move_ref: None,
                faded_since: None,
            });
        }
        run
    }

    /// 새 창을 무작위 자리에 엶 (맨 앞)
    fn open_window(&mut self) {
        let area = self.work_areas[self.rng.usize(0..self.work_areas.len())];
        let w = 300.0 + self.rng.f64() * 900.0;
        let h = 200.0 + self.rng.f64() * 600.0;
        let left = area.left - 100.0 + self.rng.f64() * (area.width() - w + 200.0);
        let top = area.top + self.rng.f64() * (area.height() - 100.0);
        let rect = Rect::new(left, top, left + w, top + h);
        self.windows.insert(0, WindowInfo { id: self.next_window, rect });
        self.next_window += 1;
    }

    /// 지금 틱을 초로
    fn secs(&self) -> f64 {
        self.tick as f64 * DT
    }


    /// 무작위 사건 (앱의 Input 처리처럼 틱 맨 앞에서). 평균적으로 몇 초에 하나
    fn events(&mut self) {
        let r = self.rng.f64();
        if r > 0.02 {
            return;
        }
        let n = self.slots.len();
        let i = self.rng.usize(0..n);
        match self.rng.u8(0..20) {
            0 | 1 if !self.windows.is_empty() => {
                let k = self.rng.usize(0..self.windows.len());
                let id = self.windows[k].id;
                let (vx, vy) = ((self.rng.f64() - 0.5) * 1600.0, (self.rng.f64() - 0.5) * 900.0);
                self.drags.push((id, vx, vy, 0.3 + self.rng.f64() * 2.5));
            }
            2 if !self.windows.is_empty() => {
                let k = self.rng.usize(0..self.windows.len());
                self.windows.remove(k);
            }
            3 if self.windows.len() < 7 => self.open_window(),
            4 if self.objects.len() < MAX_OBJECTS => {
                let kinds = [ObjectKind::Can, ObjectKind::Paper, ObjectKind::Stone, ObjectKind::Glasses, ObjectKind::Milk, ObjectKind::Shirt, ObjectKind::Umbrella, ObjectKind::Pink];
                let kind = kinds[self.rng.usize(0..kinds.len())];
                self.spawn_object(kind, None);
            }
            5 if self.objects.len() < MAX_OBJECTS => {
                let kinds = [ObjectKind::GiantStone, ObjectKind::GiantPoop, ObjectKind::GiantPink, ObjectKind::GiantRainbow, ObjectKind::Computer];
                let kind = kinds[self.rng.usize(0..kinds.len())];
                self.spawn_object(kind, None);
            }
            6 => self.feed(i),
            7 if self.hand.is_none() => {
                let food = [Food::Snack, Food::Meal, Food::Water][self.rng.usize(0..3)];
                let wait = 3.0 + self.rng.f64() * 10.0;
                self.hand = Some((food, after(self.tick, wait)));
                // 손에 든 음식으로 동물 쪽에 다가감
                let p = &self.slots[i].pet;
                self.cursor = (p.body.x, p.body.y - p.cfg.size * 0.5);
            }
            8 if self.held.is_none() && self.slots[i].on_screen() => {
                let (sx, sy) = self.slots[i].pet.window_pos();
                let size = self.slots[i].pet.cfg.size;
                self.cursor = (sx + size / 2.0, sy + size / 2.0);
                self.press();
                self.slots[i].pet.pointer_down(self.cursor.0, self.cursor.1);
                let wait = 0.1 + self.rng.f64() * 4.0;
                let until = after(self.tick, wait);
                self.held = Some((i, until));
                self.cursor_v = ((self.rng.f64() - 0.5) * 2400.0, (self.rng.f64() - 0.5) * 1600.0);
            }
            9 => {
                let to = self.rng.usize(0..self.species.len());
                self.change_species(i, to);
            }
            10 if self.control.is_none() && self.slots[i].on_screen() => {
                self.slots[i].pet.set_control(true);
                let until = after(self.tick, 3.0 + self.rng.f64() * 40.0);
                self.control = Some((i, until, ControlInput::default(), self.tick));
            }
            11 if !self.housed.iter().any(|(k, _)| *k == i) => {
                self.slots[i].pet.go_home(Owner::Ground(0), self.door_x);
                let until = after(self.tick, 3.0 + self.rng.f64() * 30.0);
                self.housed.push((i, until));
            }
            12 if self.focus_until.is_none() => {
                let secs = self.secs();
                let mut pets: Vec<&mut Pet> = self.slots.iter_mut().map(|s| &mut s.pet).collect();
                self.rhythm.start_focus_all(FocusLength::Short, secs, &mut pets);
                self.focus_until = Some(after(self.tick, 10.0 + self.rng.f64() * 60.0));
            }
            13 if self.away_until.is_none() => self.away_until = Some(after(self.tick, 20.0 + self.rng.f64() * 200.0)),
            14 | 15 => {
                // 빈 곳 클릭 (똥이 나올 수 있음)
                self.cursor = (self.rng.f64() * 1920.0, self.rng.f64() * 1040.0);
                self.press();
                self.click_until = after(self.tick, 0.1);
            }
            16 => {
                let hungry = self.rng.bool();
                self.slots[i].pet.set_hungry(hungry);
            }
            17 => {
                // 화면 끝으로 커서를 휙
                self.cursor_v = ((self.rng.f64() - 0.5) * 6000.0, (self.rng.f64() - 0.5) * 3000.0);
            }
            _ => {}
        }
    }

    /// 왼쪽 버튼을 새로 누름: 동물마다 낮은 확률로 똥 (앱 game_loop — 집중 중엔 안 함)
    fn press(&mut self) {
        self.left_down = true;
        let focusing = self.slots.iter().any(|s| s.pet.is_focusing());
        if !crate::poop::may_poop(false, false, focusing) {
            return;
        }
        for k in 0..self.slots.len() {
            if self.objects.len() >= MAX_OBJECTS {
                break;
            }
            if let Some((x, y)) = self.slots[k].pet.try_poop() {
                self.spawn_object(ObjectKind::Poop, Some((x, y)));
            }
        }
    }

    /// 물건을 놓음 (at이 없으면 화면 위 아무 데서나 떨어짐)
    fn spawn_object(&mut self, kind: ObjectKind, at: Option<(f64, f64)>) {
        let (x, y) = at.unwrap_or_else(|| {
            let area = self.work_areas[self.rng.usize(0..self.work_areas.len())];
            (area.left + 60.0 + self.rng.f64() * (area.width() - 120.0), area.top + 40.0)
        });
        let mut object = Object::new(kind, x, y, 1.0, self.seed ^ self.next_object);
        if kind.is_giant() {
            object.set_scale(object.scale() * crate::object::giant_scale(1040.0));
        }
        self.objects.push(Live { id: self.next_object, object });
        self.next_object += 1;
    }

    /// 그릇으로 먹이를 줌 (앱 Input::Feed — 하우스 안이면 문 앞 그릇)
    fn feed(&mut self, i: usize) {
        let food = [Food::Snack, Food::Meal, Food::Favorite, Food::Water][self.rng.usize(0..4)];
        let door_x = self.door_x;
        let s = &mut self.slots[i];
        if s.pet.is_housed() {
            s.pet.set_house_door(Owner::Ground(0), door_x);
        }
        let before = s.care.clone();
        let result = s.care.feed(food);
        if s.pet.feed_result(result, food) {
            if let crate::care::FeedResult::Ate { leveled_up } = result {
                s.pending_stage |= leveled_up;
            }
        } else {
            s.care = before;
        }
        s.pet.set_hungry(s.care.is_hungry());
    }

    /// 종을 바꿈 (앱 Slot::set_stage — 두뇌는 그대로, 설정만 새것)
    fn change_species(&mut self, i: usize, to: usize) {
        let s = &mut self.slots[i];
        s.species = to;
        s.pet.cfg = config_of(&self.species[to], s.stage);
        s.pet.set_cursor_species(&self.species[to].id);
        s.world = World::new(self.windows.clone(), self.work_areas.clone(), s.pet.cfg.size);
        let line = format!("{:>6} === species -> {} ({})", self.tick, self.species[to].id, s.stage.name());
        s.hist.push_back(line);
    }

    /// 창 끌기를 한 틱 움직이고, 세계를 다시 읽을 때면 다시 읽음
    fn move_windows(&mut self) {
        for d in self.drags.iter_mut() {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == d.0) {
                w.rect = Rect::new(w.rect.left + d.1 * DT, w.rect.top + d.2 * DT, w.rect.right + d.1 * DT, w.rect.bottom + d.2 * DT);
            }
            d.3 -= DT;
        }
        self.drags.retain(|d| d.3 > 0.0);
        if self.tick % REFRESH_TICKS == 0 {
            for s in self.slots.iter_mut() {
                s.world = World::new(self.windows.clone(), self.work_areas.clone(), s.pet.cfg.size);
            }
        }
    }

    /// 커서를 움직임 (천천히 떠돌다 가끔 휙)
    fn move_cursor(&mut self) {
        self.cursor_v.0 = self.cursor_v.0 * 0.97 + (self.rng.f64() - 0.5) * 120.0;
        self.cursor_v.1 = self.cursor_v.1 * 0.97 + (self.rng.f64() - 0.5) * 120.0;
        let b = World::new(Vec::new(), self.work_areas.clone(), 1.0).bounds;
        self.cursor.0 = (self.cursor.0 + self.cursor_v.0 * DT).clamp(b.left, b.right - 1.0);
        self.cursor.1 = (self.cursor.1 + self.cursor_v.1 * DT).clamp(b.top, b.bottom + 40.0);
    }

    /// 한 틱 — 게임 루프와 같은 순서
    fn step(&mut self) -> Result<(), Weird> {
        self.tick += 1;
        self.move_windows();
        self.events();
        self.move_cursor();
        let (cx, cy) = self.cursor;
        let secs = self.secs();
        // 디멘터 곁의 배고픔 (앱이 꺼내 감)
        for s in self.slots.iter_mut() {
            let chill = s.pet.take_chill();
            s.care.chill(chill);
        }
        // 직접 주기
        if let Some((food, until)) = self.hand {
            let at = (cx + 20.0, cy + 20.0);
            let mut done = self.tick >= until;
            let ready = (0..self.slots.len()).find(|&i| self.slots[i].on_screen() && self.slots[i].pet.offer_hand(at.0, at.1));
            if let Some(i) = ready {
                let s = &mut self.slots[i];
                let before = s.care.clone();
                let result = s.care.feed(food);
                if s.pet.eat_from_hand(result, food) {
                    if let crate::care::FeedResult::Ate { leveled_up } = result {
                        s.pending_stage |= leveled_up;
                        done = true;
                    }
                } else {
                    s.care = before;
                }
                s.pet.set_hungry(s.care.is_hungry());
            }
            if done {
                for s in self.slots.iter_mut() {
                    s.pet.end_hand();
                }
                self.hand = None;
            }
        }
        // 조작 모드
        if let Some((ci, until, mut keys, next)) = self.control {
            if self.tick >= until {
                self.slots[ci].pet.set_control(false);
            } else {
                if self.tick >= next {
                    let b = self.rng.u8(..);
                    keys = ControlInput { left: b & 1 != 0, right: b & 2 != 0 && b & 1 == 0, up: b & 4 != 0 && b & 32 == 0, down: b & 8 != 0, jump: b & 16 != 0 && b & 64 == 0, run: b & 128 != 0 };
                    let wait = 0.1 + self.rng.f64() * 1.5;
                    let n = after(self.tick, wait);
                    self.control = Some((ci, until, keys, n));
                }
                self.slots[ci].pet.control_keys(keys, DT);
            }
            if !self.slots[ci].pet.is_controlled() {
                self.slots[ci].pet.take_control_ended();
                self.control = None;
            }
        }
        // 하우스에서 나옴
        let door_x = self.door_x;
        let tick = self.tick;
        self.housed.retain(|&(i, until)| {
            if tick < until {
                return true;
            }
            let s = &mut self.slots[i];
            s.pet.leave_house(&s.world, door_x, Owner::Ground(0));
            false
        });
        if self.focus_until.is_some_and(|t| self.tick >= t) {
            self.focus_until = None;
            let mut pets: Vec<&mut Pet> = self.slots.iter_mut().map(|s| &mut s.pet).collect();
            self.rhythm.stop_focus_all(&mut pets);
        }
        if self.tick >= self.click_until && self.held.is_none() {
            self.left_down = false;
        }
        // 커서·잡기
        let held = self.slots.iter().position(|s| s.pet.is_held());
        let hovered = if held.is_some() { None } else { self.slots.iter().rposition(|s| s.on_screen() && s.cursor_inside(cx, cy)) };
        let release = self.held.is_some_and(|(_, t)| self.tick >= t);
        for (i, s) in self.slots.iter_mut().enumerate() {
            s.pet.cursor_input(self.left_down, held.is_some() || self.hand.is_some(), true);
            if Some(i) == held {
                if release {
                    s.pet.pointer_up(cx, cy);
                } else {
                    s.pet.pointer_move(cx, cy);
                }
            } else {
                s.pet.hover(cx, cy, Some(i) == hovered);
            }
        }
        if release || (self.held.is_some() && held.is_none()) {
            self.held = None;
            self.left_down = false;
        }
        // 동물끼리
        self.social.tick(DT, &mut self.slots);
        for l in self.objects.iter_mut() {
            self.social.object_hits(l.id, &mut l.object, DT, &mut self.slots);
        }
        for s in self.slots.iter_mut() {
            s.pet.set_exit_edges(false, false);
            s.pet.set_vertical_edges(false, false);
        }
        let poop_spots: Vec<PoopSpot> = self.objects.iter().filter_map(|l| l.object.poop_spot(l.id)).collect();
        let object_spots: Vec<ObjectSpot> = self.objects.iter().filter_map(|l| l.object.spot(l.id)).collect();
        let portkey_spots: Vec<PortkeySpot> = self.objects.iter().filter_map(|l| Some(PortkeySpot { spot: l.object.spot(l.id)?, key: PortkeyKey::of(l.object.kind) })).collect();
        hoist_moves::notice_pets(&mut self.slots);
        let web_spots: Vec<WebSpot> = self.webs.iter().map(|w| w.0).collect();
        for s in self.slots.iter_mut() {
            s.pet.set_webs(&web_spots);
        }
        for l in self.objects.iter_mut() {
            l.object.set_webs(&web_spots);
        }
        let pet_targets: Vec<Option<Target>> = self.slots.iter().map(|s| (!s.pet.is_away() && s.pet.hit_feels()).then(|| Target { x: s.pet.body.x, y: s.pet.body.y, size: s.pet.cfg.size })).collect();
        let play_busy = held.is_some() || self.hand.is_some();
        for i in 0..self.slots.len() {
            let s = &mut self.slots[i];
            if !s.pet.is_away() {
                let targets: Vec<Target> = pet_targets.iter().enumerate().filter(|(j, _)| *j != i).filter_map(|(_, t)| *t).collect();
                s.pet.notice_pile(None, &targets, !play_busy);
                if s.pet.cfg.juggler {
                    s.pet.notice_objects(&object_spots, &targets, &s.world);
                    s.pet.notice_incoming(&[], &s.world);
                }
                s.pet.set_cursor_rally_ok(!play_busy && self.away_until.is_none());
                s.pet.notice_cursor_rally(&object_spots, &s.world);
                s.pet.notice_poops(&poop_spots, &s.world);
                s.pet.notice_vehicles(&[], &s.world);
                s.pet.notice_play_objects(&object_spots, &s.world);
                if s.pet.cfg.taskbar_play {
                    s.pet.notice_taskbar(&object_spots, Toys { ready: false, live: None }, &s.world);
                }
                if s.pet.cfg.portkey {
                    s.pet.notice_portkeys(&portkey_spots);
                    s.pet.set_portkey_friend(false);
                }
                s.pet.set_icon_lift_ok(false);
                s.pet.update(DT, &s.world);
            }
            let _ = s.pet.take_lumos();
            let _ = s.pet.take_nox();
            if let Some(PortkeyEvent::AskFriend { .. }) = s.pet.take_portkey() {
                // 친구가 없음: 이 화면의 짝 물건으로, 없으면 제자리 (앱 game_loop portkey_asks)
                let Slot { pet, world, .. } = &mut *s;
                pet.portkey_return(world);
            }
            if let Some(speech) = s.pet.take_speech() {
                let name = wire_name(&speech);
                let sp = &self.species[s.species];
                if !sp.speech[s.stage.name()].contains(&name) {
                    let msg = format!("speech {name:?} has no line for {} ({}) — empty bubble", sp.id, s.stage.name());
                    return Err(self.weird(i, msg));
                }
            }
            let s = &mut self.slots[i];
            let _ = s.pet.take_speech_text();
            let _ = s.pet.take_prop();
            if let Some(side) = s.pet.take_exit() {
                return Err(self.weird(i, format!("left the screen ({side:?}) with no friend on that edge")));
            }
        }
        let _ = hoist_moves::sync_lifts(&mut self.slots);
        for s in self.slots.iter_mut() {
            if let Some(id) = s.pet.take_torn_web() {
                self.webs.retain(|w| w.0.id != id);
            }
        }
        self.social.carry(&mut self.slots);
        self.sync_objects();
        // 거미집·낙서·신호·눈 (앱 trail.rs·weather.rs가 꺼내 감)
        for i in 0..self.slots.len() {
            let s = &mut self.slots[i];
            if let Some(w) = s.pet.take_web() {
                if s.on_screen() && w.size.is_finite() && w.size > 0.0 {
                    let id = self.next_web;
                    self.next_web += 1;
                    self.webs.push((WebSpot { id, x: w.x, y: w.y, corner: w.corner, size: w.size, hidden: false }, self.tick));
                    if self.webs.len() > crate::web::MAX_WEBS {
                        self.webs.remove(0);
                    }
                }
            }
            let _ = s.pet.take_doodle();
            let _ = s.pet.take_bat_signal();
            let _ = s.pet.take_snowman();
            let _ = s.pet.take_snow_play();
            let _ = s.pet.take_stomp();
            let _ = s.pet.take_head_bump();
            let _ = s.pet.take_food_request();
        }
        let life = (crate::web::WEB_LIFE_SECS / DT) as u64;
        let now = self.tick;
        self.webs.retain(|w| now - w.1 < life);
        // 하루 리듬 (1초에 한 번)
        if self.tick % 60 == 0 {
            if self.away_until.is_some_and(|t| self.tick >= t) {
                self.away_until = None;
                self.idle_secs = 0.0;
            }
            self.idle_secs = if self.away_until.is_some() { self.idle_secs + 1.0 } else { 0.0 };
            let minute = self.start_minute + (secs / 60.0) as u32;
            let input = RhythmInput { secs, moment: LocalMoment::new(1 + (minute / (24 * 60)) as i32, (minute / 60) % 24, minute % 60), idle: Some(self.idle_secs), visible: true };
            let mut list: Vec<RhythmPet<'_>> = self.slots.iter_mut().map(|s| RhythmPet { pet: &mut s.pet, world: &s.world, care: &mut s.care }).collect();
            self.rhythm.tick_all(&input, &mut list, &mut self.marks);
        }
        // 성장
        for s in self.slots.iter_mut() {
            s.care.tick(DT);
            if s.pet.stage_change_due(s.pending_stage) {
                s.pending_stage = false;
                s.stage = s.care.stage;
                s.pet.cfg = config_of(&self.species[s.species], s.stage);
            }
        }
        self.check()
    }

    /// 앱 objects.rs의 코어 몫: 든 물건 옮기기·던지기, 물건 놀이, 커서 랠리, 물건 물리, 마른 똥 치우기
    fn sync_objects(&mut self) {
        for p in self.slots.iter_mut() {
            let pet = &mut p.pet;
            match pet.take_juggle() {
                Some(JuggleEvent::Release { id, vx, vy }) => {
                    if let Some(l) = self.objects.iter_mut().find(|l| l.id == id) {
                        l.object.launch(vx, vy);
                        self.social.spare(id, pet.social_key(), 0.6);
                        let _ = pet.take_rally();
                    }
                }
                Some(JuggleEvent::Stow { id }) => self.objects.retain(|l| l.id != id),
                Some(JuggleEvent::Pull) | Some(JuggleEvent::Catch { .. }) | None => {}
            }
            let Some(id) = pet.carried_object() else { continue };
            match self.objects.iter_mut().find(|l| l.id == id) {
                Some(l) if !l.object.is_held() => {
                    if let Some((x, y)) = pet.carry_point() {
                        l.object.carry_at(x, y);
                    }
                }
                _ => pet.lose_carry(),
            }
        }
        for l in self.objects.iter_mut().filter(|l| l.object.is_carried()) {
            if !self.slots.iter().any(|p| p.pet.carried_object() == Some(l.id)) {
                l.object.let_fall();
            }
        }
        let mut spawns = Vec::new();
        for p in self.slots.iter_mut() {
            match p.pet.take_object_play() {
                Some(ObjectPlay::Nudge { id, dir, reach }) => {
                    if let Some(l) = self.objects.iter_mut().find(|l| l.id == id) {
                        l.object.nudge(dir, reach);
                    }
                }
                Some(ObjectPlay::Gulp { id }) => self.objects.retain(|l| !(l.id == id && !l.object.kind.is_toy() && !l.object.kind.is_giant())),
                Some(ObjectPlay::Yank { id, vx, vy }) => {
                    if let Some(l) = self.objects.iter_mut().find(|l| l.id == id && l.object.spot(l.id).is_some()) {
                        l.object.launch(vx, vy);
                    }
                }
                Some(ObjectPlay::Levitate { id, rise, secs }) => {
                    if let Some(l) = self.objects.iter_mut().find(|l| l.id == id) {
                        l.object.levitate(rise, secs);
                    }
                }
                Some(ObjectPlay::Vanish { id }) => self.objects.retain(|l| !(l.id == id && l.object.spot(l.id).is_some())),
                Some(ObjectPlay::Spawn { kind, x, y }) => spawns.push((kind, x, y)),
                Some(ObjectPlay::Quake { x, y, owner, reach }) => {
                    for l in self.objects.iter_mut() {
                        let near = l.object.spot(l.id).is_some_and(|s| s.owner == owner && (s.y - y).abs() <= crate::juggle::SAME_LEVEL_PX && (s.x - x).abs() <= reach);
                        if near {
                            l.object.pop(crate::play::QUAKE_POP_VY);
                        }
                    }
                }
                None => {}
            }
            match p.pet.take_cursor_rally() {
                Some(crate::cursor_rally::CursorRallyEvent::Back { id, rally }) => {
                    if let Some(l) = self.objects.iter_mut().find(|l| l.id == id && l.object.in_flight()) {
                        l.object.set_rally(rally);
                        l.object.mark_cursor_rally();
                    }
                }
                Some(crate::cursor_rally::CursorRallyEvent::Boop { id, vx, vy, rally }) => {
                    if let Some(l) = self.objects.iter_mut().find(|l| l.id == id && l.object.spot(l.id).is_some()) {
                        l.object.boop_back(vx, vy, rally);
                    }
                }
                None => {}
            }
        }
        for (kind, x, y) in spawns {
            if self.objects.len() < MAX_OBJECTS {
                self.spawn_object(kind, Some((x, y)));
            }
        }
        let world = World::new(self.windows.clone(), self.work_areas.clone(), 128.0);
        self.objects.retain_mut(|l| l.object.tick(DT, &world, ExitEdges::default(), false).is_none() && !l.object.expired());
    }

    /// 이상한 것 하나를 기록과 함께
    fn weird(&self, i: usize, what: String) -> Weird {
        let s = &self.slots[i];
        Weird { seed: self.seed, tick: self.tick, pet: format!("#{i} {}", self.species[s.species].id), what, history: s.hist.iter().cloned().collect() }
    }

    /// 틱마다 살핌
    fn check(&mut self) -> Result<(), Weird> {
        let tick = self.tick;
        let mut fx = Vec::new();
        for i in 0..self.slots.len() {
            let s = &mut self.slots[i];
            let r = s.pet.render();
            let anim = wire_name(&r.anim);
            let name = state_name(&s.pet.state);
            if name != s.state.0 || s.hist.is_empty() {
                s.state = (name.clone(), tick);
                let line = format!("{tick:>6} {:<12} anim {:<12} x {:>7.1} y {:>7.1} std {:?} ctl {}", name, anim, s.pet.body.x, s.pet.body.y, s.pet.standing.map(|st| st.owner), r.ctl);
                s.hist.push_back(line);
                if s.hist.len() > HISTORY {
                    s.hist.pop_front();
                }
            }
            let sp = &self.species[s.species];
            let (wx, wy) = s.pet.window_pos();
            if ![s.pet.body.x, s.pet.body.y, s.pet.body.vx, s.pet.body.vy, wx, wy].iter().all(|v| v.is_finite()) {
                return Err(self.weird(i, format!("body/window not finite: body {:?} window ({wx}, {wy})", self.slots[i].pet.body)));
            }
            // main.js가 따로 대신 그리는 그림 (큰 점프 그림이 없으면 hop 줄 — hopLeap)
            let drawn = match anim.as_str() {
                "bigjump" if !sp.manifest.has_anim("bigjump") => "hop",
                a => a,
            };
            if !sp.manifest.has_anim(drawn) {
                return Err(self.weird(i, format!("anim {anim:?} is not in {}/pet.json — the webview silently draws idle", sp.id)));
            }
            let held_by_user = s.pet.is_held();
            let elapsed = (tick - s.state.1) as f64 * DT;
            if let Some(limit) = state_limit(&name) {
                // 조작 모드 중엔 사용자가 오래 끌 수 있음
                if elapsed > limit && !r.ctl {
                    return Err(self.weird(i, format!("state {name} lasted {elapsed:.1}s (> {limit}s)")));
                }
            }
            // 뿅·가방 흐림은 잠깐만
            let faded = r.poof || r.tuck > 0.0;
            if r.poof && !matches!(s.pet.state, State::Farewell { .. } | State::Away) {
                return Err(self.weird(i, format!("poof shown in state {name} (only while leaving)")));
            }
            match (faded, s.faded_since) {
                (true, None) => s.faded_since = Some(tick),
                (false, _) => s.faded_since = None,
                (true, Some(t)) if (tick - t) as f64 * DT > 8.0f64.max(crate::wizard::TUCK_MAX_SECS + 1.0) => {
                    return Err(self.weird(i, format!("faded (poof {} tuck {:.2}) for {:.1}s — pet looks gone", r.poof, r.tuck, (tick - t) as f64 * DT)));
                }
                _ => {}
            }
            // 화면에 보임: 스프라이트가 어느 작업 영역과 1/4 넘게 겹침
            if s.on_screen() && !held_by_user {
                let size = s.pet.cfg.size;
                let sprite = Rect::new(wx, wy, wx + size, wy + size);
                let shown = s.world.work_areas.iter().map(|a| overlap(&sprite, a)).sum::<f64>();
                if shown < 0.25 * size * size {
                    let since = *s.hidden_since.get_or_insert(tick);
                    if (tick - since) as f64 * DT > 3.0 {
                        return Err(self.weird(i, format!("off-screen for {:.1}s: sprite {sprite:?}", (tick - since) as f64 * DT)));
                    }
                } else {
                    s.hidden_since = None;
                }
                // 발이 그 자리 작업 영역 바닥 아래 (작업표시줄 뒤·모니터 틈)
                let below = !s.world.work_areas.iter().any(|a| s.pet.body.x >= a.left - 1.0 && s.pet.body.x <= a.right + 1.0 && s.pet.body.y <= a.bottom + 6.0);
                if below && s.pet.standing.is_some() {
                    let since = *s.below_since.get_or_insert(tick);
                    if (tick - since) as f64 * DT > 2.0 {
                        let msg = format!("standing below the work area (behind the taskbar / in a monitor gap) at ({:.0}, {:.0})", s.pet.body.x, s.pet.body.y);
                        return Err(self.weird(i, msg));
                    }
                } else {
                    s.below_since = None;
                }
            } else {
                s.hidden_since = None;
                s.below_since = None;
            }
            // 제자리 달리기
            // 제자리 달리기 = 움직이는 상태에 움직이는 그림인데 자리는 그대로 (막혀 서서 쳐다보는 그림이면 괜찮음)
            if is_moving(&s.pet.state) && !r.ctl && !matches!(anim.as_str(), "idle" | "look" | "lookup") {
                let (x, y) = (s.pet.body.x, s.pet.body.y);
                match s.move_ref {
                    Some((_, x0, y0)) if (x - x0).hypot(y - y0) > 12.0 => s.move_ref = Some((tick, x, y)),
                    Some((t, _, _)) if (tick - t) as f64 * DT > 8.0 => {
                        return Err(self.weird(i, format!("running in place in {name} for {:.1}s at ({x:.0}, {y:.0})", (tick - t) as f64 * DT)));
                    }
                    Some(_) => {}
                    None => s.move_ref = Some((tick, x, y)),
                }
            } else {
                s.move_ref = None;
            }
            fx.clear();
            s.pet.magic_fx(&mut fx);
            if let Some(f) = fx.iter().find(|f| !fx_allowed(f, &s.pet.cfg)) {
                return Err(self.weird(i, format!("magic effect {f:?} from a pet that cannot make it")));
            }
        }
        Ok(())
    }
}

/// tick에서 몇 초 뒤의 틱
fn after(tick: u64, secs: f64) -> u64 {
    tick + (secs / DT).round() as u64
}

/// seed 한 판을 secs초 돌림. 처음 걸린 이상한 것을 돌려줌
fn run(seed: u64, secs: f64) -> Result<(), Weird> {
    let mut r = Run::new(seed);
    let ticks = (secs / DT) as u64;
    while r.tick < ticks {
        r.step()?;
    }
    Ok(())
}

/// 여러 seed를 돌려 걸린 것을 모두 모아 실패함
fn run_seeds(seeds: impl Iterator<Item = u64>, secs: f64) {
    let found: Vec<String> = seeds.filter_map(|s| run(s, secs).err()).map(|w| w.to_string()).collect();
    assert!(found.is_empty(), "{} weird run(s):\n{}", found.len(), found.join("\n"));
}

#[test]
fn qa_sim_short_runs_show_nothing_weird() {
    run_seeds(1..=4, 120.0);
}

#[test]
#[ignore = "long: about ten minutes of simulated play per seed"]
fn qa_sim_long_runs_show_nothing_weird() {
    // QA_SEEDS=시작:끝 으로 다른 seed 묶음도 돌려 볼 수 있음
    let (a, b) = std::env::var("QA_SEEDS").ok().and_then(|v| v.split_once(':').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((100, 116));
    run_seeds(a..b, 900.0);
}

/// 걸린 판 들여다보기: `QA_TRACE=seed:자리:시작틱:끝틱 cargo test -- --ignored qa_sim_trace --nocapture` — 그 동물의 틱마다 상태·자리·그림을 찍음
#[test]
#[ignore = "debug aid: set QA_TRACE"]
fn qa_sim_trace() {
    let Ok(spec) = std::env::var("QA_TRACE") else { return };
    let v: Vec<u64> = spec.split(':').map(|s| s.parse().unwrap()).collect();
    let (seed, slot, from, to) = (v[0], v[1] as usize, v[2], v[3]);
    let mut r = Run::new(seed);
    while r.tick < to {
        let res = r.step();
        if r.tick >= from {
            let p = &r.slots[slot].pet;
            println!("{:>6} {:?} std {:?} body ({:.1}, {:.1}) v ({:.0}, {:.0}) anim {:?} win {:?}", r.tick, p.state, p.standing.map(|s| s.owner), p.body.x, p.body.y, p.body.vx, p.body.vy, p.render().anim, p.window_pos());
        }
        if let Err(w) = res {
            println!("{w}");
            break;
        }
    }
}

/// 🏰 호그와트 장면: 번들 동물 (ids — 청소년, 아무것도 끄지 않음)을 한 바닥 xs 자리에 풀어 놓고 앱 순서 (동물끼리 → 서로 보기 → 물건 보기 →
/// update → sync_lifts)로 secs초 돌림. 바닥의 물건 (spots)은 가방에 넣으면 치움. 틱마다 each(지금 초, 동물들)
fn hogwarts_scene(ids: &[&str], xs: &[f64], mut spots: Vec<ObjectSpot>, seed: u64, secs: f64, mut each: impl FnMut(f64, &[Pet])) {
    let bundled = crate::test_util::bundled();
    let world = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    let mut pets: Vec<Pet> = ids
        .iter()
        .zip(xs)
        .enumerate()
        .map(|(i, (id, &x))| {
            let cfg = bundled.iter().find(|(b, _)| b == id).unwrap_or_else(|| panic!("{id}")).1.clone();
            Pet::new(cfg, x, 1040.0 - 200.0, seed * 31 + i as u64)
        })
        .collect();
    let mut social = Social::new(seed);
    for k in 0..(secs / DT) as u64 {
        social.tick(DT, &mut pets);
        hoist_moves::notice_pets(&mut pets);
        for p in pets.iter_mut() {
            p.notice_play_objects(&spots, &world);
            p.update(DT, &world);
            if let Some(ObjectPlay::Vanish { id }) = p.take_object_play() {
                spots.retain(|s| s.id != id);
            }
            let _ = p.take_speech();
        }
        let _ = hoist_moves::sync_lifts(&mut pets);
        social.carry(&mut pets);
        each(k as f64 * DT, &pets);
    }
}

/// 🧔🤗 해그리드가 해리·헤르미온느를 만나면 자주 번쩍 안아 줌: 한 화면에서 5분 안에 (seed 여섯 모두 한 번은, 평균 1.5번 넘게)
#[test]
fn qa_sim_hagrid_hugs_harry_and_hermione_when_they_meet() {
    let mut total = 0;
    for seed in 1..=6 {
        let mut hugs = 0;
        let mut was = false;
        hogwarts_scene(&["hagrid", "harry", "hermione"], &[700.0, 1000.0, 1300.0], vec![], seed, 300.0, |_, pets| {
            let friends = [pets[1].social_key(), pets[2].social_key()];
            let now = matches!(pets[0].state, State::Hagrid { act: crate::brain::hagrid_moves::HagridAct::Pick { key }, .. } if friends.contains(&key));
            hugs += usize::from(now && !was);
            was = now;
        });
        assert!(hugs >= 1, "seed {seed}: 5분에 한 번도 안 안음");
        total += hugs;
    }
    assert!(total >= 9, "5분씩 여섯 판에 {total}번 안음");
}

/// 🎒 해리·헤르미온느·볼드모트가 한 화면에 있으면 해리가 낄 결투는 대개 헤르미온느가 해리를 가방에 숨기고 대신 함:
/// 6분 40초씩 여덟 판 — 거의 모든 판에 한 번은, 모두 합쳐 해리의 평소 결투보다 두 배 넘게
#[test]
fn qa_sim_hermione_usually_tucks_harry_before_the_duel() {
    let (mut tucks, mut plain, mut seeds_with) = (0, 0, 0);
    for seed in 1..=8 {
        let (mut mine, mut was_t, mut was_d) = (0, false, false);
        hogwarts_scene(&["harry", "hermione", "voldemort"], &[600.0, 1100.0, 1500.0], vec![], seed, 400.0, |_, pets| {
            let t = pets[0].tucked_in().is_some();
            let d = pets[0].dueling().is_some();
            mine += usize::from(t && !was_t);
            plain += usize::from(d && !was_d);
            was_t = t;
            was_d = d;
        });
        seeds_with += usize::from(mine > 0);
        tucks += mine;
    }
    assert!(seeds_with >= 6 && tucks >= 2 * plain, "숨기기 {tucks} ({seeds_with}판) · 해리 평소 결투 {plain}");
}

/// 👜 바닥에 물건이 있으면 헤르미온느가 몇 분 안에 가방에 하나씩 넣고, 넣을수록 가방이 눈에 띄게 커짐 (seed 넷 모두 — 물건 하나여도)
#[test]
fn qa_sim_hermione_bags_the_floor_objects_and_the_bag_grows() {
    let thing = |id: u64, x: f64| ObjectSpot { id, x, y: 1040.0, owner: Owner::Ground(0), half_w: 24.0, giant: false, local: false };
    for seed in 1..=4 {
        let mut biggest: f64 = 0.0;
        hogwarts_scene(&["hermione"], &[900.0], vec![thing(1, 400.0), thing(2, 1300.0), thing(3, 1600.0)], seed, 240.0, |_, pets| {
            biggest = biggest.max(pets[0].render().bag);
        });
        assert!(biggest >= crate::wizard::bag_growth(3) - 1e-9, "seed {seed}: 가방이 커지지 않음 ({biggest})");
        let mut seen = false;
        hogwarts_scene(&["hermione"], &[900.0], vec![thing(1, 1300.0)], seed, 240.0, |_, pets| seen |= pets[0].render().bag > 0.0);
        assert!(seen, "seed {seed}: 하나뿐이어도 치움");
    }
}
