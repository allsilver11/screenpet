"""공룡 셋 시트 검사: 모든 동작·투명 여백·pet.json (대사·성격·좋아하는 음식·선택 동작 순서)·eat에 밥그릇 없음·색 (같은 그림, 다른 팔레트)."""
import json

import numpy as np
import pytest

import draw_dino
import draw_pets
from sprite_spec import ANIMS, FRAME, OPTIONAL_ANIMS, SPEECH, STAGES, sheet_128

IDS = ["dino_yellow", "dino_green", "dino_purple"]
BOWL_COLORS = [(246, 196, 150), (252, 226, 196), (230, 222, 206)]


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    for sp in draw_dino.DINOS:
        draw_pets.write_species(sp, root / sp.id)
    return root


def cell(sheet, row, col):
    return sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def names(sp):
    return [a for a, *_ in draw_pets.rows_for(sp)]


def test_registered_with_dino_anims_after_every_other_optional_anim():
    optional = [a for a, *_ in OPTIONAL_ANIMS]
    # 공룡 동작은 선택 동작 목록 맨 끝 (Rust manifest::OPTIONAL_ANIMS와 같은 순서)
    assert optional[-3:] == ["fire", "roar", "stomp"]
    for pid in IDS:
        sp = draw_pets.SPECIES[pid]
        assert sp in draw_dino.DINOS
        extra = ["lookup"] if "lookup" in optional else []
        assert names(sp) == [a for a, *_ in ANIMS] + ["climb", "slide"] + extra + ["fire", "roar", "stomp"]
        assert all(a in sp.poses for a in names(sp))


@pytest.mark.parametrize("pid", IDS)
def test_every_frame_is_drawn_inside_the_cell(out, pid):
    sp = draw_pets.SPECIES[pid]
    rows = draw_pets.rows_for(sp)
    for st in STAGES:
        sheet = sheet_128(out / pid / f"sheet_{st}.png")
        assert sheet.size == (4 * FRAME, len(rows) * FRAME)
        for r, (anim, n, *_) in enumerate(rows):
            for c in range(n):
                box = cell(sheet, r, c).getchannel("A").getbbox()
                assert box, (st, anim, c)
                if anim == "roll":
                    continue
                x0, y0, x1, y1 = box
                assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1 and y1 <= FRAME - 1, (st, anim, c, box)
    assert not (out / pid / "sheet.png").exists()


def test_manifests(out):
    ms = {pid: json.loads((out / pid / "pet.json").read_text(encoding="utf-8")) for pid in IDS}
    assert [ms[p]["name"] for p in IDS] == ["노란 공룡", "초록 공룡", "보라 공룡"]
    assert [ms[p]["favorite_food"] for p in IDS] == [{"emoji": "🍉", "name": "수박"}, {"emoji": "🥦", "name": "브로콜리"},
                                                     {"emoji": "🍇", "name": "포도"}]
    assert [ms[p]["personality"] for p in IDS] == ["playful", "friendly", "friendly"]
    for pid, m in ms.items():
        sp = m["speech"]
        assert sp["chatter"] == ["rawr!", "♪"] and sp["tickle"] == ["hehe!"] and sp["grumpy"] == ["grrr~"]
        assert sp["roar"] == ["RAWR!", "grrr~"] and sp["fire"] == ["fwoosh~"] and sp["stomp"] == ["stomp!"]
        assert sp["hungry"] == SPEECH["cat"]["hungry"]
        # 보통 집 · 걷는다 · 벽을 탄다 (climb·slide 둘 다 있어야 앱이 켠다) · 크기 1.2
        assert not {"house", "locomotion", "hop_share"} & set(m)
        assert not {"arch", "wag"} & set(m["animations"])
        assert set(m["stage_size"].values()) == {1.2}  # 토끼만큼 — "너무 크다" 피드백에도 공룡은 사용자 요청으로 그대로
        a = m["animations"]
        base = len(ANIMS) + 2 + (1 if "lookup" in a else 0)
        assert a["climb"] == {"row": len(ANIMS), "frames": 4, "fps": 6} and a["slide"] == {"row": len(ANIMS) + 1, "frames": 2, "fps": 8}
        assert a["fire"] == {"row": base, "frames": 4, "fps": 4, "loop": False}
        assert a["roar"] == {"row": base + 1, "frames": 3, "fps": 4}
        assert a["stomp"] == {"row": base + 2, "frames": 4, "fps": 6}
        assert "spin" in a["roll"]


@pytest.mark.parametrize("pid", IDS)
def test_eat_has_no_bowl(out, pid):
    """밥그릇은 앱의 소품 창이 띄운다 — 시트 eat에는 그릇 색이 없다."""
    row = names(draw_pets.SPECIES[pid]).index("eat")
    for st in STAGES:
        sheet = sheet_128(out / pid / f"sheet_{st}.png").convert("RGBA")
        for c in range(4):
            a = np.asarray(cell(sheet, row, c)).astype(int)
            for col in BOWL_COLORS:
                near = (np.abs(a[..., :3] - col).sum(axis=2) < 12) & (a[..., 3] > 200)
                assert near.sum() < 25, (st, c, col, near.sum())


def _count(im, col, tol=24):
    a = np.asarray(im.convert("RGBA")).astype(int)
    return int(((np.abs(a[..., :3] - col).sum(axis=2) < tol) & (a[..., 3] > 200)).sum())


def test_palettes_body_spikes_and_dandelion(out):
    """같은 그림을 색만 바꿔 그린다: 몸 색이 넓게, 대비되는 가시 색도 보인다. 노랑은 Dandelion #FFD02E."""
    assert draw_dino.PALETTES["dino_yellow"]["body"] == (0xFF, 0xD0, 0x2E)
    walk = names(draw_dino.DINO_YELLOW).index("walk")
    alphas = []
    for pid in IDS:
        pal = draw_dino.PALETTES[pid]
        f = cell(sheet_128(out / pid / "sheet_teenager.png"), walk, 0)
        assert _count(f, pal["body"]) > 1500, pid
        assert _count(f, pal["spike"]) > 60, pid
        alphas.append(np.asarray(f.getchannel("A")) > 128)
    # 모양은 셋이 같다
    assert (alphas[0] != alphas[1]).sum() < 40 and (alphas[0] != alphas[2]).sum() < 40


def test_baby_wears_an_egg_shell_and_adult_has_the_biggest_spikes(out):
    sit = names(draw_dino.DINO_GREEN).index("sit_front")
    baby = cell(sheet_128(out / "dino_green" / "sheet_baby.png"), sit, 0)
    teen = cell(sheet_128(out / "dino_green" / "sheet_teenager.png"), sit, 0)
    assert _count(baby, draw_dino.SHELL, 10) > 80 and _count(teen, draw_dino.SHELL, 10) < 10
    walk = names(draw_dino.DINO_GREEN).index("walk")
    spike = draw_dino.PALETTES["dino_green"]["spike"]
    counts = [_count(cell(sheet_128(out / "dino_green" / f"sheet_{st}.png"), walk, 0), spike) for st in STAGES]
    assert counts[-1] == max(counts) and counts[0] < counts[2] < counts[-1], counts


def test_fire_puffs_a_flame_inside_the_cell(out):
    """불꽃은 퐁·화르륵 프레임에만 (흡·흐뭇엔 없다), 입 앞 (몸 오른쪽)에."""
    row = names(draw_dino.DINO_PURPLE).index("fire")
    for st in STAGES:
        sheet = sheet_128(out / "dino_purple" / f"sheet_{st}.png")
        fl = [_count(cell(sheet, row, c), draw_dino.FLAME, 30) for c in range(4)]
        assert fl[0] == 0 and fl[3] == 0 and fl[1] > 15 and fl[2] > fl[1], (st, fl)
        a = np.asarray(cell(sheet, row, 2).convert("RGBA")).astype(int)
        xs = np.nonzero((np.abs(a[..., :3] - draw_dino.FLAME).sum(axis=2) < 30) & (a[..., 3] > 200))[1]
        assert xs.min() > 64, st


# ---- 뒷모습 (벽 타기) ----
def back_frames(out, pid, st):
    sheet = sheet_128(out / pid / f"sheet_{st}.png").convert("RGBA")
    ns = names(draw_pets.SPECIES[pid])
    return [cell(sheet, ns.index("climb"), c) for c in range(4)] + [cell(sheet, ns.index("slide"), c) for c in range(2)]


def _patches(im, col, tol):
    """그 색의 2×2 덩어리 수 — 경계의 안티에일리어싱 낱개 픽셀은 세지 않는다."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    h = (a[..., 3] > 200) & (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol)
    return int((h[:-1, :-1] & h[1:, :-1] & h[:-1, 1:] & h[1:, 1:]).sum())


@pytest.mark.parametrize("pid", IDS)
def test_back_view_has_no_face_or_belly_and_spikes_run_down_the_back(out, pid):
    pal = draw_dino.PALETTES[pid]
    for st in STAGES:
        for n, im in enumerate(back_frames(out, pid, st)):
            assert _patches(im, pal["belly"], 12) == 0, (st, n)  # 배 없음
            assert _patches(im, draw_pets.EYE, 12) == 0, (st, n)  # 눈 없음
            assert _patches(im, draw_pets.BLUSH, 24) == 0, (st, n)  # 볼터치 없음
            a = np.asarray(im).astype(int)
            hit = (np.abs(a[..., :3] - pal["spike"]).sum(axis=2) < 24) & (a[..., 3] > 200)
            ys, xs = np.nonzero(hit)
            # 가시는 위아래로 길게 한 줄 (등 가운데, 좌우로 넓게 퍼지지 않는다)
            assert ys.max() - ys.min() >= 28, (st, n, ys.min(), ys.max())
            assert np.abs(xs - np.median(xs)).max() <= 16, (st, n)
            assert _count(im, pal["spike"]) >= 40, (st, n)


def _drawn_order(monkeypatch, sp, st, anim, n):
    tags, orders = {}, []
    compose = draw_pets.compose

    def tagged(kind, fn):
        def f(*a, **k):
            r = fn(*a, **k)
            for p in r if isinstance(r, list) else [r[0] if isinstance(r, tuple) else r]:
                tags[id(p)] = kind
            return r
        return f
    with monkeypatch.context() as m:
        for kind, name in (("foot", "back_foot"), ("arm", "back_arm"), ("tail", "back_tail")):
            m.setattr(draw_dino, name, tagged(kind, getattr(draw_dino, name)))

        def spy(parts, *a, **k):
            orders.append([tags.get(id(p), "body" if p.shapes else "spike") for p in parts])
            tags.clear()  # 다음 프레임이 같은 id를 다시 쓸 수 있다
            return compose(parts, *a, **k)
        m.setattr(draw_pets, "compose", spy)
        with draw_pets.stage(st, sp.stage_tweak.get(st)):
            draw_pets.frames(draw_pets.staged_species(sp), anim, n)
    return orders


def test_feet_and_arms_are_behind_the_body_and_the_tail_on_top(monkeypatch):
    sp = draw_dino.DINO_GREEN
    for st in STAGES:
        for anim, n in (("climb", 4), ("slide", 2)):
            orders = _drawn_order(monkeypatch, sp, st, anim, n)
            assert len(orders) == n
            for o in orders:
                assert o.count("foot") == 2 and o.count("arm") == 2 and o.count("tail") >= 1, o
                body = o.index("body")
                assert max(i for i, t in enumerate(o) if t in ("foot", "arm")) < body < o.index("tail"), o


def test_feet_only_peek_and_take_turns(monkeypatch):
    """발은 언뜻언뜻: 발을 뺀 그림과 비교하면 조금만 보이고, 오르는 동안 왼발·오른발이 번갈아 옆으로 나온다."""
    sp = draw_dino.DINO_GREEN

    def diff(a, b):
        d = np.abs(np.asarray(a).astype(int) - np.asarray(b).astype(int)).sum(axis=2) > 60
        return int(d[:, :FRAME // 2].sum()), int(d[:, FRAME // 2:].sum())
    for st in STAGES:
        with draw_pets.stage(st, sp.stage_tweak.get(st)):
            ssp = draw_pets.staged_species(sp)
            full = draw_pets.frames(ssp, "climb", 4)
            with monkeypatch.context() as m:
                m.setattr(draw_dino, "back_foot", lambda s, *a, **k: draw_pets.Part([], draw_dino.PALETTES[s.id]["body"]))
                bare = draw_pets.frames(ssp, "climb", 4)
        seen = [diff(f, b) for f, b in zip(full, bare)]
        sil = [int((np.asarray(f.getchannel("A")) > 128).sum()) for f in full]
        for n, ((l, r), area) in enumerate(zip(seen, sil)):
            assert 0 < l + r < 0.16 * area, (st, n, l, r, area)
        assert seen[0][1] > seen[0][0] and seen[2][0] > seen[2][1], (st, seen)
