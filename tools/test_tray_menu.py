"""트레이·우클릭 메뉴 배치 (사용자가 정한 2026-11-05 구성). 메뉴를 옮기면 도움말 경로도 같이 옮긴다."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")


def test_top_level_order():
    build = TRAY[TRAY.index("let menu = Menu::new(app)?;") :]
    build = build[: build.index("let pet_items")]
    items = re.findall(r"menu\.append\(&(?:PredefinedMenuItem::(separator)\(app\)\?|([a-z_]+))\)", build)
    names = [sep or name for sep, name in items]
    assert names == ["easter_egg", "separator", "my_pet", "feed", "house", "separator", "friends", "separator", "play", "focus", "separator", "settings", "toggle", "help", "quit"]


def test_submenus_hold_what_was_decided():
    assert 'Submenu::with_items(app, "🐾 내 펫", true, &[&growth, &timeline, &pets_menu, &style_menu, &lock, &reset])' in TRAY
    assert 'Submenu::with_items(app, "스타일", true, &[&style_round, &style_pixel])' in TRAY
    assert 'Submenu::with_items(app, "🎮 놀기", true, &[&control, &broom, &objects_menu, &dodge, &gift_book])' in TRAY
    assert 'Submenu::with_items(app, "⚙️ 설정", true, &[&weather, &poop, &cursor_play, &sizes])' in TRAY
    assert 'Submenu::with_items(app, "🎲 사다리타기", true, &[&new_game, &last, &invites])' in TRAY
    assert '"🍚 밥 주기"' in TRAY and '"👫 친구랑 놀기"' in TRAY and '"🏠 하우스"' in TRAY


def test_friends_menu_order():
    fill = TRAY[TRAY.index("fn try_fill_friends") :]
    fill = fill[: fill.index("/// 내 동물이 친구 화면에")]
    order = ["MY_NAME_ID", "인터넷 연결: 연결됨", "SHOW_CODE_ID", "ENTER_CODE_ID", '"놀러 보내기", online', "for f in &model.friends", "for (id, name) in &model.strangers", '"🎲 사다리타기"']
    positions = [fill.index(x) for x in order]
    assert positions == sorted(positions)


def test_relay_status_line_is_shown_in_the_friends_menu():
    # 연결됨·끊김·꺼짐 한 줄 (누를 수 없다). 상태가 바뀌면 메뉴를 다시 그리도록 모델에 들어 있어야 한다
    for line in ("인터넷 연결: 연결됨", "인터넷 연결: 끊김", "인터넷 연결: 꺼짐"):
        assert line in TRAY
    assert "pub relay: Option<bool>," in TRAY
    assert "self.relay_enabled.then_some(self.relay_connected)" in (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")


def test_help_item_label():
    assert 'MenuItem::with_id(app, "help", "도움말 ❓", true' in TRAY


def test_come_home_only_while_away_and_right_under_the_house():
    assert "menu.menu.insert(&menu.come_home, COME_HOME_POSITION)" in TRAY and "menu.menu.remove(&menu.come_home)" in TRAY
    assert "const COME_HOME_POSITION: usize = 5;" in TRAY  # danni, 구분선, 내 펫, 밥, 하우스 다음


def test_repair_is_gone():
    assert "다시 친구 맺기" not in TRAY and "다시 친구 맺기 필요" not in HELP and "forgot" not in TRAY


def test_help_uses_the_new_paths():
    for path in ["🐾 내 펫 ▸ 키우기 ▸", "🐾 내 펫 ▸ 스타일 ▸", "⚙️ 설정 ▸ 🌨️ 날씨 효과", "🎮 놀기 ▸ 🎁 선물 도감", "🎮 놀기 ▸ 🗑️ 쓰레기 꺼내기 ▸", "🎲 사다리타기 ▸ 새 사다리…", "🎲 사다리타기 ▸ 초대 받기", "🎲 사다리타기 ▸ 지난 결과"]:
        assert path in HELP, path
    assert "캐릭터 ▸" not in HELP and "내 친구 코드 보기" not in HELP and "사다리 초대 받기" not in HELP


def test_throwing_ten_is_one_menu_that_lists_the_friends():
    fill = TRAY[TRAY.index("fn try_fill_friends") :]
    assert 'Submenu::new(app, "🗑️ 쓰레기 10개 던지기", !targets.is_empty())' in fill
    assert "format!(\"{THROW_VOLLEY_PREFIX}{}\", f.target)" in fill
    # 친구별 하위 메뉴에서는 빠졌다
    assert "&[&send, &letter, &bring, &sep," in fill and "물건 10개 던지기" not in fill
    assert "ObjectInput::ThrowVolley(id.to_string())" in TRAY
    assert "🗑️ 쓰레기 10개 던지기" in HELP

def test_objects_appear_without_taking_focus():
    """tao는 숨긴 채 만든 창의 나중 show()를 활성화로 띄운다 (SW_SHOW): 친구가 던진 물건이 포커스를 뺏고 트레이 메뉴를 닫는다.
    만들면서 보이게 해야 tao가 활성화 없이 띄운다 (SW_SHOWNOACTIVATE)."""
    objects = (ROOT / "src-tauri" / "src" / "objects.rs").read_text(encoding="utf-8")
    overlay = (ROOT / "src-tauri" / "src" / "overlay.rs").read_text(encoding="utf-8")
    quiet = overlay[overlay.index("pub fn finish_quiet") :]
    quiet = quiet[: quiet.index("\n}\n")]
    assert ".visible(true)" in quiet and ".focused(false)" in quiet
    assert "overlay::finish_quiet(" in objects and "overlay::finish(" not in objects
    assert "window.show()" not in objects
    # tao의 set_position·set_size는 보이는 창을 다시 활성화한다
    assert ".set_position(" not in objects and ".set_size(" not in objects


def test_size_window_slides_live_and_resets_easily():
    assert '"📏 크기 조절…"' in TRAY and "SIZES_ID => send(app, Input::ShowSizes)" in TRAY
    assert "⚙️ 설정 ▸ 📏 크기 조절…" in HELP
    html = (ROOT / "ui" / "size.html").read_text(encoding="utf-8")
    js = (ROOT / "ui" / "size.js").read_text(encoding="utf-8")
    for el in ('id="pet"', 'id="object"', 'id="pet-reset"', 'id="object-reset"', 'id="reset"', "모두 기본값으로"):
        assert el in html, el
    # 움직이는 동안은 보여 주기만, 놓거나 기본값을 누르면 저장
    assert 'addEventListener("input", () => send(false))' in js and 'addEventListener("change", () => send(true))' in js
    assert "send(true)" in js[js.index("function resetTo") :]
    # 슬라이더를 끄는 동안 펫 창이 포커스를 뺏으면 안 된다
    slots = (ROOT / "src-tauri" / "src" / "slots.rs").read_text(encoding="utf-8")
    assert ".set_size(" not in slots.replace("slot.set_size(", "")
    # 친구에게는 내 슬라이더가 안 들어간 크기를 보낸다
    assert "scale: own.shown.base," in (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")


def test_friend_submenu_links_my_other_computer_with_a_code():
    fill = TRAY[TRAY.index("fn try_fill_friends") :]
    fill = fill[: fill.index("/// 내 동물이 친구 화면에")]
    # 친구 하위 메뉴: 연결 안 됐으면 코드 창을 여는 항목, 연결된 그 친구면 연결 끊기 (이름 바꾸기 위)
    assert '"📱 내 다른 컴퓨터로 연결…"' in fill and '"📱 연결 끊기 (내 컴퓨터)"' in fill
    assert 'format!("{UNLINK_PREFIX}{}", f.id)' in fill and 'format!("{DEVICE_PREFIX}{}", f.id)' in fill
    assert "&sep2, &device, &nickname, &unfriend]" in fill
    assert "CheckMenuItem::with_id(app, format!(\"{DEVICE_PREFIX}" not in fill, "no plain checkbox any more"
    # 친구 목록 이름표
    assert '"(내 컴퓨터)"' in fill
    assert 'const DEVICE_PREFIX: &str = "net:device:";' in TRAY and 'const UNLINK_PREFIX: &str = "net:unlink:";' in TRAY
    assert "net(NetInput::LinkDevice(id.to_string()))" in TRAY and "net(NetInput::UnlinkDevice(id.to_string()))" in TRAY
    network = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
    for arm in ("NetInput::LinkDevice(id) =>", "NetInput::UnlinkDevice(id) =>", "NetInput::SubmitLinkCode(code) =>", "NetEvent::DeviceLinked { id } =>", "NetEvent::DeviceLinkFailed {", "NetEvent::DeviceProven { id } =>", "NetEvent::DeviceUnlinked { id } =>"):
        assert arm in network, arm
    assert "DEVICE_ON_BUBBLE" not in network
    # 📱 코드가 맞으면 양쪽에 "본인의 PC가 맞습니까?" — 확인/취소, 창 닫기(X·Esc·닫기)도 취소
    for arm in ("NetEvent::DeviceConfirm { id } =>", "NetEvent::DeviceLinkCancelled { id } =>", "NetInput::ConfirmLink(yes) =>", "NetInput::LinkClose =>"):
        assert arm in network, arm
    assert "net.confirm_device(" in network
    assert '"저쪽 컴퓨터에서도 확인을 눌러 주세요"' in network and '"연결을 취소했어요"' in network
    build_link = network[network.index("fn build_link_window") :]
    build_link = build_link[: build_link.index("\n}\n")]
    assert "CloseRequested" in build_link and "NetInput::LinkClose" in build_link
    link_close = network[network.index("pub fn link_close") :]
    link_close = link_close[: link_close.index("\n}\n")]
    assert "NetInput::LinkClose" in link_close
    # 비밀은 설정에만 (로그에 찍지 않는다)
    assert "set_friend_device_secret(" in network and "linked_device()" in network
    for line in network.splitlines():
        if "eprintln!" in line:
            assert "secret" not in line, line
    assert "📱 내 다른 컴퓨터" in HELP and "코드" in HELP[HELP.index("📱 내 다른 컴퓨터") :][:600]


NETWORK = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")


def _fill():
    fill = TRAY[TRAY.index("fn try_fill_friends") :]
    return fill[: fill.index("/// 내 동물이 친구 화면에")]


def test_a_friend_and_their_other_computer_are_one_row_that_acts_on_the_computer_they_play_on():
    fill = _fill()
    # 사람마다 한 줄: 친구가 소개한 그 친구의 다른 컴퓨터는 따로 보이지 않는다
    assert '"(다른 컴퓨터)"' not in fill
    assert "config.people(" in NETWORK[NETWORK.index("fn menu_model") :]
    assert "pub target: String," in TRAY
    # 보내기·편지·데리고 오기·던지기는 지금 노는 컴퓨터로, 화면 끝·이름·연결 끊기는 그 사람(처음 친구)으로
    for prefix in ("SEND_LIST_PREFIX", "THROW_VOLLEY_PREFIX", "SEND_PREFIX", "LETTER_PREFIX", "BRING_PREFIX"):
        assert 'format!("{%s}{}", f.target)' % prefix in fill, prefix
    for prefix in ("NICKNAME_PREFIX", "UNFRIEND_PREFIX", "UNLINK_PREFIX", "DEVICE_PREFIX"):
        assert 'format!("{%s}{}", f.id)' % prefix in fill, prefix
    assert 'format!("{SIDE_PREFIX}{key}:{}", f.id)' in fill
    # 이름표·순위·사다리·타임라인도 그 사람의 이름
    assert "config.person_name(from)" in NETWORK and "config.person_name(&g.owner_id)" in NETWORK
    assert "config.dodge_friends()" in (ROOT / "src-tauri" / "src" / "dodge.rs").read_text(encoding="utf-8")
    assert "person_name(" in (ROOT / "src-tauri" / "src" / "ladder.rs").read_text(encoding="utf-8")
    assert "person_name(" in (ROOT / "src-tauri" / "src" / "timeline.rs").read_text(encoding="utf-8")


def test_the_computer_on_standby_says_so_and_can_take_over():
    fill = _fill()
    assert '"📱 지금은 다른 컴퓨터에서 놀고 있어요"' in fill and '"📱 이 컴퓨터로 놀기"' in fill
    assert fill.index("MY_NAME_ID") < fill.index("📱 지금은 다른 컴퓨터에서 놀고 있어요") < fill.index("SHOW_CODE_ID")
    assert "PLAY_HERE_ID => net(NetInput::PlayHere)" in TRAY or "other == PLAY_HERE_ID" in TRAY
    assert "NetInput::PlayHere =>" in NETWORK and "net.claim_active()" in NETWORK
    # 저절로 넘어오기: 대기 중에 다시 쓰기 시작하면 (core의 ClaimWatch), 쓴 지 얼마나 됐는지는 net에도
    assert "ClaimWatch" in NETWORK and "net.set_idle(" in NETWORK
    # 대기가 되면 친구에게 가 있는 내 펫만 부른다 (내 다른 컴퓨터에 가 있으면 그대로)
    standby = NETWORK[NETWORK.index("NetEvent::Standby { me, device } =>") :]
    standby = standby[: standby.index("NetEvent::RelayStatus")]
    assert "call_home_from_friends" in standby
    call = NETWORK[NETWORK.index("fn call_home_from_friends") :]
    call = call[: call.index("\n    }\n")]
    assert "is_my_device(" in call and "Visit::ComeHome" in call


def test_my_other_computers_pet_can_be_brought_over_but_my_pet_does_not_go_to_it_on_standby():
    fill = _fill()
    assert '"📱 다른 컴퓨터 펫 데리고 오기"' in fill
    assert "BRING_DEVICE_ID" in fill and "other == BRING_DEVICE_ID" in TRAY
    assert "NetInput::BringDevicePet =>" in NETWORK
    # 대기 중인 내 다른 컴퓨터는 내 펫·물건이 갈 곳이 아니다 (끝이 닫히고, 메뉴 보내기·던지기에서 빠진다)
    is_connected = NETWORK[NETWORK.index("pub fn is_connected") :]
    is_connected = is_connected[: is_connected.index("\n    }\n")]
    assert "linked::playable(" in is_connected
    objects = (ROOT / "src-tauri" / "src" / "objects.rs").read_text(encoding="utf-8")
    assert "network.is_connected(id)" in objects
    assert "self.connected.contains(" not in NETWORK[NETWORK.index("fn menu_model") :].split("can_send")[1][:200]


def test_my_other_computers_visiting_pet_can_go_on_to_a_friend_as_me():
    # 이 컴퓨터가 대신 데려다준다 (대기 중인 컴퓨터는 친구와 연결이 없다) — 받는 쪽은 한 사람으로 본다
    assert "fn carry_for_device" in NETWORK and "of: Some(" in NETWORK
    assert "config.same_person(" in NETWORK
    assert "config.reach(" in NETWORK
    assert "carried:" in NETWORK
    carry = NETWORK[NETWORK.index("fn carry_for_device") :]
    carry = carry[: carry.index("\n    }\n")]
    assert "v >= DEVICES_MIN_VERSION" in carry, "never to an older build"


def test_help_explains_one_computer_at_a_time():
    assert "📱 이 컴퓨터로 놀기" in HELP and "📱 다른 컴퓨터 펫 데리고 오기" in HELP


GAME_LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")


def test_my_two_computers_share_the_pet_and_its_growth():
    # 노는 컴퓨터가 고른 펫과 모든 펫의 돌보기 상태를 보낸다 (net이 모아서, 노는 동안만)
    share = NETWORK[NETWORK.index("pub fn share_state") :]
    share = share[: share.index("\n    }\n")]
    assert "linked::shared_care(" in share and "net.share_state(" in share
    # 쉬는 컴퓨터에서 밥을 주거나 펫을 바꾸면 그 컴퓨터가 논다 (넘겨받기가 그걸 덮지 않게), 그냥 새로 고침은 보내지 않는다
    assert "claim_active()" in share and "self.standby" in share
    # 받은 쪽: core가 아는 동물만 다듬어 합치고, 화면의 펫·단계·배고픔을 바꾼다
    arm = GAME_LOOP[GAME_LOOP.index("NetEvent::DeviceState {") :]
    arm = arm[: arm.index("Input::Net(input) =>")]
    assert "linked::adopt_shared(" in arm and "apply_own_stage(" in arm and "set_hungry(" in arm
    assert "Input::SetPet { entry, remote: true }" in arm, "the pet switches through the usual path"
    assert "share_state(" not in arm, "what came from the other computer is not sent back"
    set_pet = GAME_LOOP[GAME_LOOP.index("Input::SetPet { entry: next, remote } =>") :]
    set_pet = set_pet[: set_pet.index("Input::SetStyle(")]
    assert "sync_pet_checks(" in set_pet and "if !remote" in set_pet
    # 먹이기·성장 멈추기·처음부터·펫 바꾸기·점심 배고픔·주기적 저장 때 알린다
    assert GAME_LOOP.count("share_care(") >= 7


def test_help_says_what_my_two_computers_share():
    one = HELP[HELP.index("📱 한 번에 한 컴퓨터") :]
    one = one[: one.index("</li>")]
    assert "펫·성장·선물·기록은 컴마다 따로" not in one
    assert "고른 펫과 성장" in one and "그사이 다른 컴에서 키운 건 덮어써질 수 있어요" in one
    assert "선물 도감" in one and "따로" in one
