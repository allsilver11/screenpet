use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

pub enum Input {
    PointerDown,
    /// 동물 교체 시 걷기 속도 (논리 px/s)
    SetWalkSpeed(f64),
}

pub fn spawn(app: AppHandle, rx: Receiver<Input>, walk_speed: f64) {
    thread::spawn(move || run(app, rx, walk_speed));
}

fn refresh(source: &mut dyn WindowSource, size: f64) -> World {
    World::new(source.windows(), source.work_areas(), size)
}

fn pet_config(scale: f64, logical_walk: f64) -> PetConfig {
    let size = PET_LOGICAL_SIZE * scale;
    PetConfig { size, half_w: size * HALF_WIDTH_RATIO, walk_speed: logical_walk * scale }
}

fn run(app: AppHandle, rx: Receiver<Input>, walk_speed: f64) {
    let Some(window) = app.get_webview_window("pet") else {
        eprintln!("pet window not found; game loop not started");
        return;
    };
    let mut scale = window.scale_factor().unwrap_or(1.0);
    let mut logical_walk = walk_speed;
    let mut size = PET_LOGICAL_SIZE * scale;
    let mut source = default_source();
    let mut world = refresh(source.as_mut(), size);

    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let cfg = pet_config(scale, logical_walk);
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
                Input::SetWalkSpeed(speed) => {
                    logical_walk = speed;
                    pet.cfg.walk_speed = logical_walk * scale;
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
                let cfg = pet_config(scale, logical_walk);
                size = cfg.size;
                pet.cfg = cfg;
            }
            world = refresh(source.as_mut(), size);
            last_refresh = now;
        }

        if pet.is_held() {
            let (cx, cy) = source.cursor();
            if source.left_button_down() {
                pet.pointer_move(cx, cy);
            } else {
                pet.pointer_up(cx, cy);
            }
        }
        pet.update(dt, &world);

        let (wx, wy) = pet.window_pos();
        let pos = (wx.round() as i32, wy.round() as i32);
        if last_pos != Some(pos) {
            let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
            last_pos = Some(pos);
        }
        let render = pet.render();
        if last_render != Some(render) {
            let _ = app.emit("pet-render", render);
            last_render = Some(render);
        }

        thread::sleep(TICK);
    }
}
