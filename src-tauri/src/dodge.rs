//! 💩 똥피하기 — 내 동물이 있는 모니터를 덮는 투명 창(`dodge`, dodge.html)에 똥이 떨어진다 (규칙은 screenpet_core::dodge).
//! 게임 동안 이 창이 키보드를 받아 (다른 앱에 키가 가지 않는다) 내 동물을 조작 모드로 움직이고, Esc·닫기면 원래대로.
//! 최고 기록은 설정에 남기고, 새 기록이면 연결된 친구들에게 알린다 (network.rs).
use std::sync::Mutex;
use std::time::{Duration, Instant};

use screenpet_core::brain::control_moves::ControlInput;
use screenpet_core::collide::{Face, Hit};
use screenpet_core::config::Config;
use screenpet_core::dodge::{self, Dodge, Phase, PetBox, Rank};
use screenpet_core::geom::Rect;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::network::Network;
use crate::slots::Slot;
use crate::AppState;

pub const LABEL: &str = "dodge";
/// 게임을 할 수 없을 때 내 동물 말풍선
const NOT_NOW: &str = "지금은 못 해요";
/// 새 기록은 마지막 판 뒤 이만큼 지나 한 번만 알린다 (친구의 부탁 예산을 이름·초대와 같이 쓴다)
const SHARE_AFTER: Duration = Duration::from_secs(10);

/// 웹뷰 → 게임 루프
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DodgeInput {
    Start,
    Close,
    /// 눌린 키 (ControlInput::bits)
    Keys(u8),
}

/// 창에 보낼 내용 (좌표는 이 창 왼쪽 위 기준 논리 px)
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DodgeView {
    /// "ready" | "play" | "over"
    phase: &'static str,
    /// [id, x, y, 반지름]
    poops: Vec<[f64; 4]>,
    dodged: u32,
    best: u32,
    /// 방금 판이 최고 기록
    new_best: bool,
    ranking: Vec<Rank>,
}

/// 창이 처음 열릴 때 읽는 지금 내용 (그 뒤로는 `dodge-state` 이벤트)
static VIEW: Mutex<Option<DodgeView>> = Mutex::new(None);

struct Board {
    game: Dodge,
    window: WebviewWindow,
    /// 창이 있는 모니터 배율
    scale: f64,
    keys: ControlInput,
    new_best: bool,
    last: Option<DodgeView>,
}

#[derive(Default)]
pub struct DodgeFx {
    board: Option<Board>,
    /// 친구들에게 알릴 새 기록이 있다: 이때 알린다
    share_at: Option<Instant>,
}

impl DodgeFx {
    pub fn new() -> Self {
        Self::default()
    }

    /// 판이 열려 있다 (시작 전·끝난 뒤 포함)
    pub fn is_open(&self) -> bool {
        self.board.is_some()
    }

    /// 메뉴 "💩 똥피하기": 내 동물이 있는 모니터를 덮는 창. 이미 열려 있으면 앞으로
    pub fn open(&mut self, app: &AppHandle, own: &Slot, config: &Config) {
        if let Some(b) = &self.board {
            let _ = b.window.set_focus();
            return;
        }
        if !can_play(own) {
            let _ = app.emit_to(crate::game_loop::speech_target(&own.pet), "pet-say-text", NOT_NOW);
            return;
        }
        let area = own.world.work_area_at(own.pet.body.x, own.pet.body.y);
        let scale = crate::weather::scale_of(app, &area);
        let Some(window) = spawn(app, &area, scale) else { return };
        let game = Dodge::new(area, scale, fastrand::u64(..));
        self.board = Some(Board { game, window, scale, keys: ControlInput::default(), new_best: false, last: None });
        self.publish(app, config);
    }

    /// 창에서 온 것 (기록은 tick에서 남긴다)
    pub fn handle(&mut self, app: &AppHandle, input: DodgeInput, own: &mut Slot, config: &Config) {
        let Some(b) = self.board.as_mut() else { return };
        match input {
            DodgeInput::Keys(bits) => b.keys = ControlInput::from_bits(bits),
            DodgeInput::Start => {
                // 그사이 판 밖(다른 모니터)으로 갔거나 하우스에 들어갔으면 시작하지 않는다
                let area = b.game.area();
                let inside = own.pet.body.x >= area.left && own.pet.body.x < area.right;
                if !(can_play(own) && inside) {
                    let _ = app.emit_to(crate::game_loop::speech_target(&own.pet), "pet-say-text", NOT_NOW);
                } else if b.game.start() {
                    b.new_best = false;
                    b.keys = ControlInput::default();
                    own.pet.set_control(true);
                }
            }
            DodgeInput::Close => {
                self.close(own);
                return;
            }
        }
        self.publish(app, config);
    }

    /// 창을 닫는다 (하던 판은 기록 없이 끝)
    pub fn close(&mut self, own: &mut Slot) {
        let Some(b) = self.board.take() else { return };
        if b.game.phase() == Phase::Play {
            own.pet.set_control(false);
        }
        let _ = b.window.destroy();
        *VIEW.lock().unwrap() = None;
    }

    /// 게임 동안 키를 내 동물에게 (게임 루프가 동물 update 전에). 조작 모드가 저절로 끝났으면 다시 켠다
    pub fn steer(&mut self, own: &mut Slot, dt: f64) {
        let Some(b) = self.board.as_ref().filter(|b| b.game.phase() == Phase::Play) else { return };
        if !own.pet.is_controlled() {
            own.pet.set_control(true);
        }
        own.pet.control_keys(b.keys, dt);
    }

    /// 매 틱 (동물 update 뒤). 최고 기록이 바뀌었으면 true (저장 필요)
    pub fn tick(&mut self, app: &AppHandle, dt: f64, own: &mut Slot, config: &mut Config, network: &Network) -> bool {
        if self.share_at.is_some_and(|at| Instant::now() >= at) {
            self.share_at = None;
            network.share_dodge_best(config.dodge_best);
        }
        let Some(b) = self.board.as_mut() else { return false };
        // 창이 닫혔다 (Alt+F4 등)
        if app.get_webview_window(LABEL).is_none() {
            self.close(own);
            return false;
        }
        // 게임 중 하우스에 들어갔거나 친구에게 갔다: 기록 없이 닫는다
        if b.game.phase() == Phase::Play && !can_play(own) {
            self.close(own);
            return false;
        }
        let pet = (own.on_screen() && !own.pet.is_away()).then(|| PetBox::of(&own.pet));
        let mut dirty = false;
        if let Some(over) = b.game.tick(dt, pet) {
            own.pet.set_control(false);
            if over.hit {
                // 머리에 똥: "ew!" 하고 움찔
                own.pet.feel_hit(Hit { face: Face::Top, vx: 0.0, vy: 0.0, dir: own.pet.facing, fast: false, poop: true });
            }
            if config.record_dodge(over.score) {
                b.new_best = true;
                self.share_at = Some(Instant::now() + SHARE_AFTER);
                dirty = true;
            }
        }
        self.publish(app, config);
        dirty
    }

    /// 바뀐 내용만 창에 보낸다
    fn publish(&mut self, app: &AppHandle, config: &Config) {
        let Some(b) = self.board.as_mut() else { return };
        let area = b.game.area();
        let s = b.scale;
        let view = DodgeView {
            phase: match b.game.phase() {
                Phase::Ready => "ready",
                Phase::Play => "play",
                Phase::Over => "over",
            },
            poops: b.game.poops().iter().map(|p| [p.id as f64, (p.x - area.left) / s, (p.y - area.top) / s, p.r / s]).collect(),
            dodged: b.game.dodged(),
            best: config.dodge_best,
            new_best: b.new_best,
            ranking: ranking(config),
        };
        if b.last.as_ref() == Some(&view) {
            return;
        }
        *VIEW.lock().unwrap() = Some(view.clone());
        let _ = app.emit_to(LABEL, "dodge-state", &view);
        b.last = Some(view);
    }
}

/// 내 동물이 화면에 있다 (친구 화면에 가 있거나 하우스 안·가는 중이면 못 한다)
fn can_play(own: &Slot) -> bool {
    own.on_screen() && !own.pet.is_away() && !own.pet.is_housed() && !own.pet.is_going_home()
}

/// 나 + 기록을 알려 준 친구들
fn ranking(config: &Config) -> Vec<Rank> {
    // 📱 한 사람의 두 컴퓨터는 한 줄 (더 좋은 기록)
    let friends: Vec<(String, u32)> = config.dodge_friends();
    dodge::ranking(&config.my_name(), config.dodge_best, &friends)
}

/// 작업 영역을 꼭 덮는 투명 창 — 마우스·키를 받는다 (게임 동안 다른 앱에 가지 않게)
fn spawn(app: &AppHandle, wa: &Rect, scale: f64) -> Option<WebviewWindow> {
    let built = crate::overlay::finish(
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("dodge.html".into()))
            .title("ScreenPet 똥피하기")
            .initialization_script(crate::art::style_script())
            .inner_size(wa.width() / scale, wa.height() / scale),
    );
    let window = match built {
        Ok(w) => w,
        Err(e) => {
            eprintln!("failed to create dodge window: {e}");
            return None;
        }
    };
    let _ = window.set_position(PhysicalPosition::new(wa.left.round() as i32, wa.top.round() as i32));
    let _ = window.set_size(PhysicalSize::new(wa.width().round() as u32, wa.height().round() as u32));
    let _ = window.show();
    let _ = window.set_focus();
    Some(window)
}

fn from_dodge_window(window: &WebviewWindow) -> bool {
    window.label() == LABEL
}

/// 똥피하기 창의 지금 내용 (첫 로드용)
#[tauri::command]
pub fn dodge_state(window: WebviewWindow) -> Result<DodgeView, String> {
    if !from_dodge_window(&window) {
        return Err("wrong window".into());
    }
    Ok(VIEW.lock().unwrap().clone().unwrap_or_default())
}

/// 눌린 키가 바뀌었다 (←1 →2 ↑4 ↓8 Space16 Shift32)
#[tauri::command]
pub fn dodge_keys(window: WebviewWindow, state: tauri::State<AppState>, bits: u8) {
    if from_dodge_window(&window) {
        state.send(crate::game_loop::Input::Dodge(DodgeInput::Keys(bits)));
    }
}

/// 시작 / 다시 하기
#[tauri::command]
pub fn dodge_start(window: WebviewWindow, state: tauri::State<AppState>) {
    if from_dodge_window(&window) {
        state.send(crate::game_loop::Input::Dodge(DodgeInput::Start));
    }
}

/// 닫기 (Esc)
#[tauri::command]
pub fn dodge_close(window: WebviewWindow, state: tauri::State<AppState>) {
    if from_dodge_window(&window) {
        state.send(crate::game_loop::Input::Dodge(DodgeInput::Close));
    }
}
