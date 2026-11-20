//! 🌈 무지개 구름 자국 — 몸 색이 무지개인 동물(내 흐물이, 무지개 손님)이 달리면 발 뒤로 작은 무지개 구름이 퐁퐁 남음.
//! 언제·어디에 남길지는 screenpet_core::trail이 정하고, 그림은 모니터마다 마우스가 통과하는 투명 창 (`trail-<n>`, trail.html)이
//! 그림 (날씨 창과 같은 overlay::screen_cover). 창은 첫 구름이 나올 때 만들고, 구름이 IDLE_CLOSE 동안 없으면 닫음 —
//! 무지개가 아니거나 달리지 않으면 창도 일도 없음. 숨기기·자리 비움이면 바로 닫고, 그 모니터 앞 창이 전체 화면이면 숨김.
//!
//! 🖍️ 흐물이의 분필 낙서 (`Pet::take_doodle`)도 같은 창이 그림 (`trail-doodle`, ui/doodle.js — 구름 아래 캔버스).
//! 낙서가 남아 있는 동안 (DOODLE_LIFE)은 창을 닫지 않음. 손님 흐물이의 낙서도 그림.
//!
//! 🕷️ 거미인간의 줄 (`Pet::web_line` → `trail-lines`)과 모서리 거미집 (`Pet::take_web` → `trail-webs`)도 같은 창이 그림 (ui/web.js).
//! 거미집이 남아 있는 동안 (WEB_LIFE)은 창을 닫지 않음. 창 모서리 거미집은 창을 따라 옮기고 (`trail-web-move`, 앞 창에 가려지면 숨김),
//! 창이 닫히거나 다른 모니터로 가면 `trail-web-drop`으로 지우고,
//! 🧹 빗자루가 쓸고 지나간 거미집은 `sweep_webs`가 `trail-web-drop`으로, 🧹 다 치우기 (`clear_webs`)와 한 시간마다 (weather::AUTO_CLEAR_SECS) `trail-web-clear`로 모두 지움. 손님 거미인간 것도 그림.

use std::time::{Duration, Instant};

use screenpet_core::brain::{Doodle, DoodleKind, DOODLE_SECS};
use screenpet_core::broom::Broom;
use screenpet_core::geom::Rect;
use screenpet_core::style::PetStyle;
use screenpet_core::trail::{self, Puff, Runner, Trail};
use screenpet_core::weather::{self, OverlayRetry};
use screenpet_core::web::{web_on_window, Corner, WebAt, WebSpin, MAX_WEBS, WEB_LIFE_SECS};
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::slots::Slot;

/// 구름 덮개 창 라벨 앞부분
const PREFIX: &str = "trail-";
/// 마지막 구름 뒤 이만큼 지나면 창을 닫음
const IDLE_CLOSE: Duration = Duration::from_secs(8);
/// 새 창의 웹뷰가 듣기 시작할 때까지 구름을 모아 둠 (웹뷰는 이벤트를 보낼 수 없어서 시간으로 기다림).
/// 모아 둔 구름은 나이(age)와 함께 보내 웹뷰가 그만큼 흐려진 데서 시작함
const WARMUP: Duration = Duration::from_millis(900);
/// 모아 두는 구름 상한 (웹뷰도 MAX_PUFFS까지만 그림)
const PENDING_MAX: usize = 60;
/// 전체 화면 여부를 보는 간격
const PAUSE_EVERY: Duration = Duration::from_millis(250);
/// 🖍️ 낙서가 화면에 있는 시간 (doodle.js: 그리기 DOODLE_SECS + 그대로 20초 + 흐려지기 3초) — 이 동안 창을 닫지 않음
const DOODLE_LIFE: Duration = Duration::from_millis((DOODLE_SECS * 1000.0) as u64 + 23_000);
/// 낙서 높이 = 동물 크기(128 기준 물리 px × scale)의 이만큼
const DOODLE_HEIGHT: f64 = 0.5;
/// 이름 낙서는 이만큼 글자까지 (doodle.js MAX_NAME과 같음)
const MAX_NAME: usize = 12;
/// 🕷️ 거미집이 화면에 있는 시간 (web.js: 남는 시간 life + 흐려지기 WEB_FADE 4초) — 이 동안 창을 닫지 않음
const WEB_LIFE: Duration = Duration::from_secs(WEB_LIFE_SECS as u64 + 4);
/// 🧹 지운 거미집이 흐려지는 동안 (web.js WEB_DROP 0.8초) 창을 더 둠
const WEB_DROP: Duration = Duration::from_millis(1200);
/// 🕷️ 줄을 다시 보내는 가장 짧은 간격 (웹뷰는 받은 줄을 rAF로 그림)
const LINE_EVERY: Duration = Duration::from_millis(33);
/// 🧹 한 시간마다 거미집도 치움 (날씨 더미와 같은 간격)
const WEB_AUTO_CLEAR: Duration = Duration::from_secs(weather::AUTO_CLEAR_SECS as u64);

/// 웹뷰로 가는 거미집 (논리 px, 이 창 왼쪽 위가 0,0 — x·y는 모서리 점, size는 반지름)
#[derive(Serialize, Clone)]
struct WebWire {
    /// 거미집 번호 (지울 때 씀)
    id: u64,
    x: f64,
    y: f64,
    /// 모서리 (top_left·top_right·bottom_left·bottom_right)
    corner: Corner,
    size: f64,
    pixel: bool,
    /// 이미 지난 시간 (초)
    age: f64,
    /// 그대로 남는 시간 (초) — 그 뒤 흐려짐
    life: f64,
}

/// 웹뷰로 가는 거미집 옮기기 (논리 px, 이 창 왼쪽 위가 0,0 — 창을 따라감, hidden이면 앞 창에 가려져 안 그림)
#[derive(Serialize, Clone, Copy)]
struct WebMove {
    id: u64,
    x: f64,
    y: f64,
    hidden: bool,
}

/// 웹뷰로 가는 줄 한 가닥 (논리 px, 손 → 줄 끝)
#[derive(Serialize, Clone, Copy, PartialEq)]
struct LineWire {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// 화면에 있는 거미집 하나의 기억 (창 모서리면 그 창을 따라 지움)
struct WebTrack {
    id: u64,
    /// 그린 덮개 창 라벨
    label: String,
    /// 창 모서리면 그 창 id
    window: Option<u64>,
    /// 모서리 점 (물리 px)
    x: f64,
    y: f64,
    corner: Corner,
    /// 반지름 (물리 px) — 🧹 빗자루가 지나가는지 볼 때
    size: f64,
    /// 앞 창에 가려져 숨김
    hidden: bool,
    /// 다 사라지는 때
    until: Instant,
}

/// 거미집 모서리에서 화면 안쪽으로 1px 들어간 점 (그 모니터 작업 영역 안)
fn inward(w: &WebSpin) -> (f64, f64) {
    match w.corner {
        Corner::TopLeft => (w.x + 1.0, w.y + 1.0),
        Corner::TopRight => (w.x - 1.0, w.y + 1.0),
        Corner::BottomLeft => (w.x + 1.0, w.y - 1.0),
        Corner::BottomRight => (w.x - 1.0, w.y - 1.0),
    }
}

/// 웹뷰로 가는 낙서 (논리 px, 이 창 왼쪽 위가 0,0 — x는 가운데, y는 바닥 줄, size는 높이)
#[derive(Serialize, Clone)]
struct DoodleWire {
    x: f64,
    y: f64,
    kind: &'static str,
    /// kind가 name일 때 쓰는 펫 이름 (MAX_NAME글자까지 — 웹뷰는 캔버스 글자로만 그림)
    name: String,
    facing: f64,
    size: f64,
    pixel: bool,
    /// 분필이 그려지는 시간 (초) = 흐물이 그리기 동작 길이 (그림이 흐물이가 일어설 때 다 그려짐)
    draw: f64,
    /// 이미 지난 시간 (초)
    age: f64,
}

/// 낙서 종류의 웹뷰 이름
fn kind_name(kind: DoodleKind) -> &'static str {
    match kind {
        DoodleKind::Heart => "heart",
        DoodleKind::Smile => "smile",
        DoodleKind::Star => "star",
        DoodleKind::Name => "name",
    }
}

/// 웹뷰로 가는 구름 (논리 px, 이 창 왼쪽 위가 0,0)
#[derive(Serialize, Clone, Copy)]
struct Wire {
    x: f64,
    y: f64,
    size: f64,
    color: u8,
    pixel: bool,
    dir: f64,
    /// 이미 지난 시간 (초)
    age: f64,
}

/// 모니터 하나를 덮는 투명 창임 (구름·낙서를 그림)
struct Overlay {
    window: WebviewWindow,
    label: String,
    wa: Rect,
    scale: f64,
    born: Instant,
    paused: bool,
    pending: Vec<(Instant, Wire)>,
    doodles: Vec<(Instant, DoodleWire)>,
    /// 🕷️ 아직 보내지 않은 거미집
    webs: Vec<(Instant, WebWire)>,
    /// 🕷️ 마지막으로 보낸 줄들
    lines: Vec<LineWire>,
}

/// ☁️ 발자국 구름과 낙서 상태임 (게임 루프가 가짐)
#[derive(Default)]
pub struct TrailFx {
    /// 동물 창 라벨마다 구름 내보내기
    trails: Vec<(String, Trail)>,
    overlays: Vec<Overlay>,
    last_puff: Option<Instant>,
    next_pause: Option<Instant>,
    retry: OverlayRetry,
    next_label: u64,
    puffs: Vec<Puff>,
    /// 마지막 낙서가 사라지는 때 (그때까지 창을 닫지 않음)
    doodles_until: Option<Instant>,
    /// 🕷️ 화면에 있는 거미집들
    webs: Vec<WebTrack>,
    /// 🕷️ 다음 거미집 번호
    next_web: u64,
    /// 🕷️ 마지막 거미집이 사라지는 때 (그때까지 창을 닫지 않음)
    webs_until: Option<Instant>,
    /// 🕷️ 마지막으로 줄이 뻗어 있던 때
    lines_at: Option<Instant>,
    /// 🕷️ 마지막으로 줄을 보낸 때
    lines_sent: Option<Instant>,
    /// 🧹 다음 한 시간 치우기
    web_clear_at: Option<Instant>,
}

impl TrailFx {
    /// 빈 상태로 만듦
    pub fn new() -> Self {
        Self::default()
    }

    /// 게임 루프 스레드에서 매 틱 (창 만들기가 메인 스레드를 기다림)
    pub fn tick(&mut self, app: &AppHandle, now: Instant, dt: f64, away: bool, slots: &mut [Slot]) {
        if away || crate::windows_hidden(app) {
            // 숨기거나 자리를 비운 동안 그린 낙서는 버림
            for s in slots.iter_mut() {
                let _ = s.pet.take_doodle();
                let _ = s.pet.take_web();
            }
            self.trails.clear();
            self.tear_down();
            return;
        }
        self.doodles(app, now, slots);
        self.spider_webs(app, now, slots);
        self.web_lines(app, now, slots);
        self.trails.retain(|(label, _)| slots.iter().any(|s| s.label == *label));
        let Some(own) = slots.first() else { return };
        for s in slots.iter() {
            let rainbow = trail::rainbow(s.tint, s.entry.manifest.tint);
            let found = self.trails.iter().position(|(label, _)| *label == s.label);
            let i = match (rainbow, found) {
                (false, None) => continue,
                (false, Some(i)) => {
                    self.trails.swap_remove(i);
                    continue;
                }
                (true, Some(i)) => i,
                (true, None) => {
                    self.trails.push((s.label.clone(), Trail::new()));
                    self.trails.len() - 1
                }
            };
            let runner = Runner::of(&s.pet, s.shown.style == PetStyle::Pixel);
            // 전체 화면 앞 창은 달릴 때만 봄 (구름이 날 때만 일함)
            let visible = s.on_screen() && runner.dash.is_some() && !weather::overlay_paused(false, &own.world.windows, &s.world.work_area_at(runner.feet.0, runner.feet.1));
            self.trails[i].1.tick(dt, visible, &runner, &mut self.puffs);
        }
        let mut puffs = std::mem::take(&mut self.puffs);
        for p in puffs.drain(..) {
            self.place(app, now, &own.world.work_areas, &p);
        }
        self.puffs = puffs;
        self.flush(app, now);
        self.housekeep(app, now, own);
    }

    /// 🖍️ 동물마다 새 낙서를 받아 그 자리 모니터의 창으로 (손님 것도). 화면 위에 있지 않거나 그 모니터 앞 창이 전체 화면이면 버림
    fn doodles(&mut self, app: &AppHandle, now: Instant, slots: &mut [Slot]) {
        for i in 0..slots.len() {
            let Some(d) = slots[i].pet.take_doodle() else { continue };
            let s = &slots[i];
            let wa = s.world.work_area_at(d.x, d.y);
            if !s.on_screen() || weather::overlay_paused(false, &slots[0].world.windows, &wa) {
                continue;
            }
            let name: String = s.entry.manifest.name.chars().take(MAX_NAME).collect();
            let pixel = s.shown.style == PetStyle::Pixel;
            self.place_doodle(app, now, &s.world.work_areas, &d, name, pixel);
        }
    }

    /// 낙서 하나를 그 자리 모니터의 덮개 창에 보냄 (창이 없으면 만듦)
    fn place_doodle(&mut self, app: &AppHandle, now: Instant, work_areas: &[Rect], d: &Doodle, name: String, pixel: bool) {
        // 바닥 줄이 작업 영역 아래 끝이면 그 모니터 안에 들도록 1px 위로 봄
        let Some(i) = self.overlay_at(app, now, work_areas, d.x, d.y - 1.0) else { return };
        self.doodles_until = Some(now + DOODLE_LIFE);
        let o = &mut self.overlays[i];
        let size = 128.0 * DOODLE_HEIGHT * d.scale / o.scale;
        if !(size.is_finite() && size > 0.0) {
            return;
        }
        let wire = DoodleWire { x: (d.x - o.wa.left) / o.scale, y: (d.y - o.wa.top) / o.scale, kind: kind_name(d.kind), name, facing: d.facing.signum(), size, pixel, draw: DOODLE_SECS, age: 0.0 };
        o.doodles.push((now, wire));
    }

    /// 🕷️ 동물마다 새 거미집을 받아 그 모서리 모니터의 창으로 (손님 것도). 화면 위에 있지 않거나 그 모니터 앞 창이 전체 화면이면 버림
    fn spider_webs(&mut self, app: &AppHandle, now: Instant, slots: &mut [Slot]) {
        for i in 0..slots.len() {
            let Some(w) = slots[i].pet.take_web() else { continue };
            let s = &slots[i];
            let (ix, iy) = inward(&w);
            let wa = s.world.work_area_at(ix, iy);
            if !s.on_screen() || weather::overlay_paused(false, &slots[0].world.windows, &wa) {
                continue;
            }
            let pixel = s.shown.style == PetStyle::Pixel;
            self.place_web(app, now, &s.world.work_areas, &w, pixel);
        }
    }

    /// 거미집 하나를 그 모서리 모니터의 덮개 창에 보냄 (창이 없으면 만듦). 화면에 MAX_WEBS개 넘게 쌓이지 않게 오래된 것부터 지움
    fn place_web(&mut self, app: &AppHandle, now: Instant, work_areas: &[Rect], w: &WebSpin, pixel: bool) {
        let (ix, iy) = inward(w);
        let Some(i) = self.overlay_at(app, now, work_areas, ix, iy) else { return };
        self.next_web += 1;
        let id = self.next_web;
        let o = &mut self.overlays[i];
        let size = w.size / o.scale;
        if !(size.is_finite() && size > 0.0) {
            return;
        }
        let wire = WebWire { id, x: (w.x - o.wa.left) / o.scale, y: (w.y - o.wa.top) / o.scale, corner: w.corner, size, pixel, age: 0.0, life: WEB_LIFE_SECS };
        o.webs.push((now, wire));
        let label = o.label.clone();
        self.webs.push(WebTrack { id, label, window: w.window, x: w.x, y: w.y, corner: w.corner, size: w.size, hidden: false, until: now + WEB_LIFE });
        if self.webs.len() > MAX_WEBS {
            let old = self.webs.remove(0);
            let _ = app.emit_to(old.label.as_str(), "trail-web-drop", [old.id]);
        }
        self.webs_until = self.webs.iter().map(|t| t.until).max();
    }

    /// 🕷️ 지금 뻗은 줄들을 그 손 자리 모니터의 창으로 (바뀔 때만, LINE_EVERY마다까지). 줄이 없어지면 빈 목록을 한 번
    fn web_lines(&mut self, app: &AppHandle, now: Instant, slots: &[Slot]) {
        let mut per: Vec<Vec<LineWire>> = vec![Vec::new(); self.overlays.len()];
        for s in slots.iter() {
            let Some(l) = s.pet.web_line() else { continue };
            if !s.on_screen() {
                continue;
            }
            self.lines_at = Some(now);
            let Some(i) = self.overlay_at(app, now, &s.world.work_areas, l.from.0, l.from.1) else { continue };
            if per.len() <= i {
                per.resize(i + 1, Vec::new());
            }
            let o = &self.overlays[i];
            per[i].push(LineWire { x0: (l.from.0 - o.wa.left) / o.scale, y0: (l.from.1 - o.wa.top) / o.scale, x1: (l.to.0 - o.wa.left) / o.scale, y1: (l.to.1 - o.wa.top) / o.scale });
        }
        if self.lines_sent.is_some_and(|t| now.duration_since(t) < LINE_EVERY) {
            return;
        }
        for (i, o) in self.overlays.iter_mut().enumerate() {
            let want = per.get(i).cloned().unwrap_or_default();
            if want == o.lines || now.duration_since(o.born) < WARMUP {
                continue;
            }
            let _ = app.emit_to(o.label.as_str(), "trail-lines", &want);
            o.lines = want;
            self.lines_sent = Some(now);
        }
    }

    /// 🧹 모든 물건 다 치우기 (그리고 한 시간마다): 거미집을 모두 흐리게 지움
    pub fn clear_webs(&mut self, app: &AppHandle) {
        let now = Instant::now();
        for o in &mut self.overlays {
            o.webs.clear();
            let _ = app.emit_to(o.label.as_str(), "trail-web-clear", ());
        }
        if !self.webs.is_empty() {
            self.webs_until = Some(now + WEB_DROP);
        }
        self.webs.clear();
    }

    /// 🧹 빗자루가 쓸고 지나간 거미집은 흐리게 지움 (dpi = 커서 자리 화면 배율)
    pub fn sweep_webs(&mut self, app: &AppHandle, broom: &Broom, dpi: f64) {
        let mut drops: Vec<(String, u64)> = Vec::new();
        self.webs.retain(|t| {
            let swept = broom.sweeps_web(t.x, t.y, t.corner, t.size, dpi);
            if swept {
                drops.push((t.label.clone(), t.id));
            }
            !swept
        });
        for (label, id) in drops {
            let _ = app.emit_to(label.as_str(), "trail-web-drop", [id]);
        }
    }

    /// 창 모서리 거미집: 창을 옮기거나 크기를 바꾸면 그 모서리를 따라 옮기고 (`trail-web-move`), 앞 창에 가려지면 숨기고,
    /// 창이 닫히거나 최소화되거나 다른 모니터로 가면 지움 (`trail-web-drop`) — 판단은 코어 web::web_on_window
    fn follow_window_webs(&mut self, app: &AppHandle, now: Instant, own: &Slot) {
        let mut drops: Vec<(String, u64)> = Vec::new();
        let mut moves: Vec<(String, WebMove)> = Vec::new();
        let overlays = &self.overlays;
        self.webs.retain_mut(|t| {
            if now >= t.until {
                return false;
            }
            let Some(window) = t.window else { return true };
            let (x, y, hidden) = match web_on_window(&own.world.windows, window, t.corner) {
                WebAt::Gone => {
                    drops.push((t.label.clone(), t.id));
                    return false;
                }
                WebAt::Shown(x, y) => (x, y, false),
                WebAt::Hidden(x, y) => (x, y, true),
            };
            if (x, y, hidden) == (t.x, t.y, t.hidden) {
                return true;
            }
            // 다른 모니터로 갔으면 (그 모서리 안쪽 점이 이 덮개 창 밖) 지움
            let inward = inward(&WebSpin { x, y, corner: t.corner, size: t.size, window: t.window });
            let Some(o) = overlays.iter().find(|o| o.label == t.label && o.wa.contains(inward.0, inward.1)) else {
                drops.push((t.label.clone(), t.id));
                return false;
            };
            moves.push((t.label.clone(), WebMove { id: t.id, x: (x - o.wa.left) / o.scale, y: (y - o.wa.top) / o.scale, hidden }));
            (t.x, t.y, t.hidden) = (x, y, hidden);
            true
        });
        for (label, id) in drops {
            let _ = app.emit_to(label.as_str(), "trail-web-drop", [id]);
        }
        for (label, m) in moves {
            let _ = app.emit_to(label.as_str(), "trail-web-move", [m]);
        }
    }

    /// 그 자리 모니터의 창 (없으면 만듦)
    fn overlay_at(&mut self, app: &AppHandle, now: Instant, work_areas: &[Rect], x: f64, y: f64) -> Option<usize> {
        let wa = work_areas.iter().find(|r| r.left <= x && x < r.right && r.top <= y && y < r.bottom).copied()?;
        if let Some(i) = self.overlays.iter().position(|o| o.wa == wa) {
            return Some(i);
        }
        if !self.retry.ready(now) {
            return None;
        }
        let label = format!("{PREFIX}{}", self.next_label);
        self.next_label += 1;
        let Some((window, scale)) = crate::overlay::screen_cover(app, &label, "trail.html", "ScreenPet Rainbow", &wa) else {
            self.retry.failed(now);
            return None;
        };
        self.retry.reset();
        self.overlays.push(Overlay { window, label, wa, scale, born: now, paused: false, pending: Vec::new(), doodles: Vec::new(), webs: Vec::new(), lines: Vec::new() });
        Some(self.overlays.len() - 1)
    }

    /// 구름 하나를 그 자리 모니터의 창으로 (창이 없으면 만듦)
    fn place(&mut self, app: &AppHandle, now: Instant, work_areas: &[Rect], p: &Puff) {
        let Some(i) = self.overlay_at(app, now, work_areas, p.x, p.y) else { return };
        self.last_puff = Some(now);
        let o = &mut self.overlays[i];
        let wire = Wire { x: (p.x - o.wa.left) / o.scale, y: (p.y - o.wa.top) / o.scale, size: p.size / o.scale, color: p.color, pixel: p.pixel, dir: p.dir, age: 0.0 };
        if o.pending.len() >= PENDING_MAX {
            o.pending.remove(0);
        }
        o.pending.push((now, wire));
    }

    /// 듣기 시작했을 창에 모인 구름을 한 번에 보냄
    fn flush(&mut self, app: &AppHandle, now: Instant) {
        for o in &mut self.overlays {
            if o.pending.is_empty() || now.duration_since(o.born) < WARMUP {
                continue;
            }
            let batch: Vec<Wire> = o.pending.drain(..).map(|(at, w)| Wire { age: now.duration_since(at).as_secs_f64(), ..w }).collect();
            if !o.paused {
                let _ = app.emit_to(o.label.as_str(), "trail-puffs", &batch);
            }
        }
        for o in &mut self.overlays {
            if o.doodles.is_empty() || now.duration_since(o.born) < WARMUP {
                continue;
            }
            let batch: Vec<DoodleWire> = o.doodles.drain(..).map(|(at, w)| DoodleWire { age: now.duration_since(at).as_secs_f64(), ..w }).collect();
            let _ = app.emit_to(o.label.as_str(), "trail-doodle", &batch);
        }
        for o in &mut self.overlays {
            if o.webs.is_empty() || now.duration_since(o.born) < WARMUP {
                continue;
            }
            let batch: Vec<WebWire> = o.webs.drain(..).map(|(at, w)| WebWire { age: now.duration_since(at).as_secs_f64(), ..w }).collect();
            let _ = app.emit_to(o.label.as_str(), "trail-webs", &batch);
        }
    }

    /// 오래 조용하면 창을 닫고, 모니터가 바뀌면 그 창을 닫고, 전체 화면이면 숨김. 🕷️ 움직인 창의 거미집은 지우고, 한 시간마다 다 치움
    fn housekeep(&mut self, app: &AppHandle, now: Instant, own: &Slot) {
        if self.overlays.is_empty() {
            return;
        }
        let puffs_done = self.last_puff.is_none_or(|t| now.duration_since(t) >= IDLE_CLOSE);
        let doodles_done = self.doodles_until.is_none_or(|t| now >= t);
        let webs_done = self.webs_until.is_none_or(|t| now >= t);
        let lines_done = self.lines_at.is_none_or(|t| now.duration_since(t) >= IDLE_CLOSE);
        if puffs_done && doodles_done && webs_done && lines_done {
            self.tear_down();
            return;
        }
        if self.next_pause.is_some_and(|t| now < t) {
            return;
        }
        self.next_pause = Some(now + PAUSE_EVERY);
        self.follow_window_webs(app, now, own);
        let due = *self.web_clear_at.get_or_insert(now + WEB_AUTO_CLEAR);
        if now >= due {
            self.clear_webs(app);
            self.web_clear_at = Some(now + WEB_AUTO_CLEAR);
        }
        let areas = &own.world.work_areas;
        self.overlays.retain(|o| {
            let keep = areas.contains(&o.wa);
            if !keep {
                let _ = o.window.destroy();
            }
            keep
        });
        for o in &mut self.overlays {
            let paused = weather::overlay_paused(false, &own.world.windows, &o.wa);
            if paused != o.paused {
                o.paused = paused;
                if paused { crate::overlay::hide_quietly(&o.window) } else { crate::overlay::show_quietly(&o.window) }
            }
        }
    }

    /// 덮개 창을 모두 닫음
    fn tear_down(&mut self) {
        for o in self.overlays.drain(..) {
            let _ = o.window.destroy();
        }
        self.last_puff = None;
        self.doodles_until = None;
        self.webs.clear();
        self.webs_until = None;
        self.lines_at = None;
        self.lines_sent = None;
        self.next_pause = None;
        self.retry.reset();
    }
}
