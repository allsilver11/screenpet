//! 🛷 썰매 세리머니 창: 루돌프와 산타가 한 화면에서 만나면 둘의 창을 화면 밖에 치워 두고, 썰매 창(ui/sleigh.html)이
//! 화면을 한 바퀴 날아 처음 자리로 돌아온다 (경로·조건은 screenpet_core::sleigh). 둘은 나는 동안 멈춰 있다.
//! 창은 활성화 없이 띄우고 옮긴다 (Windows에서 tao의 set_position·show는 창을 활성화해 포커스를 뺏는다).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use screenpet_core::style::PetStyle;
use screenpet_core::sleigh::{self, Lap, Pose, COOLDOWN_SECS, PULLER, RIDER};
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::slots::Slot;

const LABEL: &str = "sleigh";

/// 썰매에 탄 동물 하나 (웹뷰가 시트에서 그린다)
#[derive(Debug, Clone, Serialize)]
struct Rider {
    id: String,
    stage: &'static str,
    pixel: bool,
    /// 한 변 (논리 px)
    side: f64,
}

#[derive(Debug, Clone, Serialize)]
struct Setup {
    puller: Rider,
    rider: Rider,
    /// 큰 동물 한 변 (논리 px)
    unit: f64,
    pixel: bool,
    /// 썰매 아래 가운데(도는 축)가 창 아래에서 떨어진 높이 (논리 px) — 기울어도 안 잘리게
    below: f64,
}

/// 웹뷰에 보내는 모습 (`sleigh-pose`)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct PosePayload {
    facing: f64,
    tilt: f64,
}

struct Ride {
    labels: (String, String),
    lap: Lap,
    window: WebviewWindow,
    hwnd: Option<isize>,
    /// 창 크기 (논리 px)
    size: (f64, f64),
    /// 축이 창 아래에서 떨어진 높이 (논리 px)
    below: f64,
    last_pose: Option<PosePayload>,
}

#[derive(Default)]
pub struct SleighFx {
    ride: Option<Ride>,
    /// 짝(끄는 쪽 창, 타는 쪽 창) → 다시 날 수 있는 때
    cooldown: HashMap<(String, String), Instant>,
}

impl SleighFx {
    pub fn new() -> Self {
        Self::default()
    }

    /// 이 창의 동물이 썰매에 타 있다 (움직이지 않고 창은 치워 둔다)
    pub fn is_riding(&self, label: &str) -> bool {
        self.ride.as_ref().is_some_and(|r| r.labels.0 == label || r.labels.1 == label)
    }

    /// 매 틱 (동물 update 뒤, 창 놓기 전). blocked = 지금은 시작하지 않는다 (숨기기·똥피하기 등)
    pub fn tick(&mut self, app: &AppHandle, dt: f64, slots: &mut [Slot], blocked: bool) {
        if self.ride.is_some() {
            self.fly(dt, slots, blocked);
        } else if !blocked {
            self.find(app, slots);
        }
    }

    fn find(&mut self, app: &AppHandle, slots: &mut [Slot]) {
        let now = Instant::now();
        self.cooldown.retain(|_, until| *until > now);
        let free = |s: &Slot| s.on_screen() && sleigh::ready(&s.pet);
        let Some(i) = slots.iter().position(|s| s.entry.id == PULLER && free(s)) else { return };
        let Some(j) = slots.iter().position(|s| s.entry.id == RIDER && free(s)) else { return };
        let (a, b) = (&slots[i].pet, &slots[j].pet);
        if !sleigh::met((a.body.x, a.body.y, a.cfg.size), (b.body.x, b.body.y, b.cfg.size)) {
            return;
        }
        let labels = (slots[i].label.clone(), slots[j].label.clone());
        if self.cooldown.contains_key(&labels) {
            return;
        }
        let start = ((a.body.x + b.body.x) / 2.0, a.body.y.max(b.body.y));
        let area = slots[i].world.work_area_at(start.0, start.1);
        let rider = |s: &Slot| Rider {
            id: s.entry.id.clone(),
            stage: s.shown.stage.name(),
            pixel: s.shown.style == PetStyle::Pixel,
            side: screenpet_core::brain::PET_LOGICAL_SIZE * s.shown.scale,
        };
        let (puller, rider) = (rider(&slots[i]), rider(&slots[j]));
        let unit = puller.side.max(rider.side);
        let b = sleigh::window_box(unit);
        let size = (b.width, b.height);
        let setup = Setup { pixel: puller.pixel && rider.pixel, puller, rider, unit, below: b.below };
        let Some(window) = build_window(app, &setup, size) else { return };
        self.cooldown.insert(labels.clone(), now + Duration::from_secs_f64(COOLDOWN_SECS));
        for k in [i, j] {
            slots[k].park();
        }
        eprintln!("sleigh: {} and {} take off", labels.0, labels.1);
        let hwnd = crate::overlay::hwnd_of(&window);
        let mut ride = Ride { labels, lap: Lap::new(start, area), window, hwnd, size, below: setup.below, last_pose: None };
        ride.place();
        self.ride = Some(ride);
    }

    fn fly(&mut self, dt: f64, slots: &mut [Slot], blocked: bool) {
        let Some(ride) = self.ride.as_mut() else { return };
        let here = |label: &str| slots.iter().any(|s| s.label == label && s.on_screen());
        // 다 돌았거나, 하나가 떠났거나(손님이 집에 감·하우스), 모두 숨겼다
        let done = ride.lap.tick(dt) || !here(&ride.labels.0) || !here(&ride.labels.1) || blocked;
        if !done {
            ride.place();
            return;
        }
        let ride = self.ride.take().expect("checked above");
        let _ = ride.window.destroy();
        for s in slots.iter_mut().filter(|s| s.label == ride.labels.0 || s.label == ride.labels.1) {
            s.unpark();
        }
    }
}

impl Ride {
    /// 썰매 날의 가운데 아래를 지금 자리에 둔다
    fn place(&mut self) {
        let pose: Pose = self.lap.pose();
        let dpi = self.window.scale_factor().unwrap_or(1.0);
        let (w, h) = self.size;
        let x = (pose.x - w * dpi / 2.0).round() as i32;
        // 축(썰매 아래 가운데)이 창 아래에서 below만큼 위에 있다
        let y = (pose.y - (h - self.below) * dpi).round() as i32;
        crate::overlay::move_quietly(&self.window, self.hwnd, x, y);
        let payload = PosePayload { facing: pose.facing, tilt: (pose.tilt * 2.0).round() / 2.0 };
        if self.last_pose != Some(payload) {
            self.last_pose = Some(payload);
            let _ = self.window.emit_to(LABEL, "sleigh-pose", payload);
        }
    }
}

fn build_window(app: &AppHandle, setup: &Setup, (w, h): (f64, f64)) -> Option<WebviewWindow> {
    let json = serde_json::to_string(setup).ok()?;
    let built = crate::overlay::finish_quiet(
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("sleigh.html".into()))
            .title("ScreenPet Sleigh")
            .initialization_script(&format!("window.SLEIGH = {json};"))
            .inner_size(w, h),
    );
    match built {
        Ok(window) => {
            // 클릭은 썰매를 지나 아래로 간다
            let _ = window.set_ignore_cursor_events(true);
            Some(window)
        }
        Err(e) => {
            eprintln!("failed to create sleigh window: {e}");
            None
        }
    }
}
