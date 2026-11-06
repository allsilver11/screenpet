//! 계절 날씨 (v0.3.7) — 모니터마다 마우스가 통과하는 투명 창 하나 (`weather-<n>`, weather.html)와, 눈덩이·눈사람 작은 창.
//! 게임 루프가 매 틱 tick을 부르지만, 날씨가 없으면 (여름 소나기 사이·메뉴로 껐거나·모두 숨겼으면) 창이 하나도 없고
//! 1초에 한 번 플래그만 본다 (달력과 시계는 1분에 한 번). 켜져 있어도 창 목록을 따로 읽지 않는다 — 내 동물 세계의 디딤판을
//! 5 Hz로 맞춰 보고, 장면이 바뀐 모니터에만 보낸다. 더미는 1초에 한 번 (screenpet_core::weather).
//! 그 모니터의 앞 창이 전체 화면이거나 사용자가 자리를 비웠으면 창을 숨기고 웹뷰도 그림을 멈춘다 (돌아오면 다시 그린다).

use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};
use screenpet_core::geom::Rect;
use screenpet_core::visit::Side;
use screenpet_core::weather::{self, OverlayRetry, PileField, Scene, Weather};
use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::slots::Slot;
use crate::AppState;

const PREFIX: &str = "weather-";
/// 개발용 숨은 미리 보기: snow·light_snow·rain·shower·petals·leaves·off (screenpet_core::weather::parse_preview). 메뉴에는 없다
const PREVIEW_ENV: &str = "SCREENPET_WEATHER";
/// 미리 보기에선 더미가 이 배로 빨리 쌓인다
const PREVIEW_GROW: f32 = 10.0;
/// 앱이 뜨고 이만큼 뒤부터 (시작을 방해하지 않게)
const START_DELAY: Duration = Duration::from_secs(2);
const SEASON_EVERY: Duration = Duration::from_secs(60);
/// 날씨가 없을 때 메뉴·숨기기 플래그를 보는 간격
const IDLE_EVERY: Duration = Duration::from_secs(1);
/// 창 목록을 웹뷰에 맞추는 간격 (5 Hz)
const SCENE_EVERY: Duration = Duration::from_millis(200);
const PILES_EVERY: Duration = Duration::from_secs(1);
/// 새 창은 이 시간 동안 1초마다 장면을 다시 보낸다 (첫 장면은 웹뷰가 듣기 전에 지나갈 수 있다)
const FRESH: Duration = Duration::from_secs(5);
/// 소품 창 크기 (논리 px, tools/draw_props.py). 아래 가운데가 자리
const SNOWMAN_SIZE: (f64, f64) = (64.0, 80.0);
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
const FLURRY_SPEED: f64 = 420.0;

#[derive(Serialize, PartialEq)]
struct Payload {
    paused: bool,
    #[serde(flatten)]
    scene: Scene,
}

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

struct Snowman {
    window: WebviewWindow,
    born: Instant,
    x: f64,
    y: f64,
}

pub struct WeatherFx {
    preview: Option<Option<Weather>>,
    season: Option<Weather>,
    active: Option<Weather>,
    next_season: Instant,
    next_scene: Instant,
    next_piles: Instant,
    last_piles: Instant,
    overlays: Vec<Overlay>,
    /// fit_overlays가 마지막으로 맞춘 작업 영역
    fitted: Vec<Rect>,
    /// 날씨 창 만들기가 실패한 뒤 다음 시도까지 기다린다
    retry: OverlayRetry,
    field: PileField,
    seed: u64,
    next_label: u64,
    snowman: Option<Snowman>,
    ball: Option<(WebviewWindow, (i32, i32))>,
}

impl WeatherFx {
    pub fn new(seed: u64) -> Self {
        let preview = std::env::var(PREVIEW_ENV).ok().and_then(|v| weather::parse_preview(&v));
        let now = Instant::now();
        Self {
            preview,
            season: None,
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
        }
    }

    /// 메뉴 체크를 지금 상태에 맞춘다
    pub fn sync_menu(&mut self, app: &AppHandle, menu_off: bool) {
        let _ = app.state::<AppState>().menu.weather.set_checked(!menu_off);
        // 다음 틱에 바로 반영한다
        self.next_scene = Instant::now().min(self.next_scene);
    }

    /// 게임 루프 스레드에서 매 틱 (창 만들기가 메인 스레드를 기다린다)
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
            self.season = weather::weather_at(today.year(), today.month(), today.day(), today.num_seconds_from_midnight());
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
            // 자리를 비운 동안엔 더미도 자라지 않는다 (돌아왔을 때 한꺼번에 쌓여 있지 않게)
            if !away {
                self.field.tick(dt, kind);
            }
        }
        self.fit_overlays(app, now, &own.world.work_areas);
        for o in &mut self.overlays {
            let paused = weather::overlay_paused(away, &own.world.windows, &o.wa);
            if paused != o.paused {
                o.paused = paused;
                let _ = if paused { o.window.hide() } else { o.window.show() };
                if let Some(s) = self.snowman.as_ref().filter(|s| on_area(&o.wa, s.x)) {
                    let _ = if paused { s.window.hide() } else { s.window.show() };
                }
            }
            let payload = Payload { paused, scene: self.field.scene(kind, &o.wa, o.scale) };
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

    /// 🧹 빗자루가 지나갔다: 쓴 양과 더미 그림 (날씨가 없으면 None). 장면은 다음 5 Hz에 나간다
    pub fn sweep(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> Option<(f32, Weather)> {
        self.active?;
        let kind = self.field.pile_kind()?;
        Some((self.field.sweep(x0, x1, y, reach_y), kind))
    }

    /// 빗자루 끝에서 알갱이가 날린다 (x, y: 물리 px, 속도: 물리 px/s)
    pub fn puff(&self, app: &AppHandle, kind: Weather, (x, y): (f64, f64), (vx, vy): (f64, f64), count: u32) {
        let Some(o) = self.overlays.iter().find(|o| on_area(&o.wa, x) && y >= o.wa.top && y <= o.wa.bottom + 80.0) else { return };
        let flurry = Flurry { kind, x: (x - o.wa.left) / o.scale, y: (y - o.wa.top) / o.scale, vx: vx / o.scale, vy: vy / o.scale, count };
        let _ = app.emit_to(o.label.as_str(), "weather-flurry", &flurry);
    }

    /// 친구가 쓸어 보낸 더미가 side 끝에서 흩날려 들어온다 (at: 끝을 따라 어디쯤). 날씨 창이 없으면(꺼 뒀거나 맑음) 보이지 않는다
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

    /// 굴리는 눈덩이를 내 동물 앞에 놓고, 다 쌓으면 눈사람을 세운다 (날씨가 켜진 동안 매 틱)
    fn track_own(&mut self, app: &AppHandle, now: Instant, own: &mut Slot) {
        let ball_at = own.pet.snowball_at();
        match (ball_at, &mut self.ball) {
            (Some((x, y)), Some((window, last))) => {
                let pos = bottom_center(x, y, crate::art::fit(SNOWBALL_SIZE, own.dpi), own.dpi);
                if *last != pos {
                    let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
                    *last = pos;
                }
            }
            (Some((x, y)), None) => {
                self.next_label += 1;
                let size = crate::art::fit(SNOWBALL_SIZE, own.dpi);
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
            self.snowman = self.sprite(app, "snowman", crate::art::fit(SNOWMAN_SIZE, own.dpi), x, y, own.dpi).map(|window| Snowman { window, born: now, x, y });
            own.pet.set_snow_ready(false);
        }
    }

    /// 스타일이 바뀌었다: 눈덩이·눈사람 창을 새 그림·크기로 (창 그림은 만들 때 정해진다).
    /// 눈덩이는 닫아 두면 다음 track_own이 만든다. 눈사람은 같은 자리에, 사는 시간은 그대로
    pub fn restyle(&mut self, app: &AppHandle, dpi: f64) {
        if let Some((window, _)) = self.ball.take() {
            let _ = window.destroy();
        }
        let Some(old) = self.snowman.take() else { return };
        let _ = old.window.destroy();
        self.next_label += 1;
        self.snowman = self.sprite(app, "snowman", crate::art::fit(SNOWMAN_SIZE, dpi), old.x, old.y, dpi).map(|window| Snowman { window, ..old });
        // 그 모니터 날씨가 쉬는 동안(전체 화면 등) 눈사람도 숨어 있다
        if let Some(s) = self.snowman.as_ref().filter(|s| self.overlays.iter().any(|o| o.paused && on_area(&o.wa, s.x))) {
            let _ = s.window.hide();
        }
    }

    /// 눈덩이·눈사람 작은 창 — 침 자국·밥그릇처럼 마우스가 통과한다 (prop- 창이라 트레이 숨기기/보이기도 따른다)
    fn sprite(&self, app: &AppHandle, image: &str, (w, h): (f64, f64), x: f64, y: f64, dpi: f64) -> Option<WebviewWindow> {
        let label = format!("prop-snow-{}", self.next_label);
        let built = crate::overlay::finish(
            WebviewWindowBuilder::new(app, &label, WebviewUrl::App("prop.html".into()))
                .title("ScreenPet Snowman")
                .initialization_script(&format!("window.PROP_IMAGE = {image:?}; {}", crate::art::style_script()))
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
        let _ = window.set_ignore_cursor_events(true);
        if !crate::windows_hidden(app) {
            let _ = window.show();
        }
        Some(window)
    }

    /// 모니터마다 창 하나 — 작업 영역이 바뀌면 그 창만 다시 만든다 (모니터 연결·해상도·작업표시줄).
    /// 만들기가 실패하면 있는 창은 그대로 두고 빠진 창만 시간을 두고 다시 해 본다 (OverlayRetry)
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
            match spawn_overlay(app, &label, wa) {
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

    /// 날씨가 꺼졌다: 창을 모두 닫고 더미도 잊는다 — 이제부터 일이 없다
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

fn new_field(seed: u64, preview: Option<Option<Weather>>) -> PileField {
    PileField::new(seed, if preview.is_some() { PREVIEW_GROW } else { 1.0 })
}

fn on_area(wa: &Rect, x: f64) -> bool {
    x >= wa.left && x < wa.right
}

fn bottom_center(x: f64, y: f64, (w, h): (f64, f64), dpi: f64) -> (i32, i32) {
    ((x - w * dpi / 2.0).round() as i32, (y - h * dpi).round() as i32)
}

/// 이 작업 영역이 있는 모니터의 배율
pub(crate) fn scale_of(app: &AppHandle, wa: &Rect) -> f64 {
    let (cx, cy) = ((wa.left + wa.right) / 2.0, (wa.top + wa.bottom) / 2.0);
    let monitors = app.available_monitors().unwrap_or_default();
    let found = monitors.iter().find(|m| {
        let (p, s) = (m.position(), m.size());
        cx >= p.x as f64 && cx < p.x as f64 + s.width as f64 && cy >= p.y as f64 && cy < p.y as f64 + s.height as f64
    });
    found.map_or(1.0, |m| m.scale_factor())
}

/// 작업 영역을 꼭 덮는 투명 창. 클릭·드래그는 그 아래(동물·다른 앱)로 간다
fn spawn_overlay(app: &AppHandle, label: &str, wa: &Rect) -> Option<(WebviewWindow, f64)> {
    let scale = scale_of(app, wa);
    let built = crate::overlay::finish(
        WebviewWindowBuilder::new(app, label, WebviewUrl::App("weather.html".into()))
            .title("ScreenPet Weather")
            .initialization_script(crate::art::style_script())
            .inner_size(wa.width() / scale, wa.height() / scale),
    );
    let window = match built {
        Ok(w) => w,
        Err(e) => {
            eprintln!("failed to create weather overlay: {e}");
            return None;
        }
    };
    let _ = window.set_position(PhysicalPosition::new(wa.left.round() as i32, wa.top.round() as i32));
    let _ = window.set_size(PhysicalSize::new(wa.width().round() as u32, wa.height().round() as u32));
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.show();
    Some((window, scale))
}
