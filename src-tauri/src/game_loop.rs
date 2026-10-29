use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use screenpet_core::behavior::Special;
use screenpet_core::brain::{Pet, PetConfig};
use screenpet_core::care::{Care, FeedResult, Food};
use screenpet_core::config::{self, Config};
use screenpet_core::manifest::FavoriteFood;
use screenpet_core::platform::{default_source, WindowSource};
use screenpet_core::world::World;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

use crate::house::{self, HouseState};
use crate::pets::PetEntry;
use crate::AppState;

const TICK: Duration = Duration::from_millis(16);
const WORLD_REFRESH: Duration = Duration::from_millis(100);
const SAVE_EVERY: Duration = Duration::from_secs(60);
const MAX_DT: f64 = 0.05;
/// teenager(배율 1.0) 스프라이트 한 변 (논리 px)
const PET_LOGICAL_SIZE: f64 = 128.0;
/// 몸 폭 절반 / 스프라이트 크기
const HALF_WIDTH_RATIO: f64 = 0.3;
/// 스프라이트 위 말풍선·음식 버튼 자리 (논리 px)
const BUBBLE_SPACE: f64 = 36.0;

pub enum Input {
    PointerDown,
    /// 동물 교체 — 돌보기 상태는 게임 루프가 가진 설정에서 찾는다
    SetPet { entry: PetEntry },
    Feed(Food),
    ToggleLock,
    ResetGrowth,
    /// 하우스로 들어가기 / 나오기
    ToggleHouse,
    /// 하우스 창을 눌렀다 (배고프면 음식 고르기, 아니면 손 흔들기)
    HouseClick,
    /// 저장하고 앱 종료
    Quit,
}

/// 웹뷰에 보내는 현재 동물과 크기 (`pet-stage` 이벤트, `current_stage` 커맨드)
#[derive(Debug, Clone, Serialize)]
pub struct StagePayload {
    pub pet: String,
    /// 시트 이름 (`sheet_<stage>.png`)
    pub stage: &'static str,
    /// 몸 크기 배율 = 단계 배율 × 동물별 배율
    pub scale: f64,
}

/// 몸 크기 배율 = 단계 배율 × 동물별 배율 (곰은 크고 토끼는 작다)
pub fn body_scale(entry: &PetEntry, care: &Care) -> f64 {
    care.stage.scale() * entry.manifest.size_mult(care.stage)
}

pub fn stage_payload(entry: &PetEntry, care: &Care) -> StagePayload {
    StagePayload { pet: entry.id.clone(), stage: care.stage.name(), scale: body_scale(entry, care) }
}

/// 좋아하는 음식 (매니페스트에 없으면 별)
pub fn favorite_of(entry: &PetEntry) -> FavoriteFood {
    entry.manifest.favorite_food.clone().unwrap_or_else(|| FavoriteFood { emoji: "⭐".into(), name: "좋아하는 것".into() })
}

pub fn spawn(app: AppHandle, rx: Receiver<Input>, entry: PetEntry, config: Config) {
    thread::spawn(move || run(app, rx, entry, config));
}

fn refresh(source: &mut dyn WindowSource, size: f64) -> World {
    World::new(source.windows(), source.work_areas(), size)
}

/// 펫 창 크기 (논리 px): 너비 128 × max(1, s), 높이 36 + 128 × s
fn window_logical_size(body_scale: f64) -> (f64, f64) {
    (PET_LOGICAL_SIZE * body_scale.max(1.0), BUBBLE_SPACE + PET_LOGICAL_SIZE * body_scale)
}

fn pet_config(dpi: f64, entry: &PetEntry, body_scale: f64) -> PetConfig {
    let m = &entry.manifest;
    let size = PET_LOGICAL_SIZE * body_scale * dpi;
    PetConfig {
        size,
        half_w: size * HALF_WIDTH_RATIO,
        // 큰 몸은 조금 빠르게, 작은 몸은 조금 느리게 (크기에 정비례하면 너무 차이 난다)
        walk_speed: m.walk_speed * body_scale.sqrt() * dpi,
        special: Special::from_manifest(|n| m.has_anim(n)),
        personality: m.personality,
        can_climb: m.can_climb(),
    }
}

/// 설정 저장 — 게임 루프만 설정을 쓴다 (단일 작성자). 하우스 여부는 config.housed에 직접 적어 둔다
fn save_config(app: &AppHandle, config: &mut Config, pet_id: &str, care: &Care) {
    config.pet = Some(pet_id.to_string());
    config.care.insert(pet_id.to_string(), care.clone());
    let state = app.state::<AppState>();
    if let Err(e) = config::save(&state.config_dir, config) {
        eprintln!("failed to save config: {e}");
    }
}

/// 트레이·우클릭 메뉴의 돌보기·하우스 항목을 현재 동물과 상태에 맞춘다
fn sync_menu(app: &AppHandle, entry: &PetEntry, care: &Care, housed: bool) {
    let state = app.state::<AppState>();
    let _ = state.menu.favorite.set_text(crate::tray::favorite_label(&favorite_of(entry)));
    let _ = state.menu.lock.set_checked(care.locked);
    let _ = state.menu.house.set_text(crate::tray::house_label(housed));
}

/// 지금 말풍선·음식 고르기를 띄울 창: 하우스 안이면 하우스 창, 아니면 펫 창
fn speech_target(pet: &Pet) -> &'static str {
    if pet.is_housed() {
        house::LABEL
    } else {
        "pet"
    }
}

/// 단계(또는 동물)가 바뀌었을 때: 창 크기, 펫 크기, 웹뷰 시트. 적용한 몸 배율을 돌려준다.
fn apply_stage(app: &AppHandle, window: &WebviewWindow, pet: &mut Pet, dpi: f64, entry: &PetEntry, care: &Care) -> f64 {
    let s = body_scale(entry, care);
    let (w, h) = window_logical_size(s);
    let _ = window.set_size(tauri::LogicalSize::new(w, h));
    pet.cfg = pet_config(dpi, entry, s);
    let payload = stage_payload(entry, care);
    *app.state::<AppState>().stage.lock().unwrap() = payload.clone();
    let _ = app.emit("pet-stage", payload);
    s
}

fn run(app: AppHandle, rx: Receiver<Input>, entry: PetEntry, config: Config) {
    let Some(window) = app.get_webview_window("pet") else {
        eprintln!("pet window not found; game loop not started");
        return;
    };
    let house_window = app.get_webview_window(house::LABEL);
    let mut config = config;
    let mut entry = entry;
    let mut care = config.care.get(&entry.id).cloned().unwrap_or_default();
    let mut dpi = window.scale_factor().unwrap_or(1.0);
    let mut source = default_source();

    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let cfg = pet_config(dpi, &entry, body_scale(&entry, &care));
    let mut world = refresh(source.as_mut(), cfg.size);
    let b = world.bounds;
    let mut pet = Pet::new(cfg, b.left + b.width() / 2.0, b.top + cfg.size, seed);
    // 화면에 적용된 몸 배율 — 성장 직후 먹는 동안에는 care.stage보다 한 단계 늦다
    let mut shown_scale = apply_stage(&app, &window, &mut pet, dpi, &entry, &care);
    pet.set_hungry(care.is_hungry());
    if config.housed {
        // 지난번에 하우스 안에서 끝났다 — 하우스 안에서 시작한다
        let spot = house::spot(&app, &world.work_areas, dpi);
        // 몸은 문 앞 바닥에 둔다 (화면 위에서 떨어지던 자리가 아니라)
        pet.body.x = spot.door_x;
        if let Some((_, ay)) = world.anchor(spot.ground) {
            pet.body.y = ay;
        }
        pet.enter_house();
        if let Some(hw) = &house_window {
            house::show_at(hw, &spot);
        }
    }
    sync_menu(&app, &entry, &care, config.housed);
    // 하우스 안에서 시작하면 펫 창은 숨긴 채로 둔다
    let mut need_show = !pet.is_housed();
    let mut was_housed = pet.is_housed();
    let mut last_house: Option<HouseState> = None;

    let mut last = Instant::now();
    let mut last_refresh = last;
    let mut last_save = last;
    let mut last_pos = None;
    let mut last_render = None;
    // 먹고 한 단계 컸지만 아직 먹는 중 — 먹기가 끝나면(축하 시작 또는 중단) 크기를 바꾼다
    let mut pending_stage = false;

    loop {
        while let Ok(input) = rx.try_recv() {
            match input {
                Input::PointerDown => {
                    let (cx, cy) = source.cursor();
                    pet.pointer_down(cx, cy);
                }
                Input::SetPet { entry: next } => {
                    save_config(&app, &mut config, &entry.id, &care);
                    entry = next;
                    care = config.care.get(&entry.id).cloned().unwrap_or_default();
                    pending_stage = false;
                    shown_scale = apply_stage(&app, &window, &mut pet, dpi, &entry, &care);
                    // 이전 동물의 종 전용 동작·벽 타기가 새 동물에 없을 수 있으니 진행 중이면 끝낸다
                    pet.set_special(pet.cfg.special);
                    pet.set_can_climb(pet.cfg.can_climb);
                    pet.set_hungry(care.is_hungry());
                    // 하우스 안이면 새 동물도 하우스 안에 있다 (같은 Pet이므로 상태가 그대로 이어진다)
                    sync_menu(&app, &entry, &care, config.housed);
                    save_config(&app, &mut config, &entry.id, &care);
                }
                Input::Feed(food) => {
                    let before = care.clone();
                    let result = care.feed(food);
                    if pet.feed_result(result, food) {
                        if let FeedResult::Ate { leveled_up: true } = result {
                            pending_stage = true;
                        }
                        save_config(&app, &mut config, &entry.id, &care);
                    } else {
                        // 공중·드래그 중이라 못 먹었다 — 먹지 않은 것으로 되돌린다
                        care = before;
                    }
                    // 먹는 도중 클릭이 다시 음식을 조르지 않도록 바로 반영
                    pet.set_hungry(care.is_hungry());
                }
                Input::ToggleLock => {
                    care.locked = !care.locked;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춘다
                    sync_menu(&app, &entry, &care, config.housed);
                    save_config(&app, &mut config, &entry.id, &care);
                }
                Input::ResetGrowth => {
                    care.reset();
                    pending_stage = false;
                    shown_scale = apply_stage(&app, &window, &mut pet, dpi, &entry, &care);
                    pet.set_hungry(care.is_hungry());
                    save_config(&app, &mut config, &entry.id, &care);
                }
                Input::ToggleHouse => {
                    let spot = house::spot(&app, &world.work_areas, dpi);
                    if pet.is_housed() || pet.is_going_home() {
                        // 문 앞에서 왼쪽으로 걸어 나온다 (가는 중이었다면 그 자리에서 멈춘다)
                        let was_inside = pet.is_housed();
                        pet.leave_house(&world, spot.door_x, spot.ground);
                        if was_inside {
                            need_show = true;
                        }
                        if let Some(hw) = &house_window {
                            let _ = hw.hide();
                        }
                        config.housed = false;
                    } else {
                        // 집이 먼저 나타나고, 동물이 집으로 달려간다
                        if let Some(hw) = &house_window {
                            house::show_at(hw, &spot);
                        }
                        pet.go_home(spot.ground, spot.door_x);
                        // 펫 창에 떠 있던 음식 고르기는 닫는다
                        let _ = app.emit_to("pet", "hide-picker", ());
                        config.housed = true;
                    }
                    sync_menu(&app, &entry, &care, config.housed);
                    save_config(&app, &mut config, &entry.id, &care);
                }
                Input::HouseClick => {
                    if pet.is_housed() {
                        if care.is_hungry() {
                            let _ = app.emit_to(house::LABEL, "food-picker", favorite_of(&entry));
                        } else {
                            let _ = app.emit_to(house::LABEL, "house-wave", ());
                        }
                    }
                }
                Input::Quit => {
                    save_config(&app, &mut config, &entry.id, &care);
                    app.exit(0);
                    return;
                }
            }
        }

        let now = Instant::now();
        let dt = (now - last).as_secs_f64().min(MAX_DT);
        last = now;
        care.tick(dt);
        if now - last_refresh >= WORLD_REFRESH {
            let new_dpi = window.scale_factor().unwrap_or(dpi);
            if new_dpi != dpi {
                dpi = new_dpi;
                pet.cfg = pet_config(dpi, &entry, shown_scale);
            }
            world = refresh(source.as_mut(), pet.cfg.size);
            pet.set_hungry(care.is_hungry());
            last_refresh = now;
        }
        if now - last_save >= SAVE_EVERY {
            save_config(&app, &mut config, &entry.id, &care);
            last_save = now;
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
            let _ = app.emit_to(speech_target(&pet), "pet-say", speech);
        }
        if pet.take_food_request() {
            let _ = app.emit_to(speech_target(&pet), "food-picker", favorite_of(&entry));
        }
        if pet.is_housed() != was_housed {
            was_housed = pet.is_housed();
            app.state::<AppState>().housed.store(was_housed, Ordering::Relaxed);
            if was_housed {
                // 문에 닿았다 — 펫 창을 숨기고 하우스 창 문간에 보인다
                let _ = window.hide();
            }
        }
        // 지금 보이는 시트·크기 (성장 직후 먹는 동안엔 care.stage보다 한 단계 늦다)
        let shown = app.state::<AppState>().stage.lock().unwrap().clone();
        let house_state = HouseState {
            pet: shown.pet,
            stage: shown.stage,
            scale: shown.scale,
            hungry: care.is_hungry(),
            housed: pet.is_housed(),
        };
        if last_house.as_ref() != Some(&house_state) {
            *app.state::<AppState>().house.lock().unwrap() = house_state.clone();
            let _ = app.emit_to(house::LABEL, "house-state", house_state.clone());
            last_house = Some(house_state);
        }

        let render = pet.render();
        // 먹다가 눌린 동안(Pressed)도 먹는 중으로 본다 — 손을 떼면 계속 먹으므로 크기를 미리 바꾸지 않는다
        if pending_stage && !pet.is_eating() {
            pending_stage = false;
            shown_scale = apply_stage(&app, &window, &mut pet, dpi, &entry, &care);
        }

        let (_, sy) = pet.window_pos();
        let win_w = PET_LOGICAL_SIZE * shown_scale.max(1.0) * dpi;
        let pos = ((pet.body.x - win_w / 2.0).round() as i32, (sy - BUBBLE_SPACE * dpi).round() as i32);
        if last_pos != Some(pos) {
            let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
            last_pos = Some(pos);
        }
        if need_show && !pet.is_housed() {
            // 창은 visible:false로 만들어 두고, 위치를 잡은 뒤에야 보여 OS 기본 위치 깜빡임을 막는다
            // (하우스에서 나올 때도 문 앞으로 옮긴 뒤에 보인다).
            let _ = window.show();
            need_show = false;
        }
        if last_render != Some(render) {
            let _ = app.emit("pet-render", render);
            last_render = Some(render);
        }

        thread::sleep(TICK);
    }
}
