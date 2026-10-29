//! 밥그릇·물그릇·꽃·쿠키 창 (phase 3 먹이 바꾸기) — 내 동물이 먹을 때 그 앞 바닥에 놓이는 작은 투명 창 하나 (`prop`, prop.html).
//! 침 자국 창처럼 마우스는 통과하고 항상 위에 있다. 게임 루프가 Pet::take_prop의 PropEvent대로 부른다:
//! Show = 그 자리에 새로 띄운다 (있던 것은 닫는다), Hide = 흐려지게 한 뒤 닫는다.

use std::time::{Duration, Instant};

use screenpet_core::food::{Prop, PropEvent};
use tauri::{AppHandle, LogicalSize, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const PREFIX: &str = "prop-";
/// 소품 창 크기 (논리 px, teenager 몸 배율 1 기준) — 128px 동물 옆에서 읽히는 크기. 그림은 아래 가운데에 붙는다
const WIDTH: f64 = 56.0;
const HEIGHT: f64 = 48.0;
/// prop.html의 흐려지기(0.6초)가 끝난 뒤 닫는다
const FADE: Duration = Duration::from_millis(700);

/// 그림 파일 이름 (ui/effects/<name>.png, tools/draw_props.py)
fn image(prop: Prop) -> &'static str {
    match prop {
        Prop::FoodBowl => "bowl_food",
        Prop::WaterBowl => "bowl_water",
        Prop::Cookie => "cookie",
        Prop::Flower => "flower",
    }
}

#[derive(Default)]
pub struct Props {
    next: u64,
    /// 지금 놓인 소품 창
    live: Option<WebviewWindow>,
    /// 흐려지는 중인 창과 닫을 시각
    fading: Vec<(WebviewWindow, Instant)>,
}

impl Props {
    /// ev를 반영한다. dpi = 동물 창의 화면 배율, scale = 동물 몸 배율 (단계 × 동물별).
    /// 게임 루프 스레드에서만 부른다 (창 만들기가 메인 스레드를 기다린다)
    pub fn apply(&mut self, app: &AppHandle, ev: PropEvent, dpi: f64, scale: f64) {
        match ev {
            PropEvent::Show { prop, x, y } => {
                if let Some(old) = self.live.take() {
                    let _ = old.destroy();
                }
                self.live = self.spawn(app, prop, x, y, dpi, scale);
            }
            PropEvent::Hide => {
                if let Some(w) = self.live.take() {
                    let _ = w.eval("document.body.classList.add('hide')");
                    self.fading.push((w, Instant::now() + FADE));
                }
            }
        }
    }

    fn spawn(&mut self, app: &AppHandle, prop: Prop, x: f64, y: f64, dpi: f64, scale: f64) -> Option<WebviewWindow> {
        let label = format!("{PREFIX}{}", self.next);
        self.next += 1;
        let (w, h) = (WIDTH * scale, HEIGHT * scale);
        let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("prop.html".into()))
            .title("ScreenPet Food")
            .initialization_script(&format!("window.PROP_IMAGE = {:?};", image(prop)))
            .inner_size(w, h)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .focused(false)
            .visible(false)
            .build();
        let window = match built {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create food prop window: {e}");
                return None;
            }
        };
        // 다른 배율의 모니터에서도 논리 크기를 맞춘다
        let _ = window.set_size(LogicalSize::new(w, h));
        let _ = window.set_position(PhysicalPosition::new((x - w * dpi / 2.0).round() as i32, (y - h * dpi).round() as i32));
        // 클릭·드래그는 그릇을 지나 그 아래(동물·다른 앱)로 간다
        let _ = window.set_ignore_cursor_events(true);
        if !crate::windows_hidden(app) {
            let _ = window.show();
        }
        Some(window)
    }

    /// 다 흐려진 창을 닫는다 (매 틱)
    pub fn tick(&mut self, now: Instant) {
        self.fading.retain(|(w, at)| {
            if now >= *at {
                let _ = w.destroy();
                false
            } else {
                true
            }
        });
    }
}

/// 손에 든 음식 창 한 변 (논리 px) — 그림(ui/effects/hand_*.png, 48×48)을 가운데에
const HAND_SIZE: f64 = 48.0;
/// 받아먹기(냠 냠) 뒤 닫기까지 — prop.html bite 0.8초
const BITE: Duration = Duration::from_millis(900);
/// 거두기(툭 떨어지며 흐려짐) 뒤 닫기까지 — prop.html drop 0.4초
const DROP: Duration = Duration::from_millis(500);

/// 직접 주기: 커서 끝을 따라다니는 음식 그림 창 하나 (`hand-<n>`, prop.html의 hand 모드). 마우스는 통과한다
#[derive(Default)]
pub struct HandWindow {
    next: u64,
    live: Option<WebviewWindow>,
    last_pos: Option<(i32, i32)>,
    fading: Vec<(WebviewWindow, Instant)>,
}

impl HandWindow {
    /// (x, y) = 음식 가운데 (물리 px). 게임 루프 스레드에서만 부른다
    pub fn show(&mut self, app: &AppHandle, icon: &str, at: (f64, f64), dpi: f64) {
        if let Some(old) = self.live.take() {
            let _ = old.destroy();
        }
        let label = format!("hand-{}", self.next);
        self.next += 1;
        let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("prop.html".into()))
            .title("ScreenPet Hand Food")
            .initialization_script(&format!("window.PROP_IMAGE = {icon:?}; window.PROP_MODE = \"hand\";"))
            .inner_size(HAND_SIZE, HAND_SIZE)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .focused(false)
            .visible(false)
            .build();
        let window = match built {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create hand food window: {e}");
                return;
            }
        };
        let _ = window.set_size(LogicalSize::new(HAND_SIZE, HAND_SIZE));
        // 클릭은 음식을 지나 그 아래(동물·다른 앱)로 간다 — 누름은 게임 루프가 전역으로 읽는다
        let _ = window.set_ignore_cursor_events(true);
        self.last_pos = None;
        self.live = Some(window);
        self.move_to(at, dpi);
        if let (Some(w), false) = (&self.live, crate::windows_hidden(app)) {
            let _ = w.show();
        }
    }

    /// 음식 가운데를 (x, y)로 (매 틱)
    pub fn move_to(&mut self, at: (f64, f64), dpi: f64) {
        let Some(w) = &self.live else { return };
        let half = HAND_SIZE / 2.0 * dpi;
        let pos = ((at.0 - half).round() as i32, (at.1 - half).round() as i32);
        if self.last_pos != Some(pos) {
            let _ = w.set_position(PhysicalPosition::new(pos.0, pos.1));
            self.last_pos = Some(pos);
        }
    }

    /// 받아먹었다 — 냠냠 베어 먹히며 사라진다
    pub fn bite(&mut self) {
        self.finish("bite", BITE);
    }

    /// 거뒀다 — 먹지 않고 툭 사라진다
    pub fn drop_back(&mut self) {
        self.finish("drop", DROP);
    }

    fn finish(&mut self, class: &str, after: Duration) {
        if let Some(w) = self.live.take() {
            let _ = w.eval(&format!("document.body.classList.add('{class}')"));
            self.fading.push((w, Instant::now() + after));
        }
    }

    pub fn tick(&mut self, now: Instant) {
        self.fading.retain(|(w, at)| {
            if now >= *at {
                let _ = w.destroy();
                false
            } else {
                true
            }
        });
    }
}
