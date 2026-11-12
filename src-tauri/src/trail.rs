//! 🌈 무지개 구름 자국 — 몸 색이 무지개인 동물(내 흐물이, 무지개 손님)이 달리면 발 뒤로 작은 무지개 구름이 퐁퐁 남는다.
//! 언제·어디에 남길지는 screenpet_core::trail이 정하고, 그림은 모니터마다 마우스가 통과하는 투명 창 (`trail-<n>`, trail.html)이
//! 그린다 (날씨 창과 같은 overlay::screen_cover). 창은 첫 구름이 나올 때 만들고, 구름이 IDLE_CLOSE 동안 없으면 닫는다 —
//! 무지개가 아니거나 달리지 않으면 창도 일도 없다. 숨기기·자리 비움이면 바로 닫고, 그 모니터 앞 창이 전체 화면이면 숨긴다.

use std::time::{Duration, Instant};

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
}

impl TrailFx {
    pub fn new() -> Self {
        Self::default()
    }

    /// 게임 루프 스레드에서 매 틱 (창 만들기가 메인 스레드를 기다린다)
    pub fn tick(&mut self, app: &AppHandle, now: Instant, dt: f64, away: bool, slots: &[Slot]) {
        if away || crate::windows_hidden(app) {
            self.trails.clear();
            self.tear_down();
            return;
        }
        self.trails.retain(|(label, _)| slots.iter().any(|s| s.label == *label));
        let Some(own) = slots.first() else { return };
        for s in slots {
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

    /// 구름 하나를 그 자리 모니터의 창으로 (창이 없으면 만든다)
    fn place(&mut self, app: &AppHandle, now: Instant, work_areas: &[Rect], p: &Puff) {
        let Some(wa) = work_areas.iter().find(|r| r.left <= p.x && p.x < r.right && r.top <= p.y && p.y < r.bottom).copied() else { return };
        self.last_puff = Some(now);
        let i = match self.overlays.iter().position(|o| o.wa == wa) {
            Some(i) => i,
            None => {
                if !self.retry.ready(now) {
                    return;
                }
                let label = format!("{PREFIX}{}", self.next_label);
                self.next_label += 1;
                let Some((window, scale)) = crate::overlay::screen_cover(app, &label, "trail.html", "ScreenPet Rainbow", &wa) else {
                    self.retry.failed(now);
                    return;
                };
                self.retry.reset();
                self.overlays.push(Overlay { window, label, wa, scale, born: now, paused: false, pending: Vec::new() });
                self.overlays.len() - 1
            }
        };
        let o = &mut self.overlays[i];
        let wire = Wire { x: (p.x - wa.left) / o.scale, y: (p.y - wa.top) / o.scale, size: p.size / o.scale, color: p.color, pixel: p.pixel, dir: p.dir, age: 0.0 };
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
    }

    /// 오래 조용하면 창을 닫고, 모니터가 바뀌면 그 창을 닫고, 전체 화면이면 숨긴다
    fn housekeep(&mut self, now: Instant, own: &Slot) {
        if self.overlays.is_empty() {
            return;
        }
        if self.last_puff.is_none_or(|t| now.duration_since(t) >= IDLE_CLOSE) {
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
        self.next_pause = None;
        self.retry.reset();
    }
}
