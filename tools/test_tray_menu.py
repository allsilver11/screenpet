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
    assert 'Submenu::new(app, "🗑️ 쓰레기 10개 던지기", !online.is_empty())' in fill
    assert "format!(\"{THROW_VOLLEY_PREFIX}{}\", f.id)" in fill
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
    assert '"(내 컴퓨터)"' in fill and '"(다른 컴퓨터)"' in fill
    assert 'const DEVICE_PREFIX: &str = "net:device:";' in TRAY and 'const UNLINK_PREFIX: &str = "net:unlink:";' in TRAY
    assert "net(NetInput::LinkDevice(id.to_string()))" in TRAY and "net(NetInput::UnlinkDevice(id.to_string()))" in TRAY
    network = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
    for arm in ("NetInput::LinkDevice(id) =>", "NetInput::UnlinkDevice(id) =>", "NetInput::SubmitLinkCode(code) =>", "NetEvent::DeviceLinked { id } =>", "NetEvent::DeviceLinkFailed {", "NetEvent::DeviceProven { id } =>", "NetEvent::DeviceUnlinked { id } =>"):
        assert arm in network, arm
    assert "DEVICE_ON_BUBBLE" not in network
    # 비밀은 설정에만 (로그에 찍지 않는다)
    assert "set_friend_device_secret(" in network and "linked_device()" in network
    for line in network.splitlines():
        if "eprintln!" in line:
            assert "secret" not in line, line
    assert "📱 내 다른 컴퓨터" in HELP and "코드" in HELP[HELP.index("📱 내 다른 컴퓨터") :][:600]
