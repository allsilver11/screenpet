"""펭귄 시트 검사: 모든 동작·투명 여백·pet.json(걷는 장난꾸러기, 새우)·검은 등과 흰 배·주황 부리와 발·회색 솜털 아기·
뒤뚱 걸음·배 미끄럼(구르기 대신)·콕콕 먹기·어른 나비넥타이·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_penguin
import draw_pets
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, cell, count_near
from sprite_spec import ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "penguin"
STANDING = ("walk", "run", "roll", "idle", "look", "sleep", "annoyed", "lookup", "sit_front", "wave", "tilt", "eat", "hungry", "heart")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"penguin": draw_penguin.PENGUIN})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    return cell(sheet_128(out / "penguin" / f"sheet_{st}.png"), names(draw_penguin.PENGUIN).index(anim), i)


def alpha(im):
    return np.asarray(im.getchannel("A")) > 128


def count(im, col, tol=20):
    return count_near(im, [col], tol)


def test_registered_with_its_own_poses():
    assert draw_pets.SPECIES["penguin"] is draw_penguin.PENGUIN
    assert names(draw_penguin.PENGUIN) == [a for a, *_ in ANIMS] + ["lookup"]
    assert all(a in draw_penguin.PENGUIN.poses for a in names(draw_penguin.PENGUIN))


def test_every_frame_is_drawn_inside_the_cell(out):
    # 배 미끄럼(roll)도 돌리지 않으니 칸 안에, 바닥에 닿는다
    assert_frames_inside_cell(out / "penguin", draw_penguin.PENGUIN, STAGES, grounded=STANDING)


def test_manifest_is_a_playful_walker_that_loves_shrimp(out):
    m = json.loads((out / "penguin" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "펭귄"
    assert m["favorite_food"] == {"emoji": "🦐", "name": "새우"}
    assert m["personality"] == "playful"
    assert "locomotion" not in m and "hop_share" not in m and "house" not in m
    assert "spin" not in m["animations"]["roll"], "구르는 대신 배로 미끄러진다 (앱이 돌리지 않는다)"
    assert "climb" not in m["animations"] and "sting" not in m["animations"]
    assert "waddle~" in m["speech"]["chatter"]
    assert m["stage_size"] == dict(dict.fromkeys(STAGES, 0.85), baby=0.935)  # 아기만 1.1배 (아기 펭귄이 너무 작아 보였다)


def test_the_build_reproduces_the_committed_penguin(out):
    names_ = sorted(p.name for p in SHIPPED.iterdir())
    assert names_ == sorted(p.name for p in (out / "penguin").iterdir())
    for name in names_:
        assert (out / "penguin" / name).read_bytes() == (SHIPPED / name).read_bytes(), name


@pytest.mark.parametrize("st", ["child", "teenager", "adult"])
@pytest.mark.parametrize("anim", ["sit_front", "idle", "walk"])
def test_black_back_white_belly_orange_beak_and_feet(out, st, anim):
    im = frame(out, st, anim, 0)
    area = int(alpha(im).sum())
    back = count(im, draw_penguin.back_color(st))
    belly = count(im, draw_penguin.WHITE_BELLY, 12) + count(im, draw_penguin.LENS, 12)  # 흰 얼굴 일부는 안경 렌즈가 덮는다
    orange = count(im, draw_penguin.BEAK, 24) + count(im, draw_penguin.FEET, 24)
    assert back > 0.18 * area, (st, anim, back, area)
    assert belly > 0.14 * area, (st, anim, belly, area)  # 옆모습은 안경 테·볼이 얼굴 흰 칸을 조금 가린다
    assert orange > 15, (st, anim, orange)
    assert sum(draw_penguin.back_color(st)) < 300, "검은(짙은) 등"


def grey(im):
    """회색 솜털 칸 수: 채도가 낮고 중간~연한 밝기 (회색 몸·연회색 배·몸 음영, 새하얀 얼굴은 빼고)."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    rgb = a[..., :3]
    lum = rgb.mean(axis=2)
    return int(((rgb.max(axis=2) - rgb.min(axis=2) < 30) & (lum > 140) & (lum < 236) & (a[..., 3] > 200)).sum())


@pytest.mark.parametrize("anim", ["sit_front", "idle"])
def test_baby_is_a_grey_fluffy_chick(out, anim):
    """아기: 회색 솜털 병아리 (검은 등이 아직 없다) · 흰 얼굴 · 정수리 솜털이 삐죽 (머리 꼭대기가 뾰족하게 좁다)."""
    baby = frame(out, "baby", anim, 0)
    area = int(alpha(baby).sum())
    assert grey(baby) > 0.2 * area  # 외곽선·검은 모자·흰 얼굴을 빼면 몸 대부분
    assert count(baby, draw_penguin.back_color("adult")) < 0.01 * area  # 검은 등은 아직 없다 (외곽선 섞인 몇 점만)
    assert grey(baby) > 3 * grey(frame(out, "adult", anim, 0))  # 어른은 회색 솜털 없이 검정·흰색 (경계 섞인 칸만)
    assert count(baby, draw_penguin.BEAK, 24) > 4

    def top_width(st):
        a = alpha(frame(out, st, anim, 0))
        rows = np.nonzero(a.any(axis=1))[0]
        return int(a[rows[0] + 2].sum())
    assert 1.5 * top_width("baby") < top_width("teenager"), (top_width("baby"), top_width("teenager"))  # 솜털 가닥만 삐죽


@pytest.mark.parametrize("st", STAGES)
def test_walk_is_a_side_to_side_waddle(out, st):
    """뒤뚱뒤뚱: 0·2 장은 몸이 서로 반대쪽으로 기우뚱 (윗부분 무게중심이 반대쪽), 1·3 장은 가운데."""
    def lean(i):
        a = alpha(frame(out, st, "walk", i))
        ys, xs = np.nonzero(a)
        top = ys < np.percentile(ys, 35)
        bottom = ys > np.percentile(ys, 80)
        return xs[top].mean() - xs[bottom].mean()
    l0, l1, l2 = lean(0), lean(1), lean(2)
    assert abs(l0 - l2) >= 5, (st, l0, l1, l2)
    assert min(l0, l2) < l1 < max(l0, l2), (st, l0, l1, l2)


@pytest.mark.parametrize("st", STAGES)
def test_roll_is_a_belly_slide_on_the_floor(out, st):
    """구르기 대신 배로 쭉 미끄러진다: 바닥에 길게 눕고, 흰 배가 아래 · 검은 등이 위, 두 장이 다르다 (얼음 가루)."""
    frames = [frame(out, st, "roll", c) for c in range(2)]
    for im in frames:
        x0, y0, x1, y1 = im.getchannel("A").getbbox()
        assert y1 >= 118 and x1 - x0 >= 70 and y1 - y0 <= 70 and x1 - x0 > 1.3 * (y1 - y0), (st, (x0, y0, x1, y1))
        a = np.asarray(im.convert("RGBA")).astype(int)
        white = (np.abs(a[..., :3] - draw_penguin.WHITE_BELLY).sum(axis=2) < 12) & (a[..., 3] > 200)
        dark = (np.abs(a[..., :3] - draw_penguin.back_color(st)).sum(axis=2) < 20) & (a[..., 3] > 200)
        if st not in ("baby", "kinder"):
            assert np.nonzero(white)[0].mean() > np.nonzero(dark)[0].mean(), st
    assert frames[0].tobytes() != frames[1].tobytes()


@pytest.mark.parametrize("st", STAGES)
def test_eat_pecks_down_to_the_bowl_and_back_up(out, st):
    """콕콕: 0·2 = 부리 끝이 그릇 밥 더미(프레임 y 84) 근처, 1·3 = 고개를 들고 냠 (부리가 더 위)."""
    def beak_bottom(i):
        a = np.asarray(frame(out, st, "eat", i).convert("RGBA")).astype(int)
        m = (np.abs(a[..., :3] - np.array(draw_penguin.BEAK)).sum(axis=2) < 24) & (a[..., 3] > 200)
        m[:, :52] = m[:, 77:] = False
        m[100:] = False  # 발은 빼고 부리만
        assert m.sum() > 3, (st, i)
        ys, xs = np.nonzero(m)
        assert abs(xs.mean() - 64) <= 4, (st, i, xs.mean())
        return ys.max()
    down, up = beak_bottom(0), beak_bottom(1)
    assert 78 <= down <= 92, (st, down)
    assert down - up >= 6, (st, down, up)


@pytest.mark.parametrize("st", STAGES)
def test_eat_has_no_bowl(out, st):
    assert_eat_has_no_bowl(out / "penguin" / f"sheet_{st}.png", draw_penguin.PENGUIN)


def test_bow_tie_only_on_the_adult(out):
    for anim in ("sit_front", "idle"):
        assert count(frame(out, "adult", anim, 0), draw_penguin.BOWTIE) > 10, anim
        assert count(frame(out, "teenager", anim, 0), draw_penguin.BOWTIE) == 0, anim






# ---- 작은 형태 연산 (numpy만 — CI에는 scipy가 없다, requirements.txt) ----
_CROSS = ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1))
_SQUARE = tuple((dy, dx) for dy in (-1, 0, 1) for dx in (-1, 0, 1))


def _shift(m, dy, dx):
    """m을 (dy, dx)만큼 민 것 (밖은 False)."""
    out = np.zeros_like(m)
    h, w = m.shape
    out[max(dy, 0): h + min(dy, 0), max(dx, 0): w + min(dx, 0)] = m[max(-dy, 0): h + min(-dy, 0), max(-dx, 0): w + min(-dx, 0)]
    return out


def dilate(m, iterations=1, offsets=_CROSS):
    """이진 팽창 (기본: 4-이웃 십자 — scipy binary_dilation 기본값과 같다)."""
    m = np.asarray(m, dtype=bool)
    for _ in range(iterations):
        m = np.logical_or.reduce([_shift(m, dy, dx) for dy, dx in offsets])
    return m


def erode(m, offsets=_CROSS):
    """이진 침식 (밖은 False로 본다)."""
    m = np.asarray(m, dtype=bool)
    return np.logical_and.reduce([_shift(m, -dy, -dx) for dy, dx in offsets])


def open3(m):
    """3×3 네모로 열기 (침식 뒤 팽창)."""
    return dilate(erode(m, _SQUARE), 1, _SQUARE)


def label(m):
    """4-이웃으로 이어진 덩어리에 1..n 번호: (번호 배열, n)."""
    m = np.asarray(m, dtype=bool)
    lab = np.zeros(m.shape, dtype=int)
    n = 0
    for y, x in zip(*np.nonzero(m)):
        if lab[y, x]:
            continue
        n += 1
        lab[y, x] = n
        stack = [(y, x)]
        while stack:
            cy, cx = stack.pop()
            for dy, dx in _CROSS[1:]:
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < m.shape[0] and 0 <= nx < m.shape[1] and m[ny, nx] and not lab[ny, nx]:
                    lab[ny, nx] = n
                    stack.append((ny, nx))
    return lab, n


def test_morphology_helpers():
    m = np.zeros((7, 7), dtype=bool)
    m[3, 3] = True
    assert dilate(m).sum() == 5 and dilate(m, 2).sum() == 13
    sq = np.zeros((7, 7), dtype=bool)
    sq[1:4, 1:4] = True
    sq[5, 5] = True
    want = np.zeros((7, 7), dtype=bool)
    want[1:4, 1:4] = True
    assert (open3(sq) == want).all()
    lab, n = label(sq)
    assert n == 2 and lab[5, 5] != lab[1, 1]


# ---- 동글 안경 + "안경 번쩍" ----
PIXEL = ROOT / "ui" / "pets_pixel" / "penguin"


def pixel_frame(st, anim, i):
    m = json.loads((PIXEL / "pet.json").read_text(encoding="utf-8"))
    fs, row = m["frame_sizes"][st], m["animations"][anim]["row"]
    from PIL import Image
    return Image.open(PIXEL / f"sheet_{st}.png").convert("RGBA").crop((i * fs, row * fs, (i + 1) * fs, (row + 1) * fs))


def lens_and_rim(im):
    """(렌즈 색 칸 수, 렌즈 둘레(렌즈에서 3칸 안)의 짙은 테 칸 수, 렌즈 가운데 y / 실루엣 높이)."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    op = a[..., 3] > 200
    lens = (np.abs(a[..., :3] - np.array(draw_penguin.LENS)).max(axis=2) <= 6) & op
    rim = (a[..., :3].mean(axis=2) < 120) & op
    near = dilate(lens, 3) & ~lens
    ys = np.nonzero(op.any(axis=1))[0]
    ly = np.nonzero(lens)[0].mean() if lens.any() else 999
    return int(lens.sum()), int((rim & near).sum()), (ly - ys[0]) / (ys[-1] - ys[0] + 1)


@pytest.mark.parametrize("st", STAGES)
@pytest.mark.parametrize("anim", ["sit_front", "idle", "sleep", "eat"])
def test_big_round_glasses_at_every_stage(out, st, anim):
    """모든 단계에 큰 동글 안경: 렌즈 전용 색이 얼굴(실루엣 위쪽)에 있고, 렌즈 둘레에 가는 짙은 테. 동글·도트 둘 다."""
    front = anim in ("sit_front", "eat")
    for kind, im, lmin, rmin in (("round", frame(out, st, anim, 0), 30 if front else 8, 12), ("pixel", pixel_frame(st, anim, 0), 6 if front else 2, 6)):
        lens, rim, ly = lens_and_rim(im)
        assert lens >= lmin, (kind, st, anim, lens)
        assert rim >= rmin, (kind, st, anim, rim)
        assert ly < (0.78 if anim == "sleep" else 0.55), (kind, st, anim, ly)  # 얼굴에 (잘 때는 고개를 푹 숙인다)


def test_front_view_has_two_lenses_side_view_one(out):
    """앞모습은 얼굴 양쪽에 렌즈 둘 (가운데 부리 양옆), 옆모습은 앞쪽 눈에 렌즈 하나 (좁게 모여 있다)."""
    for st in STAGES:
        for im in (frame(out, st, "sit_front", 0), pixel_frame(st, "sit_front", 0)):
            a = np.asarray(im.convert("RGBA")).astype(int)
            lens = (np.abs(a[..., :3] - np.array(draw_penguin.LENS)).max(axis=2) <= 6) & (a[..., 3] > 200)
            xs = np.nonzero(lens)[1]
            mid = np.nonzero((a[..., 3] > 200).any(axis=0))[0].mean()
            assert (xs < mid - 2).sum() >= 3 and (xs > mid + 2).sum() >= 3, st
        for im in (frame(out, st, "idle", 0), pixel_frame(st, "idle", 0)):
            a = np.asarray(im.convert("RGBA")).astype(int)
            op = a[..., 3] > 200
            lens = (np.abs(a[..., :3] - np.array(draw_penguin.LENS)).max(axis=2) <= 6) & op
            xs, sx = np.nonzero(lens)[1], np.nonzero(op.any(axis=0))[0]
            assert xs.max() - xs.min() < 0.36 * (sx[-1] - sx[0] + 1), st  # 렌즈 하나
            assert xs.mean() > sx.mean(), st  # 오른쪽(앞)을 본다


def _filter(fid):
    html = (ROOT / "ui" / "index.html").read_text(encoding="utf-8")
    f = html[html.index(f'<filter id="{fid}"'):]
    f = f[: f.index("</filter>")]
    assert 'color-interpolation-filters="sRGB"' in f
    return f


def _flash_tables(chans="RGB", fid="lens-flash-px"):
    """ui/index.html 필터의 채널별 discrete 표. 도트 #lens-flash-px: 렌즈 색만 1 · 동글 #lens-flash: 알파 254만 1."""
    import re
    f = _filter(fid)
    tabs = {}
    for ch in chans:
        m = re.search(rf'<feFunc{ch} type="discrete" tableValues="([^"]+)"', f)
        tabs[ch] = [float(v) for v in m.group(1).split()]
    return f, tabs


def _keyed(tabs, rgb):
    return all(tabs[ch][min(len(tabs[ch]) - 1, int(v / 255 * len(tabs[ch])))] == 1 for ch, v in zip("RGB", rgb))


def test_round_lens_flash_keys_the_marked_lens_alpha(out):
    """동글 그림: 렌즈 둥근 자리(안의 눈까지)만 알파 254로 표시돼 있고, #lens-flash 필터는 알파 254만 골라 하얗게 덮는다."""
    f, tabs = _flash_tables("A", "lens-flash")
    assert f.index('operator="erode" radius="1"') < f.index('operator="dilate" radius="1"')
    assert [k for k, v in enumerate(tabs["A"]) if v == 1] == [draw_penguin.LENS_ALPHA] and len(tabs["A"]) == 256
    for st in STAGES:
        sheet = out / "penguin" / f"sheet_{st}.png"
        from PIL import Image
        full = Image.open(sheet)
        assert (np.asarray(full.getchannel("A")) == draw_penguin.LENS_ALPHA).sum() > 500, st
        for anim in ("sit_front", "idle", "eat", "sleep"):
            im = frame(out, st, anim, 0)
            a = np.asarray(im.convert("RGBA")).astype(int)
            # 필터처럼 열림(1px)으로 외톨이 점을 지운다 (어른 256칸 시트를 128로 줄이면 외곽선에 254가 생길 수 있다)
            key = open3(a[..., 3] == draw_penguin.LENS_ALPHA)
            lens = (np.abs(a[..., :3] - np.array(draw_penguin.LENS)).max(axis=2) <= 6) & (a[..., 3] > 200)
            assert key.sum() >= lens.sum() + 10, (st, anim)  # 렌즈 색 칸 + 눈·반짝까지
            dark = key & (a[..., :3].mean(axis=2) < 90)
            assert dark.sum() >= 4, (st, anim)  # 렌즈 안의 눈도 덮는다
            ys = np.nonzero(a[..., 3] > 128)[0]
            assert (np.nonzero(key)[0].max() - ys.min()) / (ys.max() - ys.min() + 1) < (0.88 if anim == "sleep" else 0.7), (st, anim)  # 얼굴 높이 (아기는 머리가 커서 조금 낮다)
    # 몸 가장자리엔 외톨이 254가 없다 (외곽선 반투명 칸이 번쩍이지 않게): 254는 모두 렌즈 원 덩어리 (시트 그대로 — 128칸 단계)
    for st in ("baby", "kinder", "child", "teenager"):
        for anim in ("walk", "roll", "belly", "heart", "level_up", "sleep"):
            for c in range(2):
                a = np.asarray(frame(out, st, anim, c).convert("RGBA")).astype(int)
                lab, n = label(a[..., 3] == draw_penguin.LENS_ALPHA)
                sizes = [int((lab == j).sum()) for j in range(1, n + 1)]
                assert 1 <= n <= 3 and min(sizes) >= 20, (st, anim, c, sizes)


def test_lens_flash_filter_keys_only_the_lens_colour():
    """도트 안경 번쩍 (#lens-flash-px): 렌즈 전용 색만 골라 하얗게 덮는다 (눈 구멍은 닫힘(closing)으로 메움). 몸의 다른 색은 그대로."""
    f, tabs = _flash_tables()
    assert _keyed(tabs, draw_penguin.LENS)
    others = [draw_penguin.WHITE_BELLY, draw_penguin.ICE, draw_penguin.ICE_DARK, draw_penguin.CHICK_GREY, draw_penguin.CHICK_GREY_K,
              draw_penguin.CHICK_CAP, draw_penguin.belly_color("baby"), draw_penguin.belly_color("kinder"), draw_penguin.GLASS_ARM,
              draw_penguin.GLASS_FRAME, draw_pets.WHITE] + list(draw_penguin.BACK.values())
    assert not [c for c in others if _keyed(tabs, c)]
    assert 'operator="dilate"' in f and 'operator="erode"' in f and 'flood-color="#fff' in f


@pytest.mark.parametrize("st", STAGES)
def test_only_the_glasses_wear_the_lens_colour(out, st):
    """도트 필터가 렌즈 말고는 하얗게 만들지 않게: 필터 표에 걸리는 칸은 렌즈 덩어리 바로 곁뿐 (배·발·얼음 가루·외곽선 섞인 칸은 몇 점 이하)."""
    _, tabs = _flash_tables()
    keyed = {}
    for anim in ("sit_front", "walk", "roll", "belly", "eat", "sleep", "heart"):
        for kind, im in (("round", frame(out, st, anim, 0)), ("pixel", pixel_frame(st, anim, 0))):
            a = np.asarray(im.convert("RGBA")).astype(int)
            op = a[..., 3] > 200
            rgb = a[..., :3]
            hit = op.copy()
            for ch in range(3):
                t = np.array(tabs["RGB"[ch]])
                hit &= t[np.minimum(len(t) - 1, (rgb[..., ch] / 255 * len(t)).astype(int))] == 1
            core = (np.abs(rgb - np.array(draw_penguin.LENS)).max(axis=2) <= 6) & op
            stray = hit & ~dilate(core, 2)
            assert hit.sum() >= 3, (kind, st, anim)
            keyed[(kind, anim)] = int(stray.sum())
    assert max(keyed.values()) <= 4, keyed


def test_penguin_flashes_its_glasses_on_click():
    """누르면(잡을 때도) 안경 번쩍: PET_FX에 펭귄 · CSS 애니메이션이 #lens-flash 필터를 켜고 ✦ 반짝이."""
    js = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    css = (ROOT / "ui" / "style.css").read_text(encoding="utf-8")
    fx = js[js.index("const PET_FX = {"):]
    fx = fx[: fx.index("};")]
    assert 'penguin: { on: ["click", "play"], cls: "glasses-flash" }' in fx
    assert "if (!fx || ![].concat(fx.on).includes(trigger)) return;" in js
    assert "#pet.glasses-flash {" in css and "@keyframes glasses-flash" in css
    kf = css[css.index("@keyframes glasses-flash"):]
    assert "url(#lens-flash)" in kf[: kf.index("\n}\n")]
    kp = css[css.index("@keyframes glasses-flash-px"):]
    assert "url(#lens-flash-px)" in kp[: kp.index("\n}\n")] and "body.pixel #pet.glasses-flash { animation-name: glasses-flash-px; }" in css
    assert "#pet.glasses-flash::before" in css and "✦" in css[css.index("#pet.glasses-flash::before"):][:400]


def test_playful_moments_from_the_app_flash_the_glasses():
    """장난칠 때 (백엔드 render.flash가 켜지는 순간 — brain_flash.rs) 웹뷰가 petFx("play")로 같은 번쩍 클래스를 붙인다. node로 그대로 돌린다."""
    import subprocess
    js = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    fx = js[js.index("const PET_FX = {"):]
    fx = fx[: fx.index("\n// 반짝이(::before/::after)의 animationend")]
    listen = js[js.index('const listeningRender = listen("pet-render", (e) => {'):]
    line = next(ln for ln in listen.splitlines() if "e.payload.flash" in ln)
    script = """
const added = [];
const petEl = { classList: { remove() {}, add(c) { added.push(c); } }, offsetWidth: 0 };
const stage = { pet: "penguin" };
let render = { flash: false };
%s
function onRender(e) { %s render = e.payload; }
onRender({ payload: { flash: true } });   // 켜지는 순간 번쩍
onRender({ payload: { flash: true } });   // 켜져 있는 동안 다시 붙이지 않는다
onRender({ payload: { flash: false } });
onRender({ payload: { flash: true } });   // 다음 장난
stage.pet = "cat"; render = { flash: false };
onRender({ payload: { flash: true } });   // 효과 없는 동물은 그대로
petFx("click"); stage.pet = "penguin"; petFx("click");
console.log(JSON.stringify(added));
""" % (fx, line.strip())
    out = subprocess.run(["node", "-e", script], capture_output=True, text=True, check=True).stdout
    assert json.loads(out) == ["glasses-flash", "glasses-flash", "glasses-flash"]


# ---- 매끈한 등 (사용자: 2단계부터 등에 털인지 뿔인지 뾰족뾰족) ----

def test_only_the_baby_keeps_the_spiky_down_on_its_body():
    """2단계(유아) 등은 매끈하게 (사용자: 등에 털인지 뿔인지 뾰족뾰족) — 삐죽 솜털은 아기 병아리만."""
    assert [lvl for lvl, st in draw_penguin.PG_STAGE.items() if st["fuzz"]] == [0]


@pytest.mark.parametrize("st", STAGES)
@pytest.mark.parametrize("anim", ["idle", "walk", "look", "sit_front"])  # 잠(sleep)은 머리 위에 z가 있다
def test_pixel_head_has_no_horn(st, anim):
    """도트 그림: 정수리 솜털 한 가닥이 작은 칸에선 뾰족한 뿔(끝에 주황 점)이 된다 → 도트엔 정수리 솜털이 없다.
    머리 꼭대기 줄이 한두 칸짜리 뾰족 끝이 아니고, 위 1/5에 부리·발 주황이 없다."""
    for i in (0, 1):
        im = pixel_frame(st, anim, i)
        a = alpha(im)
        ys = np.nonzero(a.any(axis=1))[0]
        # 뾰족 뿔이면 위 세 줄이 한두 칸씩 (동그란 머리는 위 셋째 줄이 넓다)
        assert a[ys[0] + 2].sum() >= 5, (st, anim, i, [int(a[ys[0] + k].sum()) for k in range(3)])
        band = im.crop((0, ys[0], im.width, ys[0] + (ys[-1] - ys[0]) // 5))
        assert count_near(band, [draw_penguin.FEET], 30) == 0, (st, anim, i)


def _back_bumps(im):
    """옆모습 등 쪽(왼쪽) 윤곽: 줄마다 가장 왼쪽 칸 x. 위에서 아래로 그 x가 앞뒤 줄보다 2칸 넘게 튀어나온(작은) 곳 수 + 아래 1/4이 몸통보다 왼쪽으로 튀어나온 칸 수."""
    a = alpha(im)
    rows = [np.nonzero(r)[0].min() for r in a if r.any()]
    n = len(rows)
    spikes = sum(1 for i in range(1, n - 1) if rows[i] < min(rows[i - 1], rows[i + 1]) - 2)
    mid = min(rows[int(n * 0.35): int(n * 0.75)])
    tail = max(0, mid - min(rows[int(n * 0.75):]))
    return spikes, tail


@pytest.mark.parametrize("st", ["kinder", "child", "teenager", "adult"])
@pytest.mark.parametrize("anim", ["idle", "walk", "look"])
def test_the_back_is_smooth_without_spikes_or_a_pointy_tail(out, st, anim):
    for i in (0, 1):
        spikes, tail = _back_bumps(frame(out, st, anim, i))
        assert spikes == 0 and tail <= 2, (st, anim, i, spikes, tail)







def test_glaring_lenses_stay_white_while_the_app_says_so():
    """안경 반사 (render.glare, brain_flash.rs): 켜져 있는 동안 렌즈가 하얗게 차 있다 — 번쩍(✦ 애니메이션)과 달리 그대로 머문다.
    동글은 #lens-flash, 도트는 #lens-flash-px 필터. 번쩍 애니메이션이 끝나면 반사로 돌아간다 (애니메이션은 forwards가 아니다)."""
    js = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    css = (ROOT / "ui" / "style.css").read_text(encoding="utf-8")
    assert 'if (e.payload.glare) petEl.classList.add("glasses-glare");' in js
    assert 'else petEl.classList.remove("glasses-glare");' in js
    assert "#pet.glasses-glare { filter: url(#lens-flash); }" in css
    assert "body.pixel #pet.glasses-glare { filter: url(#lens-flash-px); }" in css
    flash = css[css.index("#pet.glasses-flash {"):]
    assert "forwards" not in flash[: flash.index("}")]
