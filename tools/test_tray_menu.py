"""트레이·우클릭 메뉴 배치 (사용자가 정한 2026-11-05 구성, 2026-11-26 개편: 펫마다 하위 메뉴, 깊이 둘까지). 메뉴를 옮기면 도움말 경로도 같이 옮긴다."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")


def test_top_level_order():
    build = TRAY[TRAY.index("let menu = Menu::new(app)?;") :]
    build = build[: build.index("TrayIconBuilder::with_id")]
    items = re.findall(r"menu\.append\(&(?:PredefinedMenuItem::(separator)\(app\)\?|([a-z_]+))\)", build)
    names = [sep or name for sep, name in items]
    # 펫 하위 메뉴는 게임 루프가 danni·구분선 다음(PETS_POSITION)에 펫마다 끼움 — 그 아래 펫 추가·밥·하우스
    # 나무는 docs/superpowers/specs/2026-11-26-tray-menu.md — 🏡 집에 와(놀러 갔을 때)는 하우스 아래, 🎮 조작은 숨기기 위에 게임 루프가 끼움
    assert names == ["easter_egg", "separator", "add_menu", "separator", "feed", "house", "toggle", "focus", "separator", "play", "separator", "settings", "info", "separator", "refresh", "restart", "quit"]
    assert "const PETS_POSITION: usize = 2;" in TRAY and "menu.menu.insert(&m.sub, PETS_POSITION + i)" in TRAY
    # 📮 건의함은 누구나, 건의하기 바로 밑 (개발자 컴퓨터에서는 개발자 건의함을 연다) — ℹ️ 도움말·건의 ▸ 안
    assert 'Submenu::with_items(app, "ℹ️ 도움말·건의", true, &[&help, &whats_new, &feedback, &board])' in TRAY
    assert "menu.append(&play)?;" in build and '"👫 친구랑 놀기"' not in TRAY, "one play menu holds solo and friend play"


def test_submenus_hold_what_was_decided():
    # 🐾 펫마다: 성장 단계 · 종류 ▸ · (🎨 흐물이 색 ▸) · 🎮 조작 · 성장 멈추기 · 처음부터 다시 키우기 ▸ · (🏡 집에 와) · 놀러 보내기 ▸ · (보내 주기 ▸)
    pet = TRAY[TRAY.index("fn build_pet_menu(") :]
    pet = pet[: pet.index("\n}\n")]
    order = ["sub.append(&growth)?", "sub.append(&kind)?", '"🎨 흐물이 색"', "sub.append(&lock)?", "sub.append(&reset)?", '"🏡 집에 와"', "sub.append(&send)?", '"보내 주기"']
    positions = [pet.index(x) for x in order]
    assert positions == sorted(positions)
    assert "if shape.tintable {" in pet and "if shape.away {" in pet and "if shape.many {" in pet
    # 펫 고르기(고른 펫 ▸)는 없어짐: 누른 항목이 그 펫 라벨을 들고 감
    assert "PICK_PREFIX" not in TRAY and "고른 펫:" not in TRAY and "pick_menu" not in TRAY
    # 🟦 스타일(동글/픽셀)과 🎁 선물 도감은 v0.7.0에 빠짐
    assert '"스타일"' not in TRAY and "style_pixel" not in TRAY and "gift" not in TRAY.lower() and "선물" not in TRAY
    assert 'Submenu::with_items(app, "🎮 놀기", true, &[&broom, &objects_menu, &vehicles_menu, &dodge, &timeline])' in TRAY
    assert "play.append(&PredefinedMenuItem::separator(app)?)?;" in TRAY and "const PLAY_FIXED: usize = 5;" in TRAY
    assert 'Submenu::with_items(app, "⚙️ 설정", true, &[&weather, &weather_kind, &places_menu, &s1, &poop, &cursor_play, &typing_cheer, &lumos_light, &s2, &sizes, &opacity, &s3, &autostart, &pet_stats])' in TRAY
    assert "const TYPING_PERMISSION_POSITION: usize = 7;" in TRAY
    assert 'Submenu::with_items(app, "🎲 사다리타기", true, &[&new_game, &last, &invites])' in TRAY
    assert '"🍚 밥 주기"' in TRAY and '"👫 친구"' in TRAY and '"🏠 하우스"' in TRAY and '"🍅 뽀모도로"' in TRAY and '"🚪 종료"' in TRAY
    assert "집중 친구" not in TRAY


def test_friends_menu_order():
    fill = TRAY[TRAY.index("fn try_fill_friends") :]
    fill = fill[: fill.index("/// 하우스 항목 글자")]
    # 🎮 놀기 ▸ 의 혼자 놀기 아래: 👫 친구 · 친구마다 ▸ · 친구 신청 · 🗑️ 던지기 ▸ · 🎲 사다리타기 ▸ · 👫 내 이름·친구 코드 ▸
    assert "while submenu.remove_at(PLAY_FIXED)?.is_some() {}" in fill
    order = ['"👫 친구"', "for f in &model.friends", "for (id, name) in &model.strangers", '"🗑️ 쓰레기 10개 던지기"', '"🎲 사다리타기"', '"👫 내 이름·친구 코드"', "MY_NAME_ID", "인터넷 연결: 연결됨", "SHOW_CODE_ID", "ENTER_CODE_ID"]
    positions = [fill.index(x) for x in order]
    assert positions == sorted(positions)
    # 놀러 보내기 ▸ 친구 목록은 펫 하위 메뉴로 옮김 (친구 ▸ 놀러 보내기 ▸ 펫 처럼 셋째 단계도 없음 — 펫마다 "<펫> 보내기")
    assert 'Submenu::new(app, "놀러 보내기", online' not in fill and "pet_choices" not in TRAY and "SEND_LIST_PREFIX" not in TRAY
    assert 'format!("{} 보내기", menu_text(name))' in fill


def test_relay_status_line_is_shown_in_the_friends_menu():
    # 연결됨·끊김·꺼짐 한 줄 (누를 수 없다). 상태가 바뀌면 메뉴를 다시 그리도록 모델에 들어 있어야 한다
    for line in ("인터넷 연결: 연결됨", "인터넷 연결: 끊김", "인터넷 연결: 꺼짐"):
        assert line in TRAY
    assert "pub relay: Option<bool>," in TRAY
    # 잠깐 끊긴 건 30초 동안 "연결됨" 그대로 (회사 망처럼 끊겼다 붙었다 하면 줄이 깜빡이고 열린 메뉴가 닫혔다 — screenpet_core::flicker)
    net = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
    assert "self.relay_enabled.then_some(self.flicker.shown(RELAY_FLICKER, self.relay_connected, now))" in net


def test_help_item_label():
    assert 'MenuItem::with_id(app, "help", "❓ 도움말", true' in TRAY


def test_come_home_only_while_away_and_right_under_the_house():
    sync = TRAY[TRAY.index("fn sync_come_home(") :]
    sync = sync[: sync.index("\n}\n")]
    assert "menu.menu.insert(&menu.come_home, at + 1)" in sync and "menu.menu.remove(&menu.come_home)" in sync
    assert "item.id() == HOUSE_ID" in sync
    # "모두 집에 와"는 둘 이상 가 있을 때만 (한 마리씩은 펫 하위 메뉴의 🏡 집에 와)
    label = TRAY[TRAY.index("pub fn come_home_label(") :]
    assert "away >= 2" in label[: label.index("\n}\n")] and '"🏡 모두 집에 와"' in label


def test_repair_is_gone():
    assert "다시 친구 맺기" not in TRAY and "다시 친구 맺기 필요" not in HELP and "forgot" not in TRAY


def test_help_uses_the_new_paths():
    for path in ["▸ 종류 ▸", "⚙️ 설정 ▸ 🌨️ 날씨 효과", "⚙️ 설정 ▸ 🌦️ 날씨 종류 ▸", "⚙️ 설정 ▸ 📍 날씨 지역 ▸", "🔄 새로고침", "♻️ 재시작", "🎮 놀기 ▸ 🗑️ 쓰레기 꺼내기 ▸", "🎲 사다리타기 ▸ 새 사다리…", "🎲 사다리타기 ▸ 초대 받기", "🎲 사다리타기 ▸ 지난 결과"]:
        assert path in HELP, path
    assert "고른 펫 ▸" not in HELP and "🐾 내 펫 ▸" not in HELP and "🌦️ 날씨 종류 ▸ 📍" not in HELP
    assert "캐릭터 ▸" not in HELP and "내 친구 코드 보기" not in HELP and "사다리 초대 받기" not in HELP
    assert "스타일 ▸" not in HELP and "선물" not in HELP and "픽셀" not in HELP and "유령" not in HELP


def test_throwing_ten_is_one_menu_that_lists_the_friends():
    fill = TRAY[TRAY.index("fn try_fill_friends") :]
    assert 'Submenu::new(app, "🗑️ 쓰레기 10개 던지기", !targets.is_empty())' in fill
    assert "format!(\"{THROW_VOLLEY_PREFIX}{}\", f.target)" in fill
    # 친구별 하위 메뉴에서는 빠졌다
    assert "friend.append_items(&[&letter, &bring, &sep," in fill and "물건 10개 던지기" not in fill
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
    fill = fill[: fill.index("/// 하우스 항목 글자")]
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
        if "eprintln!" in line or "log!(" in line:
            assert "secret" not in line, line
    assert "📱 내 다른 컴퓨터" in HELP and "코드" in HELP[HELP.index("📱 내 다른 컴퓨터") :][:600]


NETWORK = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")


def _fill():
    fill = TRAY[TRAY.index("fn try_fill_friends") :]
    return fill[: fill.index("/// 하우스 항목 글자")]


def test_a_friend_and_their_other_computer_are_one_row_that_acts_on_the_computer_they_play_on():
    fill = _fill()
    # 사람마다 한 줄: 친구가 소개한 그 친구의 다른 컴퓨터는 따로 보이지 않는다
    assert '"(다른 컴퓨터)"' not in fill
    assert "config.people(" in NETWORK[NETWORK.index("fn menu_model") :]
    assert "pub target: String," in TRAY
    # 보내기·편지·데리고 오기·던지기는 지금 노는 컴퓨터로, 화면 끝·이름·연결 끊기는 그 사람(처음 친구)으로
    assert 'format!("{SEND_PET_PREFIX}{uid}|{}", f.target)' in fill
    for prefix in ("THROW_VOLLEY_PREFIX", "SEND_PREFIX", "LETTER_PREFIX", "BRING_PREFIX"):
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
    assert fill.index('"👫 친구"') < fill.index("📱 지금은 다른 컴퓨터에서 놀고 있어요") < fill.index("for f in &model.friends")
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
    assert "Input::SetPet { entry, remote: true, label: None }" in arm, "the pet switches through the usual path"
    assert "share_state(" not in arm, "what came from the other computer is not sent back"
    set_pet = GAME_LOOP[GAME_LOOP.index("Input::SetPet { entry: next, remote, label } =>") :]
    set_pet = set_pet[: set_pet.index("Input::SetTint(")]
    assert "menu_dirty = true;" in set_pet and "if !remote" in set_pet
    # 먹이기·성장 멈추기·처음부터·펫 바꾸기·점심 배고픔·주기적 저장 때 알린다
    assert GAME_LOOP.count("share_care(") >= 7


def test_help_says_what_my_two_computers_share():
    one = HELP[HELP.index("📱 한 번에 한 컴퓨터") :]
    one = one[: one.index("</li>")]
    assert "펫·성장·선물·기록은 컴마다 따로" not in one
    assert "고른 펫과 성장" in one and "그사이 다른 컴에서 키운 건 덮어써질 수 있어요" in one
    assert "똥피하기 기록" in one and "따로" in one and "선물 도감" not in one and "스타일" not in one


def test_a_short_drop_does_not_flip_friends_offline_or_redraw_the_menu():
    """친구·릴레이가 잠깐 끊겼다 붙으면 메뉴 모델이 바뀌지 않는다 (OFFLINE_GRACE_SECS 유예), 유예가 끝나면 그때 한 번 다시 그린다."""
    net = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
    assert "self.flicker.down(&id, now);" in net and "self.flicker.up(&id);" in net
    assert "for id in self.flicker.expired(secs) {" in net
    assert 'log!(Info, "net", "friend_down", "friend" => short_id(&id));' in net


def test_heumul_color_menu_sits_in_each_tintable_pets_submenu():
    """🎨 흐물이 색 ▸ (기본·흰색·…·🌈 무지개, 지금 색에 체크): 잉크 선 동물(pet.json tint) 펫의 하위 메뉴, 종류 ▸ 바로 아래. 색은 하나라 모든 흐물이 펫 메뉴의 체크가 같이 바뀐다."""
    pet = TRAY[TRAY.index("fn build_pet_menu(") :]
    pet = pet[: pet.index("\n}\n")]
    assert 'Submenu::new(app, "🎨 흐물이 색", true)' in pet and "if shape.tintable {" in pet
    assert pet.index("sub.append(&kind)?") < pet.index('"🎨 흐물이 색"') < pet.index("sub.append(&lock)?")
    assert 'const TINT_PREFIX: &str = "tint:";' in TRAY
    assert "for t in BodyTint::ALL" in pet and "t.label()" in pet and 'format!("{TINT_PREFIX}{label}|{}", t.id())' in pet
    assert ".strip_prefix(TINT_PREFIX).and_then(|r| r.split_once('|')).and_then(|(_, t)| BodyTint::from_id(t))" in TRAY and "Input::SetTint(tint)" in TRAY
    assert "p.manifest.tint" in TRAY[TRAY.index("fn pet_shape(") :]
    sync = TRAY[TRAY.index("pub fn sync_tint(") :]
    sync = sync[: sync.index("\n}\n")]
    assert "flat_map(|m| &m.tints)" in sync
    game = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    assert "crate::tray::sync_pets(&app, menu_dirty);" in game
    assert "Input::SetTint(tint) => {" in game and "own.set_tint(&app, tint);" in game and "config.tint = tint;" in game
    assert "🎨 흐물이 색 ▸" in HELP


def test_weather_kind_menu_picks_real_or_season_weather_and_the_city():
    """☀️ 실제 날씨 (용인) / 📅 계절 날씨 체크 둘과 📍 날씨 지역 ▸ (도시마다 체크) — ⚙️ 설정 ▸ 🌨️ 날씨 효과 바로 아래."""
    assert 'Submenu::with_items(app, "🌦️ 날씨 종류", true, &[&weather_real, &weather_season, &PredefinedMenuItem::separator(app)?])' in TRAY
    assert "weather_kind.append(&places_menu)?;" not in TRAY  # 날씨 지역 ▸ 은 ⚙️ 설정 바로 아래 (깊이 둘까지)
    assert 'Submenu::new(app, "📍 날씨 지역", true)?' in TRAY
    assert "for place in Place::ALL" in TRAY and 'format!("{WEATHER_PLACE_PREFIX}{}", place.id())' in TRAY
    assert 'const WEATHER_PLACE_PREFIX: &str = "weather-place:";' in TRAY
    assert ".strip_prefix(WEATHER_PLACE_PREFIX).and_then(Place::from_id)" in TRAY and "Input::SetWeatherPlace(place)" in TRAY
    assert "WEATHER_REAL_ID => send(app, Input::SetWeatherSource(WeatherSource::Real))" in TRAY
    assert "WEATHER_SEASON_ID => send(app, Input::SetWeatherSource(WeatherSource::Season))" in TRAY
    # 실제 날씨 글자에 고른 도시가 보인다
    assert 'format!("☀️ 실제 날씨 ({})", place.label())' in TRAY
    game = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    assert "Input::SetWeatherSource(source) => {" in game and "config.weather_source = source;" in game and "weather.set_source(source);" in game
    assert "Input::SetWeatherPlace(place) => {" in game and "config.weather_place = place;" in game and "network.want_weather(place);" in game
    # 체크 항목은 누를 때 스스로 뒤집히므로 늘 다시 맞춘다 (시작할 때도)
    assert game.count("crate::tray::sync_weather_kind(") >= 3


def test_weather_kind_menu_lets_you_pick_the_weather_yourself():
    """❄️ 눈 / 🌧️ 비 / 🍂 낙엽 / 🌸 꽃잎 / 🌿 푸른 잎 — 실제·계절 다음에 체크 항목으로, 고르면 설정에 남고 날씨 효과도 켬."""
    build = TRAY[TRAY.index('"🌦️ 날씨 종류"'):TRAY.index("let broom = CheckMenuItem")]
    assert "for source in WeatherSource::MANUAL" in build
    assert 'format!("{WEATHER_SRC_PREFIX}{}", source.id())' in build and "source.label()" in build
    assert "weather_kind.append(&item)?;" in build and "weather_manual.push((item, source));" in build
    assert 'const WEATHER_SRC_PREFIX: &str = "weather-src:";' in TRAY
    assert ".strip_prefix(WEATHER_SRC_PREFIX).and_then(WeatherSource::from_id)" in TRAY
    # 실제·계절은 정확한 id가 먼저 잡힘 (prefix 분기는 그 밖의 것만)
    assert TRAY.index("WEATHER_REAL_ID => send(") < TRAY.index(".strip_prefix(WEATHER_SRC_PREFIX)")
    sync = TRAY[TRAY.index("pub fn sync_weather_kind("):]
    sync = sync[: sync.index("\n}\n")]
    assert "for (item, s) in &menu.weather_manual" in sync and "item.set_checked(*s == source)" in sync
    assert "weather_manual," in TRAY[TRAY.index("Ok(TrayMenu {"):]
    game = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    arm = game[game.index("Input::SetWeatherSource(source) => {"):game.index("Input::SetWeatherPlace(place) => {")]
    assert "source.manual().is_some() && !config.weather_on" in arm
    assert "config.weather_on = true;" in arm and "weather.sync_menu(&app, false);" in arm
    # 날씨 효과 체크는 일 년 내내 누를 수 있음
    assert "set_enabled" not in TRAY[TRAY.index("let weather = CheckMenuItem"):TRAY.index("let weather_kind")]


def test_refresh_and_restart_sit_right_above_quit():
    """🔄 새로고침 (떠 있는 창을 모두 다시 읽고 다시 보임 — 돌던 것·연결은 그대로)과 ♻️ 재시작 (저장·정리 후 다시 띄움)이 종료 바로 위."""
    assert "menu.append(&refresh)?;\n    menu.append(&restart)?;\n    menu.append(&quit)?;" in TRAY and '"🚪 종료"' in TRAY
    assert '"🔄 새로고침"' in TRAY and '"♻️ 재시작"' in TRAY
    assert "REFRESH_ID => send(app, Input::Refresh)," in TRAY
    restart = TRAY[TRAY.index("RESTART_ID => {") :]
    restart = restart[: restart.index("}\n            }")]
    assert "Input::Restart" in restart and "app.request_restart()" in restart
    game = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    arm = game[game.index("Input::Restart => {") :]
    arm = arm[: arm.index("Input::Refresh => {")]
    # 종료처럼 저장하고 🌗 테마를 되돌린 뒤, Tauri가 정리(single-instance 잠금 풀기)를 마치고 새 앱을 띄움
    assert "save_config(&app, &mut config);" in arm and "l.quit(LUMOS_QUIT_WAIT)" in arm and "app.request_restart();" in arm
    assert "app.exit(" not in arm and "std::process::exit" not in game
    refresh = game[game.index("Input::Refresh => {") :]
    refresh = refresh[: refresh.index("Input::SaveAndStop(done)")]
    assert "refresh_windows(" in refresh and "Input::PageReady(label) =>" in game and "resend_to(" in game
    assert "🔄 새로고침" in HELP and "♻️ 재시작" in HELP


def test_the_update_item_no_longer_shares_the_refresh_icon():
    assert '"🔄 업데이트' not in TRAY and '"⬆️ 업데이트"' in TRAY


def test_menu_clicks_only_send_inputs():
    """메인 스레드의 메뉴 처리기는 입력만 보냄: 메뉴를 바꾸면 메인 스레드가 스스로를 기다려 멈출 수 있음 (게임 루프가 sync_pets로 맞춤)."""
    handler = TRAY[TRAY.index(".on_menu_event(move |app, event|") :]
    handler = handler[: handler.index(".build(app)?;")]
    for op in ("set_checked(", "set_text(", "set_enabled(", "sync_pets(", "insert(", "remove("):
        assert op not in handler, op
    assert "fn send_to_pet(" in TRAY
    for arm in ("KIND_PREFIX", "CONTROL_PREFIX", "LOCK_PREFIX", "RESET_PREFIX", "LET_GO_PREFIX"):
        assert "other.strip_prefix(%s)" % arm in handler, arm


def test_control_is_at_the_top_and_picks_the_pet_when_there_are_many():
    """🎮 조작: 한 마리면 맨 위 체크 하나, 여럿이면 🎮 조작 ▸ 펫마다 체크 (다른 펫을 고르면 그 펫으로 넘어감) — 🙈 숨기기 바로 위."""
    build = TRAY[TRAY.index("fn build_control(") :]
    build = build[: build.index("\n}\n")]
    assert "if let [only] = views {" in build and "ControlMenu::One(" in build and "ControlMenu::Many(" in build
    assert 'format!("{CONTROL_PREFIX}{}", v.label)' in build
    sync = TRAY[TRAY.index("fn sync_control(") :]
    sync = sync[: sync.index("\n}\n")]
    assert "c.labels() == labels" in sync, "rebuilt only when the pet list changes"
    assert "let taken = menu.control.lock().unwrap().take();" in sync
    insert = TRAY[TRAY.index("fn insert_into(") :]
    assert "item.id() == TOGGLE_ID" in insert[: insert.index("\n    }\n")]
    assert "sync_control(app, menu, &views);" in TRAY
    # 펫 하위 메뉴에는 없음 (같은 항목이 두 군데 있지 않음)
    pet = TRAY[TRAY.index("fn build_pet_menu(") :]
    assert "CONTROL_PREFIX" not in pet[: pet.index("\n}\n")]


def test_help_and_feedback_sit_in_one_info_menu_that_rings_for_news():
    assert "pub fn sync_info(" in TRAY and '"ℹ️ 도움말·건의 🔔"' in TRAY
    mailbox = (ROOT / "src-tauri" / "src" / "mailbox.rs").read_text(encoding="utf-8")
    assert "menu.bell[1].store(n > 0" in mailbox and "crate::tray::sync_info(menu);" in mailbox
