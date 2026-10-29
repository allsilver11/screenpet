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
        let _ = window.show();
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
