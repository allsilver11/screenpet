//! 계절 날씨 (v0.3.7) — 모니터마다 마우스가 통과하는 투명 창 하나 (`weather-<n>`, weather.html)와, 눈덩이·눈사람 작은 창.
//! 게임 루프가 매 틱 tick을 부르지만, 날씨가 없으면 (실제 날씨가 맑은 겨울·메뉴로 껐거나·모두 숨겼으면) 창이 하나도 없고
//! 1초에 한 번 플래그만 봄 (달력과 시계는 1분에 한 번). 켜져 있어도 창 목록을 따로 읽지 않음 — 내 동물 세계의 디딤판을
//! 5 Hz로 맞춰 보고, 장면이 바뀐 모니터에만 보냄. 더미는 1초에 한 번 (screenpet_core::weather).
//! 그 모니터의 앞 창이 전체 화면이거나 사용자가 자리를 비웠으면 창을 숨기고 웹뷰도 그림을 멈춤 (돌아오면 다시 그림).
//! 🌀 태풍 (v0.6.8): 게임 루프가 매 틱 storm을 부름 — 코어 typhoon::Storm이 돌풍과 회오리를 정하고, 여기서 동물(Pet)과
//! 물건(Objects)에 그대로 시킴. 바람·회오리 자리는 장면(wind·tornado)에 실어 웹뷰가 깔때기를 그림 (새 창 없음)

use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};
use screenpet_core::geom::Rect;
use screenpet_core::visit::Side;
use screenpet_core::real_weather::{self, Reading, WeatherSource};
use screenpet_core::typhoon::{Loose, Storm, Thing, Whirl};
use screenpet_core::weather::{self, OverlayRetry, PileField, Scene, Weather};
use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::slots::Slot;
use crate::AppState;

/// 🌀 돌풍에 더미가 흩날리는 알갱이 수 (모니터마다), 빠르기 (논리 px/s)
const GUST_PUFF: u32 = 24;
const GUST_PUFF_SPEED: f64 = 620.0;
/// 🌀 한 틱에 넘기는 시간 상한 (초)
const STORM_MAX_DT: f64 = 0.05;

/// 날씨 덮개 창 라벨 앞부분
const PREFIX: &str = "weather-";
/// 개발용 숨은 미리 보기: snow·light_snow·rain·shower·petals·leaves·green_leaves·typhoon·off (screenpet_core::weather::parse_preview). 메뉴에는 없음
const PREVIEW_ENV: &str = "SCREENPET_WEATHER";
/// 미리 보기에선 더미가 이 배로 빨리 쌓임
const PREVIEW_GROW: f32 = 10.0;
/// 앱이 뜨고 이만큼 뒤부터 (시작을 방해하지 않게)
const START_DELAY: Duration = Duration::from_secs(2);
/// 계절(날씨 종류)을 다시 보는 간격
const SEASON_EVERY: Duration = Duration::from_secs(60);
/// 날씨가 없을 때 메뉴·숨기기 플래그를 보는 간격
const IDLE_EVERY: Duration = Duration::from_secs(1);
/// 창 목록을 웹뷰에 맞추는 간격 (5 Hz)
const SCENE_EVERY: Duration = Duration::from_millis(200);
/// 쌓인 더미를 웹뷰에 보내는 간격
const PILES_EVERY: Duration = Duration::from_secs(1);
/// 새 창은 이 시간 동안 1초마다 장면을 다시 보냄 (첫 장면은 웹뷰가 듣기 전에 지나갈 수 있음)
const FRESH: Duration = Duration::from_secs(5);
/// 소품 창 크기 (논리 px, tools/draw_props.py). 아래 가운데가 자리
const SNOWMAN_SIZE: (f64, f64) = (64.0, 80.0);
/// 눈덩이 창 크기 (논리 px)
const SNOWBALL_SIZE: (f64, f64) = (40.0, 40.0);

/// 흩날리는 알갱이 한 뭉치 (논리 px, 이 창 왼쪽 위가 (0, 0))
#[derive(Serialize)]
struct Flurry {
    kind: Weather,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    count: u32,
}

/// 친구가 쓸어 보낸 알갱이 수, 날아 들어오는 속도 (논리 px/s)
const FLURRY_COUNT: u32 = 40;
/// 날아 들어오는 속도 (논리 px/s)
const FLURRY_SPEED: f64 = 420.0;
/// 동물이 뛰어든 더미를 비우는 높이: 발에서 위아래로 (논리 px)
const BURST_REACH_Y: f64 = 8.0;

/// 덮개 창에 보내는 장면임
#[derive(Serialize, PartialEq)]
struct Payload {
    paused: bool,
    #[serde(flatten)]
    scene: Scene,
}

/// 모니터 하나를 덮는 날씨 창임
struct Overlay {
    window: WebviewWindow,
    label: String,
    wa: Rect,
    scale: f64,
    paused: bool,
    born: Instant,
    resent: Instant,
    last: Option<Payload>,
}

/// 바닥에 세운 눈사람 창임
struct Snowman {
    window: WebviewWindow,
    born: Instant,
    x: f64,
}

/// ☀️ 날씨 효과 상태임 (게임 루프가 가짐): 덮개 창, 더미, 눈사람
pub struct WeatherFx {
    preview: Option<Option<Weather>>,
    season: Option<Weather>,
    /// ☀️ 실제 날씨 / 📅 계절 날씨 / 직접 고른 날씨 (설정)
    source: WeatherSource,
    /// ☀️ 중계 서버가 마지막으로 준 고른 도시의 날씨 (3시간이 지나면 달력으로)
    reading: Option<Reading>,
    active: Option<Weather>,
    next_season: Instant,
    next_scene: Instant,
    next_piles: Instant,
    last_piles: Instant,
    overlays: Vec<Overlay>,
    /// fit_overlays가 마지막으로 맞춘 작업 영역
    fitted: Vec<Rect>,
    /// 날씨 창 만들기가 실패한 뒤 다음 시도까지 기다림
    retry: OverlayRetry,
    field: PileField,
    seed: u64,
    next_label: u64,
    snowman: Option<Snowman>,
    ball: Option<(WebviewWindow, (i32, i32))>,
    /// 🌀 태풍 동안의 돌풍·회오리 (태풍이 아니면 None)
    storm: Option<Storm>,
    storm_at: Instant,
    /// 🌀 틱마다 다시 쓰는 그릇 (빨아들일 것, 회오리가 시킨 것)
    loose: Vec<Loose>,
    whirls: Vec<Whirl>,
}

impl WeatherFx {
    /// 🔄 날씨 덮개 창 웹뷰를 다시 읽음 (새로고침·깨어남 — 눈사람은 prop- 창이라 게임 루프가 같이 읽음). 장면은 resend가 다시 보냄 (더미는 앱이 가짐)
    pub fn reload_windows(&mut self) {
        for o in &self.overlays {
            let _ = o.window.reload();
        }
    }

    /// 다시 읽은 덮개 창(label)에 지금 장면을 다시 보냄 (페이지가 듣기를 마쳤을 때)
    pub fn resend(&mut self, label: &str) {
        for o in self.overlays.iter_mut().filter(|o| o.label == label) {
            o.last = None;
        }
    }

    /// 새로 만듦. `SCREENPET_WEATHER`가 있으면 미리 보기로 둠
    pub fn new(seed: u64) -> Self {
        let preview = std::env::var(PREVIEW_ENV).ok().and_then(|v| weather::parse_preview(&v));
        let now = Instant::now();
        Self {
            preview,
            season: None,
            source: WeatherSource::default(),
            reading: None,
            active: None,
            next_season: now,
            next_scene: now + START_DELAY,
            next_piles: now,
            last_piles: now,
            overlays: Vec::new(),
            fitted: Vec::new(),
            retry: OverlayRetry::default(),
            field: new_field(seed, preview),
            seed,
            next_label: 0,
            snowman: None,
            ball: None,
            storm: None,
            storm_at: now,
            loose: Vec::new(),
            whirls: Vec::new(),
        }
    }

    /// 메뉴 체크를 지금 상태에 맞춤
    pub fn sync_menu(&mut self, app: &AppHandle, menu_off: bool) {
        let _ = app.state::<AppState>().menu.weather.set_checked(!menu_off);
        // 다음 틱에 바로 반영함
        self.next_scene = Instant::now().min(self.next_scene);
    }

    /// ☀️ 실제 날씨 / 📅 계절 날씨 / 직접 고른 날씨를 바꿨음: 바로 다시 봄
    pub fn set_source(&mut self, source: WeatherSource) {
        self.source = source;
        self.next_season = Instant::now();
        self.next_scene = Instant::now().min(self.next_scene);
    }

    /// ☀️ 중계 서버가 새 날씨를 줬음 (None = 도시를 바꿔 예전 값을 버림): 바로 다시 봄
    pub fn set_reading(&mut self, reading: Option<Reading>) {
        self.reading = reading;
        self.next_season = Instant::now();
        self.next_scene = Instant::now().min(self.next_scene);
    }

    /// 게임 루프 스레드에서 매 틱 (창 만들기가 메인 스레드를 기다림)
    pub fn tick(&mut self, app: &AppHandle, now: Instant, menu_off: bool, away: bool, slots: &mut [Slot]) {
        if self.active.is_some() {
            if let Some(own) = slots.first_mut() {
                self.track_own(app, now, own);
            }
        }
        if now < self.next_scene {
            return;
        }
        if now >= self.next_season {
            self.next_season = now + SEASON_EVERY;
            let today = chrono::Local::now();
            // 달력 날씨 (weather::weather_at)는 실제 날씨가 없거나 오래됐을 때만 — core가 고름
            self.season = real_weather::weather_now(self.source, self.reading, today.timestamp(), today.year(), today.month(), today.day(), today.num_seconds_from_midnight());
        }
        let want = weather::wanted(self.season, self.preview, menu_off, crate::windows_hidden(app));
        if want.is_none() {
            if self.active.take().is_some() {
                self.tear_down(slots);
            }
            self.next_scene = now + IDLE_EVERY;
            return;
        }
        self.active = want;
        self.next_scene = now + SCENE_EVERY;
        let Some(kind) = want else { return };
        self.step(app, now, kind, away, slots);
    }

    /// 날씨가 켜진 동안 5 Hz
    fn step(&mut self, app: &AppHandle, now: Instant, kind: Weather, away: bool, slots: &mut [Slot]) {
        let Some(own) = slots.first() else { return };
        self.field.sync(&own.world);
        if now >= self.next_piles {
            self.next_piles = now + PILES_EVERY;
            let dt = now.duration_since(self.last_piles).as_secs_f64().min(5.0);
            self.last_piles = now;
            // 자리를 비운 동안엔 더미도 자라지 않음 (돌아왔을 때 한꺼번에 쌓여 있지 않게)
            if !away {
                self.field.tick(dt, kind);
            }
        }
        self.fit_overlays(app, now, &own.world.work_areas);
        for o in &mut self.overlays {
            let paused = weather::overlay_paused(away, &own.world.windows, &o.wa);
            if paused != o.paused {
                o.paused = paused;
                if paused { crate::overlay::hide_quietly(&o.window) } else { crate::overlay::show_quietly(&o.window) }
                if let Some(s) = self.snowman.as_ref().filter(|s| on_area(&o.wa, s.x)) {
                    if paused { crate::overlay::hide_quietly(&s.window) } else { crate::overlay::show_quietly(&s.window) }
                }
            }
            let mut scene = self.field.scene(kind, &o.wa, o.scale);
            if let Some(storm) = self.storm.as_ref().filter(|_| kind.is_stormy()) {
                // 바람은 0.1 단위로 (돌풍이 부는 동안만 장면이 바뀜), 회오리는 그 모니터와, 모니터 끝에 걸쳐 있으면 옆 모니터에도
                // (옆 모니터에선 화면 밖 x로 반쪽만 보임 — 그 모니터 바닥에 그림)
                scene.wind = Some((storm.wind() * 10.0).round() as f32 / 10.0);
                // 넷째 값 = 깔때기 크기 (🌀 허리케인급은 크게 — 웹뷰가 그만큼 크게 그리고 걸침 거리도 같음)
                scene.tornado = storm.tornado().filter(|t| t.area() == o.wa || screenpet_core::typhoon::straddles(t.x(), &o.wa, o.scale, t.size())).map(|t| {
                    let ground = if t.area() == o.wa { t.ground() } else { o.wa.bottom };
                    [((t.x() - o.wa.left) / o.scale).round() as f32, ((ground - o.wa.top) / o.scale).round() as f32, (t.vx() / o.scale).round() as f32, t.size() as f32]
                });
            }
            let payload = Payload { paused, scene };
            let young = now.duration_since(o.born) < FRESH && now.duration_since(o.resent) >= PILES_EVERY;
            if young || o.last.as_ref() != Some(&payload) {
                let _ = app.emit_to(o.label.as_str(), "weather-scene", &payload);
                o.last = Some(payload);
                o.resent = now;
            }
        }
        let looks_up = kind.looks_up();
        for s in slots.iter_mut() {
            s.pet.set_petals(looks_up);
        }
        let snow_left = self.field.snow_on_ground();
        if self.snowman.as_ref().is_some_and(|s| !weather::snowman_stays(Some(kind), now.duration_since(s.born).as_secs_f64(), snow_left)) {
            if let Some(s) = self.snowman.take() {
                let _ = s.window.destroy();
            }
        }
        let ready = !away && kind.is_snow() && self.snowman.is_none() && self.field.snow_settled();
        if let Some(own) = slots.first_mut() {
            own.pet.set_snow_ready(ready);
        }
    }

    /// 🌀 태풍 (게임 루프가 매 틱, 동물 update·물건 놀이 뒤 objects.tick 전에): 돌풍이 일면 동물·물건·더미가 휘청이고,
    /// 회오리가 가까운 것을 빨아들여 돌리다 내던짐. 태풍이 아니거나 자리 비움·모두 숨김이면 회오리를 보냄 —
    /// 돌던 것은 코어가 0.3초 뒤 그 자리에서 떨어뜨림 (잃어버리지 않음). skip = 지금 건드리면 안 되는 동물 (🛷 썰매)
    #[allow(clippy::too_many_arguments)]
    pub fn storm(&mut self, app: &AppHandle, now: Instant, away: bool, slots: &mut [Slot], objects: &mut crate::objects::Objects, network: &crate::network::Network, config: &screenpet_core::config::Config, skip: impl Fn(&Slot) -> bool) {
        let dt = now.duration_since(self.storm_at).as_secs_f64().min(STORM_MAX_DT);
        self.storm_at = now;
        let stormy = self.active.is_some_and(Weather::is_stormy) && !away && !crate::windows_hidden(app);
        if !stormy {
            self.storm = None;
            return;
        }
        let Some(own) = slots.first() else { return };
        let (bounds, dpi) = (own.world.bounds, own.dpi);
        // 쉬는 모니터 (전체 화면 앱이 덮음)에는 회오리가 오지 않음
        let areas: Vec<Rect> = own.world.work_areas.iter().copied().filter(|wa| !self.overlays.iter().any(|o| o.paused && o.wa == *wa)).collect();
        self.loose.clear();
        for s in slots.iter() {
            if skip(s) || !s.on_screen() || !s.pet.storm_free() {
                continue;
            }
            let cross = s.pet.whirl_cross_side(&s.world);
            self.loose.push(Loose { thing: Thing::Pet(s.pet.social_key()), x: s.pet.body.x, y: s.pet.body.y, cross, heavy: false });
        }
        objects.storm_loose(bounds, network, config, &mut self.loose);
        let seed = self.seed;
        let storm = self.storm.get_or_insert_with(|| Storm::new(seed ^ fastrand::u64(..)));
        self.whirls.clear();
        let gust = storm.tick(dt, &areas, bounds, dpi, &self.loose, &mut self.whirls);
        for w in &self.whirls {
            match w.thing() {
                Thing::Object(_) => objects.storm_apply(w),
                Thing::Pet(key) => {
                    let Some(s) = slots.iter_mut().find(|s| s.pet.social_key() == key) else { continue };
                    match *w {
                        Whirl::Hold { x, y, .. } => {
                            s.pet.whirl_to(x, y);
                        }
                        Whirl::Fling { vx, vy, cross, .. } => s.pet.whirl_release(vx, vy, cross),
                    }
                }
            }
        }
        let Some(gust) = gust else { return };
        for s in slots.iter_mut() {
            if !skip(&*s) && s.on_screen() {
                s.pet.feel_gust(gust);
            }
        }
        objects.gust(gust);
        // 남은 더미가 바람 쪽으로 흩날림 (더미는 코어가 빨리 녹임 — rates)
        let Some(kind) = self.field.pile_kind().filter(|_| self.field.has_piles()) else { return };
        for o in self.overlays.iter().filter(|o| !o.paused) {
            let x = o.wa.left + fastrand::f64() * o.wa.width();
            let speed = GUST_PUFF_SPEED * gust.dir * o.scale;
            self.puff(app, kind, (x, o.wa.bottom - 6.0 * o.scale), (speed, -90.0 * o.scale), GUST_PUFF);
        }
    }

    /// 🧹 쓰레기 다 치우기: 눈·꽃잎·낙엽 더미도 싹 치움. 장면은 다음 5 Hz에 나감
    pub fn clear_piles(&mut self) {
        self.field.clear_all();
    }

    /// 🧹 빗자루가 지나갔음: 쓴 양과 더미 그림 (날씨가 없으면 None). 장면은 다음 5 Hz에 나감
    pub fn sweep(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> Option<(f32, Weather)> {
        self.active?;
        let kind = self.field.pile_kind()?;
        Some((self.field.sweep(x0, x1, y, reach_y), kind))
    }

    /// ⛄ 날씨가 켜져 있고 쌓인 더미가 눈임 (문지르기를 볼 때만 더미를 셈)
    pub fn has_snow(&self) -> bool {
        self.active.is_some() && self.field.pile_kind().is_some_and(Weather::is_snow)
    }

    /// ⛄ [x0, x1]의 높이 y(±reach_y)에 보이는 눈 양 (논리 px). 날씨가 없거나 그 모니터 날씨가 쉬는 중(전체 화면 등)이면 0
    pub fn snow_under(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> f32 {
        let x = (x0 + x1) / 2.0;
        if !self.has_snow() || self.overlays.iter().any(|o| o.paused && on_area(&o.wa, x)) {
            return 0.0;
        }
        self.field.snow_amount(x0, x1, y, reach_y)
    }

    /// ⛄ 눈덩이를 뭉침: 그 칸들의 눈을 비움 (모은 양). 장면은 다음 5 Hz에 나감
    pub fn pack_snow(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> f32 {
        if !self.has_snow() {
            return 0.0;
        }
        self.field.pack_snow(x0, x1, y, reach_y)
    }

    /// ⛄🍂 발밑 더미 (x, 발 y 둘레 ±half_w): 그림과 평균 높이 (논리 px). 날씨가 없거나 그 모니터 날씨가 쉬는 중이면 None
    pub fn pile_under(&self, x: f64, y: f64, half_w: f64) -> Option<(Weather, f32)> {
        self.active?;
        if self.overlays.iter().any(|o| o.paused && on_area(&o.wa, x)) {
            return None;
        }
        self.field.pile_under(x, y, half_w)
    }

    /// ⛄🍂 동물이 더미에 뛰어들었거나 뒹굴었음: (x, 발 y) 둘레 ±half의 더미를 비우고 그 알갱이를 위로 확 날림
    pub fn burst_pile(&mut self, app: &AppHandle, x: f64, y: f64, half: f64, dpi: f64) {
        if self.active.is_none() {
            return;
        }
        let Some(kind) = self.field.pile_kind() else { return };
        let got = self.field.sweep(x - half, x + half, y, BURST_REACH_Y * dpi);
        if got > 0.0 {
            let count = ((got * 3.0).ceil() as u32).clamp(8, 30);
            self.puff(app, kind, (x, y - 4.0 * dpi), (0.0, -260.0 * dpi), count);
        }
    }

    /// 빗자루 끝에서 알갱이가 날림 (x, y: 물리 px, 속도: 물리 px/s)
    pub fn puff(&self, app: &AppHandle, kind: Weather, (x, y): (f64, f64), (vx, vy): (f64, f64), count: u32) {
        let Some(o) = self.overlays.iter().find(|o| on_area(&o.wa, x) && y >= o.wa.top && y <= o.wa.bottom + 80.0) else { return };
        let flurry = Flurry { kind, x: (x - o.wa.left) / o.scale, y: (y - o.wa.top) / o.scale, vx: vx / o.scale, vy: vy / o.scale, count };
        let _ = app.emit_to(o.label.as_str(), "weather-flurry", &flurry);
    }

    /// 친구가 쓸어 보낸 더미가 side 끝에서 흩날려 들어옴 (at: 끝을 따라 어디쯤). 날씨 창이 없으면(꺼 뒀거나 맑음) 보이지 않음
    pub fn receive_flurry(&self, app: &AppHandle, kind: Weather, side: Side, at: f64) {
        let at = if at.is_finite() { at.clamp(0.0, 1.0) } else { 0.5 };
        let areas = || self.overlays.iter();
        let picked = match side {
            Side::Left => areas().min_by(|a, b| a.wa.left.total_cmp(&b.wa.left)),
            Side::Right => areas().max_by(|a, b| a.wa.right.total_cmp(&b.wa.right)),
            Side::Up | Side::Down => {
                let (left, right) = areas().fold((f64::MAX, f64::MIN), |(l, r), o| (l.min(o.wa.left), r.max(o.wa.right)));
                let x = left + at * (right - left);
                areas().find(|o| on_area(&o.wa, x)).or_else(|| areas().last())
            }
        };
        let Some(o) = picked else { return };
        let (w, h) = (o.wa.width() / o.scale, o.wa.height() / o.scale);
        let across = |o: &Overlay| {
            let (left, right) = areas().fold((f64::MAX, f64::MIN), |(l, r), o| (l.min(o.wa.left), r.max(o.wa.right)));
            (left + at * (right - left) - o.wa.left) / o.scale
        };
        let (x, y, vx, vy) = match side {
            Side::Left => (0.0, at * h, FLURRY_SPEED, -60.0),
            Side::Right => (w, at * h, -FLURRY_SPEED, -60.0),
            Side::Up => (across(o), 0.0, 0.0, FLURRY_SPEED * 0.5),
            Side::Down => (across(o), h, 0.0, -FLURRY_SPEED),
        };
        let _ = app.emit_to(o.label.as_str(), "weather-flurry", &Flurry { kind, x, y, vx, vy, count: FLURRY_COUNT });
    }

    /// 굴리는 눈덩이를 내 동물 앞에 놓고, 다 쌓으면 눈사람을 세움 (날씨가 켜진 동안 매 틱)
    fn track_own(&mut self, app: &AppHandle, now: Instant, own: &mut Slot) {
        let ball_at = own.pet.snowball_at();
        match (ball_at, &mut self.ball) {
            (Some((x, y)), Some((window, last))) => {
                let pos = bottom_center(x, y, SNOWBALL_SIZE, own.dpi);
                if *last != pos {
                    let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
                    *last = pos;
                }
            }
            (Some((x, y)), None) => {
                self.next_label += 1;
                let size = SNOWBALL_SIZE;
                self.ball = self.sprite(app, "snowball", size, x, y, own.dpi).map(|w| (w, bottom_center(x, y, size, own.dpi)));
            }
            (None, Some(_)) => {
                if let Some((window, _)) = self.ball.take() {
                    let _ = window.destroy();
                }
            }
            (None, None) => {}
        }
        if let Some((x, y)) = own.pet.take_snowman() {
            if let Some(old) = self.snowman.take() {
                let _ = old.window.destroy();
            }
            self.next_label += 1;
            self.snowman = self.sprite(app, "snowman", SNOWMAN_SIZE, x, y, own.dpi).map(|window| Snowman { window, born: now, x });
            own.pet.set_snow_ready(false);
        }
    }

    /// 눈덩이·눈사람 작은 창 — 침 자국·밥그릇처럼 마우스가 통과함 (prop- 창이라 트레이 숨기기/보이기도 따름)
    fn sprite(&self, app: &AppHandle, image: &str, (w, h): (f64, f64), x: f64, y: f64, dpi: f64) -> Option<WebviewWindow> {
        let label = format!("prop-snow-{}", self.next_label);
        let built = crate::overlay::finish(
            WebviewWindowBuilder::new(app, &label, WebviewUrl::App("prop.html".into()))
                .title("ScreenPet Snowman")
                .initialization_script(&format!("window.PROP_IMAGE = {image:?};"))
                .inner_size(w, h),
        );
        let window = match built {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create {image} window: {e}");
                return None;
            }
        };
        let _ = window.set_size(LogicalSize::new(w, h));
        let pos = bottom_center(x, y, (w, h), dpi);
        let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
        crate::overlay::set_click_through(&window, true);
        if !crate::windows_hidden(app) {
            crate::overlay::show_quietly(&window);
        }
        Some(window)
    }

    /// 모니터마다 창 하나 — 작업 영역이 바뀌면 그 창만 다시 만듦 (모니터 연결·해상도·작업표시줄).
    /// 만들기가 실패하면 있는 창은 그대로 두고 빠진 창만 시간을 두고 다시 해 봄 (OverlayRetry)
    fn fit_overlays(&mut self, app: &AppHandle, now: Instant, work_areas: &[Rect]) {
        if self.fitted != work_areas {
            self.fitted = work_areas.to_vec();
            self.retry.reset();
            self.overlays.retain(|o| {
                let keep = work_areas.contains(&o.wa);
                if !keep {
                    let _ = o.window.destroy();
                }
                keep
            });
        }
        if self.overlays.len() >= work_areas.len() || !self.retry.ready(now) {
            return;
        }
        let mut failed = false;
        for wa in work_areas {
            if self.overlays.iter().any(|o| o.wa == *wa) {
                continue;
            }
            let label = format!("{PREFIX}{}", self.next_label);
            self.next_label += 1;
            match crate::overlay::screen_cover(app, &label, "weather.html", "ScreenPet Weather", wa) {
                Some((window, scale)) => self.overlays.push(Overlay { window, label, wa: *wa, scale, paused: false, born: now, resent: now, last: None }),
                None => failed = true,
            }
        }
        if failed {
            self.retry.failed(now);
        } else {
            self.retry.reset();
        }
    }

    /// 날씨가 꺼졌음: 창을 모두 닫고 더미도 잊음 — 이제부터 일이 없음
    fn tear_down(&mut self, slots: &mut [Slot]) {
        for o in self.overlays.drain(..) {
            let _ = o.window.destroy();
        }
        self.fitted.clear();
        self.retry.reset();
        if let Some((window, _)) = self.ball.take() {
            let _ = window.destroy();
        }
        if let Some(s) = self.snowman.take() {
            let _ = s.window.destroy();
        }
        self.field = new_field(self.seed, self.preview);
        for s in slots.iter_mut() {
            s.pet.set_petals(false);
            s.pet.set_snow_ready(false);
        }
    }
}

/// 더미 밭을 새로 만듦 (미리 보기면 빨리 쌓임)
fn new_field(seed: u64, preview: Option<Option<Weather>>) -> PileField {
    PileField::new(seed, if preview.is_some() { PREVIEW_GROW } else { 1.0 })
}

/// x가 이 작업 영역 안인지 봄
fn on_area(wa: &Rect, x: f64) -> bool {
    x >= wa.left && x < wa.right
}

/// 아래 가운데 (x, y)에 놓을 창의 왼쪽 위 물리 좌표를 돌려줌
fn bottom_center(x: f64, y: f64, (w, h): (f64, f64), dpi: f64) -> (i32, i32) {
    ((x - w * dpi / 2.0).round() as i32, (y - h * dpi).round() as i32)
}
