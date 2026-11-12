//! 🌈 무지개 구름 자국 — 몸 색이 무지개인 동물(내 흐물이, 무지개 손님)이 달리면 발 뒤로 작은 무지개 구름이 퐁퐁 남는다.
//! 언제·어디에 남길지는 screenpet_core::trail이 정하고, 그림은 모니터마다 마우스가 통과하는 투명 창 (`trail-<n>`, trail.html)이
//! 그린다 (날씨 창과 같은 overlay::screen_cover). 창은 첫 구름이 나올 때 만들고, 구름이 IDLE_CLOSE 동안 없으면 닫는다 —
//! 무지개가 아니거나 달리지 않으면 창도 일도 없다. 숨기기·자리 비움이면 바로 닫고, 그 모니터 앞 창이 전체 화면이면 숨긴다.
//!
//! 🖍️ 흐물이의 분필 낙서 (`Pet::take_doodle`)도 같은 창이 그린다 (`trail-doodle`, ui/doodle.js — 구름 아래 캔버스).
//! 낙서가 남아 있는 동안 (DOODLE_LIFE)은 창을 닫지 않는다. 손님 흐물이의 낙서도 그린다.

use std::time::{Duration, Instant};

use screenpet_core::brain::{Doodle, DoodleKind, DOODLE_SECS};
use screenpet_core::geom::Rect;
use screenpet_core::style::PetStyle;
use screenpet_core::trail::{self, Puff, Runner, Trail};
use screenpet_core::weather::{self, OverlayRetry};
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::slots::Slot;

const PREFIX: &str = "trail-";
/// 마지막 구름 뒤 이만큼 지나면 창을 닫는다
const IDLE_CLOSE: Duration = Duration::from_secs(8);
/// 새 창의 웹뷰가 듣기 시작할 때까지 구름을 모아 둔다 (웹뷰는 이벤트를 보낼 수 없어서 시간으로 기다린다).
/// 모아 둔 구름은 나이(age)와 함께 보내 웹뷰가 그만큼 흐려진 데서 시작한다
const WARMUP: Duration = Duration::from_millis(900);
/// 모아 두는 구름 상한 (웹뷰도 MAX_PUFFS까지만 그린다)
const PENDING_MAX: usize = 60;
/// 전체 화면 여부를 보는 간격
const PAUSE_EVERY: Duration = Duration::from_millis(250);
/// 🖍️ 낙서가 화면에 있는 시간 (doodle.js: 그리기 DOODLE_SECS + 그대로 20초 + 흐려지기 3초) — 이 동안 창을 닫지 않는다
const DOODLE_LIFE: Duration = Duration::from_millis((DOODLE_SECS * 1000.0) as u64 + 23_000);
/// 낙서 높이 = 동물 크기(128 기준 물리 px × scale)의 이만큼
const DOODLE_HEIGHT: f64 = 0.5;
/// 이름 낙서는 이만큼 글자까지 (doodle.js MAX_NAME과 같다)
const MAX_NAME: usize = 12;

/// 웹뷰로 가는 낙서 (논리 px, 이 창 왼쪽 위가 0,0 — x는 가운데, y는 바닥 줄, size는 높이)
#[derive(Serialize, Clone)]
struct DoodleWire {
    x: f64,
    y: f64,
    kind: &'static str,
    /// kind가 name일 때 쓰는 펫 이름 (MAX_NAME글자까지 — 웹뷰는 캔버스 글자로만 그린다)
    name: String,
    facing: f64,
    size: f64,
    pixel: bool,
    /// 분필이 그려지는 시간 (초) = 흐물이 그리기 동작 길이 (그림이 흐물이가 일어설 때 다 그려진다)
    draw: f64,
    /// 이미 지난 시간 (초)
    age: f64,
}

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

struct Overlay {
    window: WebviewWindow,
    label: String,
    wa: Rect,
    scale: f64,
    born: Instant,
    paused: bool,
    pending: Vec<(Instant, Wire)>,
    doodles: Vec<(Instant, DoodleWire)>,
}

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
    /// 마지막 낙서가 사라지는 때 (그때까지 창을 닫지 않는다)
    doodles_until: Option<Instant>,
}

impl TrailFx {
    pub fn new() -> Self {
        Self::default()
    }

    /// 게임 루프 스레드에서 매 틱 (창 만들기가 메인 스레드를 기다린다)
    pub fn tick(&mut self, app: &AppHandle, now: Instant, dt: f64, away: bool, slots: &mut [Slot]) {
        if away || crate::windows_hidden(app) {
            // 숨기거나 자리를 비운 동안 그린 낙서는 버린다
            for s in slots.iter_mut() {
                let _ = s.pet.take_doodle();
            }
            self.trails.clear();
            self.tear_down();
            return;
        }
        self.doodles(app, now, slots);
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
            // 전체 화면 앞 창은 달릴 때만 본다 (구름이 날 때만 일한다)
            let visible = s.on_screen() && runner.dash.is_some() && !weather::overlay_paused(false, &own.world.windows, &s.world.work_area_at(runner.feet.0, runner.feet.1));
            self.trails[i].1.tick(dt, visible, &runner, &mut self.puffs);
        }
        let mut puffs = std::mem::take(&mut self.puffs);
        for p in puffs.drain(..) {
            self.place(app, now, &own.world.work_areas, &p);
        }
        self.puffs = puffs;
        self.flush(app, now);
        self.housekeep(now, own);
    }

    /// 🖍️ 동물마다 새 낙서를 받아 그 자리 모니터의 창으로 (손님 것도). 화면 위에 있지 않거나 그 모니터 앞 창이 전체 화면이면 버린다
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

    fn place_doodle(&mut self, app: &AppHandle, now: Instant, work_areas: &[Rect], d: &Doodle, name: String, pixel: bool) {
        // 바닥 줄이 작업 영역 아래 끝이면 그 모니터 안에 들도록 1px 위로 본다
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

    /// 그 자리 모니터의 창 (없으면 만든다)
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
        self.overlays.push(Overlay { window, label, wa, scale, born: now, paused: false, pending: Vec::new(), doodles: Vec::new() });
        Some(self.overlays.len() - 1)
    }

    /// 구름 하나를 그 자리 모니터의 창으로 (창이 없으면 만든다)
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

    /// 듣기 시작했을 창에 모인 구름을 한 번에 보낸다
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
    }

    /// 오래 조용하면 창을 닫고, 모니터가 바뀌면 그 창을 닫고, 전체 화면이면 숨긴다
    fn housekeep(&mut self, now: Instant, own: &Slot) {
        if self.overlays.is_empty() {
            return;
        }
        let puffs_done = self.last_puff.is_none_or(|t| now.duration_since(t) >= IDLE_CLOSE);
        let doodles_done = self.doodles_until.is_none_or(|t| now >= t);
        if puffs_done && doodles_done {
            self.tear_down();
            return;
        }
        if self.next_pause.is_some_and(|t| now < t) {
            return;
        }
        self.next_pause = Some(now + PAUSE_EVERY);
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
                let _ = if paused { o.window.hide() } else { o.window.show() };
            }
        }
    }

    fn tear_down(&mut self) {
        for o in self.overlays.drain(..) {
            let _ = o.window.destroy();
        }
        self.last_puff = None;
        self.doodles_until = None;
        self.next_pause = None;
        self.retry.reset();
    }
}
