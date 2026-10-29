use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use screenpet_core::behavior::{Personality, Special};
use screenpet_core::brain::{Pet, PetConfig};
use screenpet_core::platform::{default_source, WindowSource};
use screenpet_core::world::World;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

const TICK: Duration = Duration::from_millis(16);
const WORLD_REFRESH: Duration = Duration::from_millis(100);
const MAX_DT: f64 = 0.05;
/// 펫 창 한 변 (논리 px) — tauri.conf.json의 width/height와 같아야 한다.
const PET_LOGICAL_SIZE: f64 = 128.0;
/// 몸 폭 절반 / 스프라이트 크기
const HALF_WIDTH_RATIO: f64 = 0.3;
/// 성장 단계 배율 (teenager = 1.0). 2B에서 단계별 값으로 바뀐다.
const STAGE_SCALE: f64 = 1.0;
/// 스프라이트 위 말풍선 자리 (논리 px) — tauri.conf.json height = 128 + 36
const BUBBLE_SPACE: f64 = 36.0;

pub enum Input {
    PointerDown,
    /// 동물 교체: 걷기 속도(논리 px/s)와 특별 동작
    SetPet { walk_speed: f64, special: Option<Special> },
}

pub fn spawn(app: AppHandle, rx: Receiver<Input>, walk_speed: f64, special: Option<Special>) {
    thread::spawn(move || run(app, rx, walk_speed, special));
}

fn refresh(source: &mut dyn WindowSource, size: f64) -> World {
    World::new(source.windows(), source.work_areas(), size)
}

fn pet_config(scale: f64, logical_walk: f64, special: Option<Special>) -> PetConfig {
    let size = PET_LOGICAL_SIZE * STAGE_SCALE * scale;
    // personality and can_climb are wired in Task 6
    PetConfig { size, half_w: size * HALF_WIDTH_RATIO, walk_speed: logical_walk * scale, special, personality: Personality::Friendly, can_climb: false }
}

fn run(app: AppHandle, rx: Receiver<Input>, walk_speed: f64, special: Option<Special>) {
    let Some(window) = app.get_webview_window("pet") else {
        eprintln!("pet window not found; game loop not started");
        return;
    };
    let logical_side = PET_LOGICAL_SIZE * STAGE_SCALE;
    let _ = window.set_size(tauri::LogicalSize::new(logical_side, logical_side + BUBBLE_SPACE));
    let mut scale = window.scale_factor().unwrap_or(1.0);
    let mut logical_walk = walk_speed;
    let mut special = special;
    let mut size = pet_config(scale, walk_speed, special).size;
    let mut source = default_source();
    let mut world = refresh(source.as_mut(), size);

    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let cfg = pet_config(scale, logical_walk, special);
    let b = world.bounds;
    let mut pet = Pet::new(cfg, b.left + b.width() / 2.0, b.top + size, seed);

    let mut last = Instant::now();
    let mut last_refresh = last;
    let mut last_pos = None;
    let mut last_render = None;

    loop {
        while let Ok(input) = rx.try_recv() {
            match input {
                Input::PointerDown => {
                    let (cx, cy) = source.cursor();
                    pet.pointer_down(cx, cy);
                }
                Input::SetPet { walk_speed, special: sp } => {
                    logical_walk = walk_speed;
                    special = sp;
                    pet.cfg = pet_config(scale, logical_walk, special);
                    // 이전 동물의 종 전용 동작이 새 동물에 없을 수 있으니 진행 중이면 끝낸다
                    pet.set_special(special);
                }
            }
        }

        let now = Instant::now();
        let dt = (now - last).as_secs_f64().min(MAX_DT);
        last = now;
        if now - last_refresh >= WORLD_REFRESH {
            let new_scale = window.scale_factor().unwrap_or(scale);
            if new_scale != scale {
                scale = new_scale;
                let cfg = pet_config(scale, logical_walk, special);
                size = cfg.size;
                pet.cfg = cfg;
            }
            world = refresh(source.as_mut(), size);
            last_refresh = now;
        }

        let (cx, cy) = source.cursor();
        if pet.is_held() {
            if source.left_button_down() {
                pet.pointer_move(cx, cy);
            } else {
                pet.pointer_up(cx, cy);
            }
        } else {
            let (sx, sy) = pet.window_pos();
            let side = pet.cfg.size;
            // 걷는 펫이 커서 밑에서 빠져나가지 않도록 사방 20% 여유를 둔다
            let pad = side * 0.2;
            let inside = cx >= sx - pad && cx < sx + side + pad && cy >= sy - pad && cy < sy + side + pad;
            pet.hover(cx, cy, inside);
        }
        pet.update(dt, &world);
        if let Some(speech) = pet.take_speech() {
            let _ = app.emit("pet-say", speech);
        }

        let (wx, wy) = pet.window_pos();
        let pos = (wx.round() as i32, (wy - BUBBLE_SPACE * scale).round() as i32);
        if last_pos != Some(pos) {
            let first = last_pos.is_none();
            let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
            last_pos = Some(pos);
            if first {
                // 창은 visible:false로 만들어 두고, 첫 위치를 잡은 뒤에야 보여 OS 기본 위치 깜빡임을 막는다.
                let _ = window.show();
            }
        }
        let render = pet.render();
        if last_render != Some(render) {
            let _ = app.emit("pet-render", render);
            last_render = Some(render);
        }

        thread::sleep(TICK);
    }
}
