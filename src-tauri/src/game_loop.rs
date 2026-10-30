use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use screenpet_core::brain::Pet;
use screenpet_core::care::{Care, FeedResult, Food, Stage};
use screenpet_core::config::{self, Config};
use screenpet_core::food::{self, FeedMode, HandFeed, HandInput};
use screenpet_core::manifest::FavoriteFood;
use screenpet_core::platform::default_source;
use screenpet_core::social::Social;
use screenpet_core::visit::Side;
use screenpet_core::world::World;
use tauri::{AppHandle, Emitter, Manager};

use crate::decal::Decals;
use crate::house::{self, HouseState};
use crate::network::{NetInput, Network};
use crate::pets::PetEntry;
use crate::prop::{HandWindow, Props};
use crate::slots::{self, GuestInfo, Role, Slot, StagePayload, OWN_LABEL};
use crate::AppState;

const TICK: Duration = Duration::from_millis(16);
const WORLD_REFRESH: Duration = Duration::from_millis(100);
const SAVE_EVERY: Duration = Duration::from_secs(60);
const MAX_DT: f64 = 0.05;
/// slots[OWN] = 내 동물 (항상 맨 앞), 그 뒤는 손님
const OWN: usize = 0;
/// 이 환경 변수가 있으면 시작할 때 번들 동물 하나를 손님으로 들인다 (네트워크 없이 여러 창 확인용)
const DEBUG_GUEST_ENV: &str = "SCREENPET_DEBUG_GUEST";

pub enum Input {
    /// 창(label)의 동물을 눌렀다
    PointerDown { label: String },
    /// 동물 교체 — 돌보기 상태는 게임 루프가 가진 설정에서 찾는다
    SetPet { entry: PetEntry },
    Feed(Food),
    /// 밥 주기 방법 (직접 주기 / 자동 주기)
    SetFeedMode(FeedMode),
    ToggleLock,
    ResetGrowth,
    /// 하우스로 들어가기 / 나오기
    ToggleHouse,
    /// 하우스 창을 눌렀다 (배고프면 음식 고르기, 아니면 손 흔들기)
    HouseClick,
    /// 하우스를 끌어 옮겼다 (house.rs). save = 다 놓았음 — 새 자리를 설정에 적는다
    HouseMoved { save: bool },
    /// 손님(label)을 주인에게 돌려보낸다 (손님 우클릭 메뉴)
    SendHome { label: String },
    /// 친구 네트워크 (네트워크 이벤트, 트레이 `친구랑 놀기`, 짝맞추기 창)
    Net(NetInput),
    /// 저장하고 앱 종료
    Quit,
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

/// 지금 내 동물의 말풍선·음식 고르기를 띄울 창: 하우스 안이면 하우스 창, 아니면 펫 창
fn speech_target(pet: &Pet) -> &'static str {
    if pet.is_housed() {
        house::LABEL
    } else {
        OWN_LABEL
    }
}

/// 내 동물의 단계(또는 동물)가 바뀌었을 때: 창 크기, 펫 크기, 웹뷰 시트
fn apply_own_stage(app: &AppHandle, own: &mut Slot, care: &Care) {
    let payload = stage_payload(&own.entry, care);
    own.set_stage(app, payload);
}

fn seed_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

/// 디버그: 번들 동물 하나(되도록 내 동물과 다른 종)를 오른쪽 끝에서 손님으로 들인다
fn spawn_debug_guest(app: &AppHandle, slots: &mut Vec<Slot>) {
    let state = app.state::<AppState>();
    let own_id = &slots[OWN].entry.id;
    let Some(entry) = state.pets.iter().find(|p| &p.id != own_id).or(state.pets.first()).cloned() else { return };
    let stage = Stage::Teenager;
    let scale = stage.scale() * entry.manifest.size_mult(stage);
    let guest = GuestInfo { visit_id: "debug".into(), owner_id: "debug".into(), owner_name: "디버그 친구".into() };
    match slots::spawn_guest(app, guest, entry, stage, scale, Side::Right, &slots[OWN].world, seed_now()) {
        Ok(slot) => slots.push(slot),
        Err(e) => eprintln!("failed to create debug guest window: {e}"),
    }
}

fn run(app: AppHandle, rx: Receiver<Input>, entry: PetEntry, config: Config) {
    let Some(window) = app.get_webview_window(OWN_LABEL) else {
        eprintln!("pet window not found; game loop not started");
        return;
    };
    let house_window = app.get_webview_window(house::LABEL);
    let mut config = config;
    house::restore(config.house_spot.clone());
    let mut care = config.care.get(&entry.id).cloned().unwrap_or_default();
    let dpi = window.scale_factor().unwrap_or(1.0);
    let mut source = default_source();

    let shown = stage_payload(&entry, &care);
    let cfg = slots::pet_config(dpi, &entry, shown.scale);
    let world = World::new(source.windows(), source.work_areas(), cfg.size);
    let b = world.bounds;
    let pet = Pet::new(cfg, b.left + b.width() / 2.0, b.top + cfg.size, seed_now());
    let mut slots = vec![Slot::new(OWN_LABEL.to_string(), Role::Own, entry, pet, window, world, dpi, shown.clone())];
    // 창 크기·웹뷰 시트를 맞춘다
    slots[OWN].set_stage(&app, shown);
    slots[OWN].pet.set_hungry(care.is_hungry());
    if config.housed {
        // 지난번에 하우스 안에서 끝났다 — 하우스 안에서 시작한다
        let own = &mut slots[OWN];
        let spot = house::spot(&app, &own.world.work_areas, own.dpi);
        // 몸은 문 앞 바닥에 둔다 (화면 위에서 떨어지던 자리가 아니라)
        own.pet.body.x = spot.door_x;
        if let Some((_, ay)) = own.world.anchor(spot.ground) {
            own.pet.body.y = ay;
        }
        own.pet.enter_house();
        if let Some(hw) = &house_window {
            house::show_at(hw, &spot);
        }
    }
    sync_menu(&app, &slots[OWN].entry, &care, config.housed);
    let mut was_housed = slots[OWN].pet.is_housed();
    let mut was_away = false;
    let mut last_house: Option<HouseState> = None;
    if std::env::var_os(DEBUG_GUEST_ENV).is_some() {
        spawn_debug_guest(&app, &mut slots);
    }
    let (mut network, new_id) = Network::start(&app, &mut config);
    if new_id {
        save_config(&app, &mut config, &slots[OWN].entry.id, &care);
    }
    // 내 동물이 친구 화면에 가 있거나 가는 중 (하우스·동물 바꾸기 메뉴를 막는다)
    let mut was_busy = false;

    let mut last = Instant::now();
    let mut last_refresh = last;
    let mut last_save = last;
    // 먹고 한 단계 컸지만 아직 먹는 중 — 먹기가 끝나면(축하 시작 또는 중단) 크기를 바꾼다
    let mut pending_stage = false;
    // 꿀벌이 쏜 침 자국 창들
    let mut decals = Decals::default();
    // 내 동물이 먹을 때 앞에 놓이는 밥그릇·물그릇·꽃·쿠키 창
    let mut props = Props::default();
    // 직접 주기: 커서 끝에 붙은 음식과 그 그림 창
    let mut hand: Option<HandFeed> = None;
    let mut hand_win = HandWindow::default();
    // 한 화면의 동물끼리(내 동물 + 손님) 인사·술래 놀이
    let mut social = Social::new(seed_now());

    loop {
        while let Ok(input) = rx.try_recv() {
            match input {
                Input::PointerDown { label } => {
                    // 한 번에 한 마리만 잡는다. 음식을 들고 있는 동안의 누름은 주기·거두기 (아래 매 틱 처리)
                    if hand.is_some() || slots.iter().any(|s| s.pet.is_held()) {
                        continue;
                    }
                    if let Some(i) = slots::find(&slots, &label) {
                        let (cx, cy) = source.cursor();
                        slots[i].pet.pointer_down(cx, cy);
                    }
                }
                Input::SetPet { entry: next } => {
                    let own = &mut slots[OWN];
                    // 친구 화면에 가 있거나 가는 중엔 바꾸지 않는다 (트레이 체크만 되돌린다)
                    if network.own_busy(own) {
                        crate::tray::sync_pet_checks(&app.state::<AppState>().menu, &own.entry.id);
                        continue;
                    }
                    save_config(&app, &mut config, &own.entry.id, &care);
                    // 들고 있던 음식은 거둔다 (좋아하는 음식 그림이 동물마다 다르다)
                    if hand.take().is_some() {
                        own.pet.end_hand();
                        hand_win.drop_back();
                        app.state::<AppState>().hand_feeding.store(false, Ordering::Relaxed);
                    }
                    own.entry = next;
                    care = config.care.get(&own.entry.id).cloned().unwrap_or_default();
                    pending_stage = false;
                    apply_own_stage(&app, own, &care);
                    // 이전 동물의 종 전용 동작·벽 타기가 새 동물에 없을 수 있으니 진행 중이면 끝낸다
                    own.pet.set_special(own.pet.cfg.special);
                    own.pet.set_can_climb(own.pet.cfg.can_climb);
                    own.pet.set_hungry(care.is_hungry());
                    // 하우스 안이면 새 동물도 하우스 안에 있다 (같은 Pet이므로 상태가 그대로 이어진다)
                    sync_menu(&app, &own.entry, &care, config.housed);
                    save_config(&app, &mut config, &own.entry.id, &care);
                }
                Input::Feed(food) => {
                    let own = &mut slots[OWN];
                    // 직접 주기: 음식이 커서 끝에 붙는다 (먹이기는 동물 입에 닿을 때). 하우스 안·친구 화면에선 그릇으로
                    if config.feed_mode.by_hand(food) && own.on_screen() {
                        let (cx, cy) = source.cursor();
                        let off = food::HAND_OFFSET * own.dpi;
                        if hand.is_some() {
                            own.pet.end_hand();
                        }
                        hand = Some(HandFeed::new(food));
                        hand_win.show(&app, food::hand_icon(food, &own.entry.id), (cx + off, cy + off), own.dpi);
                        app.state::<AppState>().hand_feeding.store(true, Ordering::Relaxed);
                        let _ = app.emit_to(OWN_LABEL, "hide-picker", ());
                        continue;
                    }
                    if own.pet.is_housed() {
                        // 하우스 안이면 문 앞에 그릇이 놓이고 동물이 나와서 먹는다
                        let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                        own.pet.set_house_door(spot.ground, spot.door_x);
                    }
                    let before = care.clone();
                    let result = care.feed(food);
                    if own.pet.feed_result(result, food) {
                        if let FeedResult::Ate { leveled_up: true } = result {
                            pending_stage = true;
                        }
                        save_config(&app, &mut config, &own.entry.id, &care);
                    } else {
                        // 공중·드래그·놀러 가는 중이라 못 먹었다 — 먹지 않은 것으로 되돌린다
                        care = before;
                    }
                    // 먹는 도중 클릭이 다시 음식을 조르지 않도록 바로 반영
                    own.pet.set_hungry(care.is_hungry());
                }
                Input::SetFeedMode(mode) => {
                    config.feed_mode = mode;
                    crate::tray::sync_feed_mode(&app.state::<AppState>().menu, mode);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::ToggleLock => {
                    care.locked = !care.locked;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춘다
                    sync_menu(&app, &slots[OWN].entry, &care, config.housed);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::ResetGrowth => {
                    let own = &mut slots[OWN];
                    care.reset();
                    pending_stage = false;
                    apply_own_stage(&app, own, &care);
                    own.pet.set_hungry(care.is_hungry());
                    save_config(&app, &mut config, &own.entry.id, &care);
                }
                Input::ToggleHouse => {
                    let own = &mut slots[OWN];
                    // 친구 화면에 가 있거나 가는 중엔 하우스를 다루지 않는다
                    if network.own_busy(own) {
                        sync_menu(&app, &own.entry, &care, config.housed);
                        continue;
                    }
                    let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                    // 하우스에서 나와 먹는 중(다 먹으면 들어갈 참)도 "하우스에서 나오기"다
                    if own.pet.is_housed() || own.pet.is_going_home() || own.pet.returns_home() {
                        // 문 앞에서 왼쪽으로 걸어 나온다 (가는 중이었다면 그 자리에서 멈춘다)
                        let was_inside = own.pet.is_housed();
                        own.pet.leave_house(&own.world, spot.door_x, spot.ground);
                        if was_inside {
                            own.need_show = true;
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
                        own.pet.go_home(spot.ground, spot.door_x);
                        // 펫 창에 떠 있던 음식 고르기는 닫는다
                        let _ = app.emit_to(OWN_LABEL, "hide-picker", ());
                        config.housed = true;
                    }
                    sync_menu(&app, &own.entry, &care, config.housed);
                    save_config(&app, &mut config, &own.entry.id, &care);
                }
                Input::HouseClick => {
                    if slots[OWN].pet.is_housed() {
                        if care.is_hungry() {
                            let _ = app.emit_to(house::LABEL, "food-picker", favorite_of(&slots[OWN].entry));
                        } else {
                            let _ = app.emit_to(house::LABEL, "house-wave", ());
                        }
                    }
                }
                Input::HouseMoved { save } => {
                    // 문 자리가 집을 따라간다: 집에 가는 중·나와서 먹는 중이면 새 문으로 (하던 일은 끊지 않는다),
                    // 안에 있으면 그대로 안에 (먹이 그릇은 새 문 앞에)
                    let own = &mut slots[OWN];
                    let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                    own.pet.set_house_door(spot.ground, spot.door_x);
                    if save {
                        config.house_spot = house::placed();
                        save_config(&app, &mut config, &own.entry.id, &care);
                    }
                }
                Input::SendHome { label } => {
                    if let Some(i) = slots::find(&slots, &label).filter(|&i| !slots[i].is_own()) {
                        slots[i].pet.go_home_now();
                    }
                }
                Input::Net(NetInput::SendTo(id)) => {
                    // 놀러 보내기: 하우스 안이면 먼저 문 앞으로 나온 뒤 그 친구 쪽 끝으로 간다
                    let own = &mut slots[OWN];
                    if !network.can_send(own, &id) {
                        continue;
                    }
                    if own.pet.is_housed() || own.pet.is_going_home() || own.pet.returns_home() {
                        let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                        let was_inside = own.pet.is_housed();
                        own.pet.leave_house(&own.world, spot.door_x, spot.ground);
                        if was_inside {
                            own.need_show = true;
                        }
                        if let Some(hw) = &house_window {
                            let _ = hw.hide();
                        }
                        config.housed = false;
                        sync_menu(&app, &own.entry, &care, config.housed);
                        save_config(&app, &mut config, &own.entry.id, &care);
                    }
                    network.send_to(&config, own, &id);
                }
                Input::Net(input) => {
                    if network.handle(&app, input, &mut config, &mut slots) {
                        save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                    }
                }
                Input::Quit => {
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
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
            let (windows, work_areas) = (source.windows(), source.work_areas());
            for s in &mut slots {
                s.refresh(&windows, &work_areas);
            }
            slots[OWN].pet.set_hungry(care.is_hungry());
            last_refresh = now;
        }
        if now - last_save >= SAVE_EVERY {
            save_config(&app, &mut config, &slots[OWN].entry.id, &care);
            last_save = now;
        }

        // 커서·간질이기·드래그는 커서 아래 맨 위 한 마리에게만
        let (cx, cy) = source.cursor();
        // 직접 주기: 음식이 커서를 따라가고, 내 동물 입에 닿으면 먹는다. 오른쪽 버튼·Esc·빈 곳 클릭·30초면 거둔다
        if let Some(h) = hand.as_mut() {
            let own = &mut slots[OWN];
            let off = food::HAND_OFFSET * own.dpi;
            let at = (cx + off, cy + off);
            hand_win.move_to(at, own.dpi);
            let input = h.tick(dt, source.left_button_down(), source.right_button_down(), source.escape_down());
            let food = h.food;
            let ready = input != HandInput::Cancel && own.on_screen() && own.pet.offer_hand(at.0, at.1);
            let mut done = false;
            if ready {
                let before = care.clone();
                let result = care.feed(food);
                if own.pet.eat_from_hand(result, food) {
                    if let FeedResult::Ate { leveled_up } = result {
                        pending_stage |= leveled_up;
                        hand_win.bite();
                        done = true;
                        save_config(&app, &mut config, &own.entry.id, &care);
                    }
                    // 배불러 갸웃 — 음식은 손에 남는다
                } else {
                    care = before;
                }
                own.pet.set_hungry(care.is_hungry());
            } else {
                // 동물 위를 누른 건 거두기가 아니다 (닿을 때까지 더 가까이)
                let empty_click = input == HandInput::Click && !own.cursor_inside(cx, cy);
                if input == HandInput::Cancel || empty_click || !own.on_screen() {
                    own.pet.end_hand();
                    hand_win.drop_back();
                    done = true;
                }
            }
            if done {
                hand = None;
                app.state::<AppState>().hand_feeding.store(false, Ordering::Relaxed);
            }
        }
        let held = slots.iter().position(|s| s.pet.is_held());
        let hovered = if held.is_some() { None } else { slots::hover_target(&slots, cx, cy) };
        for (i, s) in slots.iter_mut().enumerate() {
            if Some(i) == held {
                if source.left_button_down() {
                    s.pet.pointer_move(cx, cy);
                } else {
                    s.pet.pointer_up(cx, cy);
                }
            } else {
                s.pet.hover(cx, cy, Some(i) == hovered);
            }
        }

        // 가까이 마주친 동물끼리 인사·술래 놀이 (손길·먹이·집 생각은 각 동물 update가 먼저 챙긴다)
        social.tick(dt, &mut slots);
        // 연결된 친구가 있는 끝으로만 나간다
        network.set_exit_edges(&config, &mut slots);
        let mut own_exit = None;
        let mut left = Vec::new();
        for (i, s) in slots.iter_mut().enumerate() {
            // 친구 화면에 가 있는 동안엔 물리·행동이 멈춘다
            if !s.pet.is_away() {
                s.pet.update(dt, &s.world);
            }
            if let Some(speech) = s.pet.take_speech() {
                let target = if s.is_own() { speech_target(&s.pet) } else { s.label.as_str() };
                let _ = app.emit_to(target, "pet-say", speech);
            }
            // 꿀벌이 화면을 쐈다 — 그 자리에 침 자국
            if let Some((x, y)) = s.pet.take_sting() {
                decals.spawn(&app, x, y, s.dpi);
            }
            // 밥그릇·꽃 놓기·거두기 (손님은 먹이를 받지 않는다)
            if let Some(ev) = s.pet.take_prop() {
                if s.is_own() {
                    props.apply(&app, ev, s.dpi, s.shown.scale);
                }
            }
            if let Some(side) = s.pet.take_exit() {
                if s.is_own() {
                    own_exit = Some(side);
                } else {
                    // 손님이 집 쪽(또는 다른 친구 쪽) 끝으로 걸어 나갔거나 손 흔들고 사라졌다
                    left.push((i, side));
                }
            }
        }
        for (i, side) in left.into_iter().rev() {
            let mut slot = slots.remove(i);
            // 던져서 넘어갔으면 그 움직임 (주인 화면이나 다음 친구 화면으로 날아간다)
            let fling = slot.pet.take_fling();
            if let Role::Guest(g) = &slot.role {
                eprintln!("guest {} ({}) left", g.visit_id, g.owner_id);
                network.guest_left(&config, g, side, slot.pet.guest(), fling);
            }
            slots::close(&app, slot);
        }

        if let Some(side) = own_exit {
            // 내 동물이 화면 끝으로 다 나갔다 — 그 끝의 친구에게 놀러 간다
            network.own_exited(&config, &mut slots[OWN], side);
        }
        network.tick(&app, now, &config, &mut slots[OWN]);
        decals.tick(now);
        props.tick(now);
        hand_win.tick(now);
        let own = &mut slots[OWN];
        let busy = network.own_busy(own);
        if busy != was_busy {
            was_busy = busy;
            crate::tray::set_own_busy(&app.state::<AppState>().menu, busy);
        }
        if own.pet.take_food_request() {
            let _ = app.emit_to(speech_target(&own.pet), "food-picker", favorite_of(&own.entry));
        }
        if own.pet.is_housed() != was_housed {
            was_housed = own.pet.is_housed();
            app.state::<AppState>().housed.store(was_housed, Ordering::Relaxed);
            if was_housed {
                // 문에 닿았다 — 펫 창을 숨기고 하우스 창 문간에 보인다
                let _ = own.window.hide();
            } else {
                // 하우스에서 나왔다 (나오기, 또는 문 앞 그릇으로 먹으러) — 문 앞 자리로 옮긴 뒤 보인다
                own.need_show = true;
            }
        }
        if own.pet.is_away() != was_away {
            was_away = own.pet.is_away();
            app.state::<AppState>().away.store(was_away, Ordering::Relaxed);
            if was_away {
                // 화면 끝으로 걸어 나갔다 — 돌아올 때까지 펫 창을 숨긴다
                let _ = own.window.hide();
            } else {
                // 돌아왔다 — 들어오는 자리로 옮긴 뒤 보인다
                own.need_show = true;
            }
        }
        // 지금 보이는 시트·크기 (성장 직후 먹는 동안엔 care.stage보다 한 단계 늦다)
        let house_state = HouseState {
            pet: own.shown.pet.clone(),
            stage: own.shown.stage,
            scale: own.shown.scale,
            hungry: care.is_hungry(),
            housed: own.pet.is_housed(),
        };
        if last_house.as_ref() != Some(&house_state) {
            *app.state::<AppState>().house.lock().unwrap() = house_state.clone();
            let _ = app.emit_to(house::LABEL, "house-state", house_state.clone());
            last_house = Some(house_state);
        }

        // 먹다가 눌린 동안(Pressed)도 먹는 중으로 본다 — 손을 떼면 계속 먹으므로 크기를 미리 바꾸지 않는다
        if pending_stage && !own.pet.is_eating() {
            pending_stage = false;
            apply_own_stage(&app, own, &care);
        }

        let hidden = crate::windows_hidden(&app);
        for s in &mut slots {
            s.place(hidden);
            if let Some(render) = s.take_render(&app) {
                let _ = app.emit_to(s.label.as_str(), "pet-render", render);
                if s.is_own() {
                    // 하우스 창 문간도 내 동물의 먹기·축하를 보여 준다
                    let _ = app.emit_to(house::LABEL, "pet-render", render);
                }
            }
        }

        thread::sleep(TICK);
    }
}
