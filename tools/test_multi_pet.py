"""🐾🐾🐾 펫 여러 마리 키우기 (2026-11-23 docs/superpowers/specs/2026-11-23-multi-pet.md).

최대 3마리, 같은 종류도 됨. 펫마다 따로 자라고 따로 놀러 감. 하우스·숨기기·집중은 모두 한 번에.
"집에 와"는 펫마다 + 모두. 소스를 읽어 메뉴 항목과 전체 적용 길이 있는지 봄 (창은 Windows·macOS에서만 실제로 확인됨).
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src-tauri" / "src"


def _text(name):
    return (SRC / name).read_text(encoding="utf-8")


def _fn(src, head):
    body = src[src.index(head):]
    return body[: body.index("\n}\n")] if "\n}\n" in body else body


def test_each_own_pet_is_a_slot_with_its_own_window():
    slots = _text("slots.rs")
    assert "Own(String)" in slots, "Role::Own carries the pet uid"
    assert "pub fn is_own_label(" in slots and 'const OWN_PREFIX: &str = "pet-";' in slots
    spawn = _fn(slots, "pub fn spawn_own(") + _fn(slots, "fn build_own_window(")
    # 새 펫 창도 다른 떠 있는 창처럼 overlay를 거쳐 만듦 (포커스를 뺏지 않고 작업 표시줄에 안 뜸)
    assert "crate::overlay::finish(" in spawn
    assert ".show()" not in spawn
    # 대표 펫을 보내 주면 다음 펫이 "pet" 창을 물려받음
    assert "pub fn swap_window(" in slots


def test_new_pet_windows_are_floating_pet_windows():
    overlay = _text("overlay.rs")
    assert "crate::slots::is_own_label(label)" in overlay
    main = _text("main.rs")
    assert "slots::is_own_label(label)" in main
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]
    for ident in ("events", "floating-windows", "pet-windows"):
        cap = next(c for c in caps if c["identifier"] == ident)
        assert "pet-*" in cap["windows"], ident


def test_menu_adds_lets_go_and_has_a_submenu_per_pet():
    tray = _text("tray.rs")
    assert '"🐾 펫 추가"' in tray
    assert '"보내 주기"' in tray and "보내 줄까요?" in tray
    # 펫마다 하위 메뉴 (고르고 닫히고 다시 여는 "고른 펫 ▸"은 없음)
    assert "pub fn sync_pets(" in tray and "fn build_pet_menu(" in tray and "고른 펫:" not in tray
    for input_ in ("Input::AddPet(", "Input::RemovePet", "Input::PickPet("):
        assert input_ in tray, input_
    loop = _text("game_loop.rs")
    assert "MAX_OWN_PETS" in loop
    # 마지막 한 마리는 못 뺌 (코어 Config::remove_pet이 거절)
    assert "config.remove_pet(" in loop


def test_come_home_is_per_pet_and_for_all():
    tray = _text("tray.rs")
    assert "모두 집에 와" in tray
    assert "COME_HOME_PREFIX" in tray
    net = _text("network.rs")
    assert "NetInput::ComeHome(" in net


def test_house_hide_and_focus_apply_to_every_pet():
    loop = _text("game_loop.rs")
    house = _fn(loop, "Input::ToggleHouse =>")
    assert "for i in 0..n" in house or "for s in slots[..n]" in house or "own_range" in house
    assert "presence.toggle(&app, config.focus_length, &mut slots[..n])" in loop
    tray = _text("tray.rs")
    toggle = tray[tray.index("pub fn show_all"):]
    toggle = toggle[: toggle.index("\n}\n", toggle.index("fn toggle_windows"))]
    assert "own_windows(app)" in toggle
    presence = _text("presence.rs")
    assert "tick_all(" in presence and "start_focus_all(" in presence


def test_feeding_a_bowl_calls_every_hungry_pet_and_the_hand_goes_to_the_nearest():
    loop = _text("game_loop.rs")
    assert "fn feed_targets(" in loop
    assert "is_hungry()" in _fn(loop, "fn feed_targets(")
    # 펫마다 밥그릇 하나 (창 라벨이 겹치지 않게 번호는 모두가 같이 씀)
    prop = _text("prop.rs")
    assert "static NEXT" in prop


def test_each_pet_visits_friends_on_its_own():
    net = _text("network.rs")
    assert "away: HashMap<String, AwayVisit>" in net
    assert "MULTI_PET_MIN_VERSION" in net
    assert "visit::may_join_friend(" in net and "visit::may_host_another(" in net
    # 친구 화면에서 돌아온 펫은 그 펫 창에 답장·물건을 받음 (늘 "pet" 창이 아님)
    leave = net[net.index("Visit::Leave { visit_id, next, side, vside, homesick_in, fling, note, boost, carry } =>"):]
    leave = leave[: leave.index("Visit::ComeHome { visit_id } =>")]
    assert "OWN_LABEL" not in leave


def test_old_config_moves_into_the_pet_list_at_start():
    main = _text("main.rs")
    assert "settle_pets(" in main
    loop = _text("game_loop.rs")
    assert "sync_legacy()" in loop


def test_help_and_readme_tell_about_many_pets():
    help_ = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    for text in (help_, readme):
        assert "펫 추가" in text and "모두 집에 와" in text


# ---- 코드 점검에서 찾은 것 (2026-11-23) ----


def test_a_pet_tied_to_a_vehicle_sleigh_or_object_is_not_let_go():
    """보내 준 펫이 탈것 좌석·썰매·든 물건을 잡고 있으면 그 창·물건이 주인을 잃음"""
    loop = _text("game_loop.rs")
    remove = loop[loop.index("Input::RemovePet(label) =>"):loop.index("Input::PickPet(label) =>")]
    for check in ("riding()", "boarding()", "carried_object()", "sleigh.is_riding("):
        assert check in remove, check


def test_control_follows_the_pet_that_inherits_the_lead_window():
    """첫 펫을 보내 주면 둘째가 "pet" 창을 물려받음 — 조종하던 둘째는 계속 조종되고, 조종하던 펫이 없어지면 두뇌도 조작 모드를 끔"""
    loop = _text("game_loop.rs")
    remove = loop[loop.index("Input::RemovePet(label) =>"):loop.index("Input::PickPet(label) =>")]
    assert "control_label = a[0].label.clone();" in remove
    lost = loop[loop.index("if control_on && control_i.is_none() {"):]
    lost = lost[: lost.index("control_on = false;")]
    assert "set_control(false)" in lost


def test_the_dodge_game_keeps_every_own_pet_on_screen():
    loop = _text("game_loop.rs")
    dodge = loop[loop.index("if dodge.is_open() {\n            // 🐾🐾🐾"):]
    dodge = dodge[: dodge.index("        }\n        //")]
    assert "slots[..n]" in dodge


def test_a_lifted_pet_is_not_carried_to_an_old_friend_that_already_hosts_one():
    net = _text("network.rs")
    lift = _fn(net, "    pub fn pair_lift(")
    assert "self.may_send_more(id, carried)" in lift


def test_the_pet_number_sent_to_a_friend_never_clashes_there():
    """펫을 보내 주면 차례가 바뀜 — 슬롯 번호를 Visit.pet으로 쓰면 이미 가 있는 펫과 겹쳐 친구가 돌려보냄. 친구마다 빈 번호를 씀"""
    net = _text("network.rs")
    assert "fn free_pet_no(" in net
    assert net.count("self.free_pet_no(") >= 2, "own_exited and forward_visit"
    loop = _text("game_loop.rs")
    assert "i as u8" not in loop


def test_a_friend_cannot_rotate_pet_numbers_past_the_cooldown():
    """보안 점검: Visit.pet을 바꿔 가며 보내면 번호마다 따로 세는 쉬는 시간을 건너뛰어 손님 창을 쏟아낼 수 있었음"""
    net = _text("network.rs")
    visit = net[net.index("fn on_visit_message("):]
    visit = visit[: visit.index("Visit::Leave { visit_id, next, side, vside, homesick_in, fling, note, boost, carry } =>")]
    assert visit.index("visit::sane_pet_no(pet)") < visit.index("cooldown_key")
    assert "visit::may_host_another(&here, pet_no)" in net


# ---- 독립 리뷰 2 (2026-11-23) ----


def test_sending_one_housed_pet_out_keeps_the_house_for_the_others():
    """한 마리만 하우스에서 꺼내 보내도 집 창을 숨겨 남은 펫들이 안 보이던 것: 집은 하우스 안 펫이 하나도 없을 때만 숨김"""
    loop = _text("game_loop.rs")
    leave = _fn(loop, "fn leave_house_now(")
    assert "hide_quietly" not in leave
    assert "fn hide_house_if_empty(" in loop
    house = loop[loop.index("Input::ToggleHouse =>"):loop.index("Input::ToggleControl(_) if dodge.is_open()")]
    assert "hide_house_if_empty(" in house
    send = loop[loop.index("input @ (Input::Net(NetInput::SendTo(_))"):loop.index("Input::Vehicle(input) =>")]
    assert "hide_house_if_empty(" in send


def test_a_window_handed_to_an_off_screen_pet_is_hidden():
    """첫 펫을 보내 주며 "pet" 창을 놀러 간·하우스 안 펫에게 넘기면 그 창이 화면에 남던 것"""
    slots = _text("slots.rs")
    swap = _fn(slots, "    pub fn swap_window(")
    assert "on_screen()" in swap and "hide_quietly(" in swap


def test_pet_menus_are_never_changed_under_a_lock_or_from_the_menu_click():
    """메뉴 바꾸기는 메인 스레드를 기다림 — 잠금을 쥔 채 바꾸면 메인 스레드 클릭 처리와 서로 기다릴 수 있음.
    손잡이는 잠금 안에서 꺼내고 (clone) 넣기만 하고, 모양이 그대로면 다시 만들지 않음 (열린 메뉴가 닫히지 않게)"""
    tray = _text("tray.rs")
    sync = _fn(tray, "pub fn sync_pets(")
    assert "let old = menu.pet_menus.lock().unwrap().clone();" in sync
    assert "*menu.pet_menus.lock().unwrap() = next;" in sync
    # 잠금은 꺼내고 넣고 (실패하면 비우기만) — 메뉴 바꾸기는 그 밖에서
    assert sync.count("pet_menus.lock()") == 3
    for line in sync.splitlines():
        if "pet_menus.lock()" in line:
            assert "menu.menu" not in line and "set_" not in line, line
    assert "m.shape == shape" in sync and "next.push(m.clone());" in sync
    handler = tray[tray.index(".on_menu_event(move |app, event|"):]
    handler = handler[: handler.index(".build(app)?;")]
    assert "sync_pets(" not in handler and "pick(app" not in handler
    # 누른 펫 항목은 그 펫을 고른 뒤 (PickPet) 할 일을 보냄 — 게임 루프가 차례로 처리
    send_to = _fn(tray, "fn send_to_pet(")
    assert send_to.index("Input::PickPet(") < send_to.index("send(app, input)")


def test_one_pet_has_no_extra_level_and_no_let_go():
    tray = _text("tray.rs")
    pet = _fn(tray, "fn build_pet_menu(")
    assert "let let_go = if shape.many {" in pet
    shape = _fn(tray, "fn pet_shape(")
    assert "many: count > 1" in shape


def test_control_moves_to_the_pet_whose_menu_was_used():
    loop = _text("game_loop.rs")
    arm = loop[loop.index("Input::ToggleControl(target) => {"):loop.index("Input::ToggleBroom =>")]
    assert "control_on && control_label != target" in arm
    assert "controlled: control == Some(s.label.as_str())," in loop


def test_a_new_pet_window_never_reuses_a_label_still_being_destroyed():
    slots = _text("slots.rs")
    free = _fn(slots, "pub fn free_own_label(")
    assert "get_webview_window(" in free


def test_a_pet_whose_window_could_not_be_made_is_retried():
    """시작할 때 창을 못 만든 펫이 설정에만 남아 (배만 고파지고) 뺄 수도 없던 것: 틱마다 가끔 다시 만듦"""
    loop = _text("game_loop.rs")
    assert "fn spawn_missing_pets(" in loop
    assert loop.count("spawn_missing_pets(") >= 3, "defined, at start, in the tick"


def test_letters_wait_while_any_pet_refuses_guests():
    mailbox = _text("mailbox.rs")
    assert "slots[0].pet.refuses_arrivals()" not in mailbox
    assert "slots::own_refuses(" in mailbox


def test_records_shared_by_my_other_computer_are_kept_as_old_records():
    loop = _text("game_loop.rs")
    assert "config.keep_old_records()" in loop


# ---- 독립 리뷰 3 (2026-11-26 메뉴 개편) ----


def test_a_failed_pet_submenu_rebuild_leaves_no_orphans():
    """펫 하위 메뉴를 다시 만들다 하나가 실패하면 이미 끼운 것·남은 옛것이 메뉴에 남아 다시는 지워지지 않던 것: 모두 빼고 다음에 처음부터"""
    tray = _text("tray.rs")
    sync = _fn(tray, "pub fn sync_pets(")
    fail = sync[sync.index("Err(e) =>"):]
    fail = fail[: fail.index("return;")]
    assert "for m in next.iter().chain(&old)" in fail and "menu.menu.remove(&m.sub)" in fail
    assert "menu.pet_menus.lock().unwrap().clear()" in fail


def test_pet_submenus_write_only_what_changed():
    """펫을 누를 때마다 모든 종류 체크를 펫마다 다시 쓰던 것 (메인 스레드 왕복 150번쯤): 지난번에 맞춘 모습과 다른 것만 씀.
    메뉴를 누른 뒤(체크가 스스로 뒤집힘)에는 force로 다 씀"""
    tray = _text("tray.rs")
    assert "pub applied: Option<Applied>," in tray
    assert "pub fn sync_pets(app: &AppHandle, force: bool)" in tray
    apply = _fn(tray, "fn sync_pet_menu(")
    assert "let was = m.applied.as_ref().filter(|_| !force);" in apply
    loop = _text("game_loop.rs")
    assert "crate::tray::sync_pets(&app, menu_dirty);" in loop
    down = loop[loop.index("Input::PointerDown { label } =>"):loop.index("Input::SetPet {")]
    assert "menu_dirty = true" not in down and "menu_soft = true" in down


def test_menu_actions_name_the_pet_and_do_nothing_if_it_is_gone():
    """PickPet이 사라진 펫 라벨이면 무시되어 다음 보내 주기·처음부터·종류 바꾸기가 그 전에 고른 펫에게 가던 것: 입력이 펫 라벨을 들고 감"""
    loop = _text("game_loop.rs")
    for head in ("Input::RemovePet(label) =>", "Input::ResetGrowth(label) =>", "Input::ToggleLock(label) =>", "Input::ToggleControl(target) =>"):
        assert head in loop, head
    assert "Input::SetPet { entry: next, remote, label } =>" in loop
    assert loop.count("let Some(i) = own_index(&slots, &label) else") >= 3 and "own_index(&slots, &target).is_none()" in loop
    tray = _text("tray.rs")
    for sent in ("Input::RemovePet(label.to_string())", "Input::ResetGrowth(label.to_string())", "Input::ToggleLock(label.to_string())", "Input::ToggleControl(label.to_string())"):
        assert sent in tray, sent
