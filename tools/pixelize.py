"""픽셀(네모네모) 버전 프로토타입 — 기존 동글동글 시트의 프레임을 저해상도 도트로 변환해 비교한다.

앱 코드·시트는 건드리지 않는다. 시트는 작업 트리가 아니라 커밋(`--ref`, 기본 HEAD)에서 읽는다.

접근법
  a: 면적 평균 축소 + 팔레트 양자화 + 알파 문턱값 (그냥 줄이기)
  b: a + 몸을 격자에 맞춰 키움 + 1px 어두운 외곽선 + 눈·볼 도장
  c: b + 평균 대신 칸마다 최빈색(원본 외곽선 띠·얼굴 제외, 드문 색 가중) → 색이 탁해지지 않는다.
     외곽선은 자리마다 원본의 어두운 색을 따라가고(selective outline), 격자를 몸 중심·바닥에 맞춰
     좌우 대칭을 유지하며, 얼룩 점과 떨어진 효과(zZ)를 정리한다.
  집·밥그릇·손 음식·선물·물건은 pixelize_art (c와 같은 최빈색 + 외곽선, 얼굴 도장 없음).

사용
  python tools/pixelize.py --preview                   # tools/preview/pixel_proto.png
  python tools/pixelize.py --pet cat --stage baby --anim walk --frame 0 --size 32 --approach c --out x.png
"""

import argparse
import io
import json
import subprocess
from collections import Counter, deque
from functools import lru_cache
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
FRAME = 128
STAGES = ["baby", "kinder", "child", "teenager", "adult"]

EYE_RGB = np.array([84, 58, 56])  # draw_pets.EYE
EYE_LIGHT_RGB = np.array([138, 96, 92])  # draw_pets.EYE_LIGHT
BLUSH_RGB = np.array([252, 196, 194])  # draw_pets.BLUSH
BLUSH_LINE_RGB = np.array([238, 150, 150])  # draw_pets.BLUSH_LINE
ZZZ_RGB = np.array([112, 132, 196])  # draw_pets.ZZZ
GLOW_Y_RGB = np.array([255, 226, 92])  # draw_halloween 호박 얼굴
GLOW_CORE_RGB = np.array([255, 252, 218])
CARVE_RGB = np.array([128, 58, 30])
LANTERN_PIX = (255, 226, 92, 255)
CARVE_PIX = (128, 58, 30, 255)
EYE_DARK = (58, 38, 42, 255)  # 도장 눈: 원본보다 살짝 더 진하게
EYE_SHINE = (255, 255, 255, 255)
EYE_IRIS = (138, 96, 92, 255)
MOUTH_PIX = (150, 96, 96, 255)
BLUSH_PIX = (250, 150, 160, 255)
Z_PIX = (112, 144, 214, 255)
PALETTE_SIZE = 14

SHOW_PETS = ["cat", "rabbit", "bee", "pumpkin", "rudolph", "giraffe", "dino_yellow"]
SHOW_STAGES = ["baby", "adult"]
SHOW_ANIMS = ["walk", "sit_front", "sleep"]
LIGHT_BG = (240, 240, 244, 255)
DARK_BG = (44, 46, 58, 255)


# ---------------------------------------------------------------- 시트 읽기
@lru_cache(maxsize=None)
def _git_show(ref, rel):
    return subprocess.check_output(["git", "-C", str(ROOT), "show", f"{ref}:{rel}"])


def load_manifest(pet, ref="HEAD"):
    return json.loads(_git_show(ref, f"ui/pets/{pet}/pet.json"))


@lru_cache(maxsize=32)
def load_sheet(pet, stage, ref="HEAD"):
    raw = _git_show(ref, f"ui/pets/{pet}/sheet_{stage}.png")
    return np.array(Image.open(io.BytesIO(raw)).convert("RGBA"))


def get_frame(pet, stage, anim, index=0, ref="HEAD"):
    man = load_manifest(pet, ref)
    fs = man["frame_size"]
    row = man["animations"][anim]["row"]
    return load_sheet(pet, stage, ref)[row * fs:(row + 1) * fs, index * fs:(index + 1) * fs].copy()


# ---------------------------------------------------------------- 공통 도구
def components(mask, connectivity=8):
    """연결 성분 목록 — 각 성분은 (ys, xs) 배열. scipy 없이 BFS."""
    h, w = mask.shape
    seen = np.zeros(mask.shape, bool)
    steps = [(1, 0), (-1, 0), (0, 1), (0, -1)]
    if connectivity == 8:
        steps += [(1, 1), (1, -1), (-1, 1), (-1, -1)]
    out = []
    for y0, x0 in zip(*np.nonzero(mask)):
        if seen[y0, x0]:
            continue
        seen[y0, x0] = True
        q, ys, xs = deque([(y0, x0)]), [], []
        while q:
            y, x = q.popleft()
            ys.append(y)
            xs.append(x)
            for dy, dx in steps:
                ny, nx = y + dy, x + dx
                if 0 <= ny < h and 0 <= nx < w and mask[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    q.append((ny, nx))
        out.append((np.array(ys), np.array(xs)))
    return out


def erode(mask, n=1):
    """4-이웃 침식 n번 (프레임 바깥은 투명으로 본다)."""
    m = mask.copy()
    for _ in range(n):
        p = np.pad(m, 1)
        m = p[1:-1, 1:-1] & p[:-2, 1:-1] & p[2:, 1:-1] & p[1:-1, :-2] & p[1:-1, 2:]
    return m


def body_mask(frame):
    """가장 큰 덩어리 = 몸. 떨어져 있는 조각(zZ, 하트…)은 효과로 본다."""
    solid = frame[..., 3] >= 128
    comps = components(solid)
    body = np.zeros(solid.shape, bool)
    fx = []
    if not comps:
        return body, fx
    comps.sort(key=lambda c: -len(c[0]))
    body[comps[0]] = True
    fx = comps[1:]
    return body, fx


@lru_cache(maxsize=64)
def species_palette(pet, ref="HEAD", k=PALETTE_SIZE):
    """종 전체(5단계) 시트에서 넓은 면을 차지하는 색부터 뽑는다 — 비슷한 색은 합친다."""
    counts = Counter()
    for st in STAGES:
        s = load_sheet(pet, st, ref)
        px = s[s[..., 3] >= 250][:, :3]
        q = (px // 4).astype(np.int32)
        keys, n = np.unique(q[:, 0] * 65536 + q[:, 1] * 256 + q[:, 2], return_counts=True)
        for key, c in zip(keys.tolist(), n.tolist()):
            counts[key] += c
    picks = []
    for key, c in counts.most_common(4000):
        rgb = np.array([(key >> 16) * 4 + 2, ((key >> 8) & 255) * 4 + 2, (key & 255) * 4 + 2], float)
        if all(np.abs(rgb - p).sum() > 42 for p in picks):
            picks.append(rgb)
            if len(picks) == k:
                break
    return np.array(picks)


def nearest(rgb, palette):
    d = np.abs(rgb[..., None, :].astype(np.int32) - palette.astype(np.int32)).sum(-1)
    return d.argmin(-1)


@lru_cache(maxsize=64)
def sheet_fit(pet, stage, ref="HEAD"):
    """시트 전체(모든 동작)의 몸 경계 → 한 칸 크기 계산의 기준. 프레임마다 바꾸면 애니메이션 중 크기가 튄다."""
    man = load_manifest(pet, ref)
    fs = man["frame_size"]
    sheet = load_sheet(pet, stage, ref)
    x0, y0, x1, y1 = fs, fs, 0, 0
    bottom = 0
    for name, a in man["animations"].items():
        for i in range(a["frames"]):
            f = sheet[a["row"] * fs:(a["row"] + 1) * fs, i * fs:(i + 1) * fs]
            body, _ = body_mask(f)
            if not body.any():
                continue
            ys, xs = np.nonzero(body)
            x0, x1 = min(x0, xs.min()), max(x1, xs.max() + 1)
            y0, y1 = min(y0, ys.min()), max(y1, ys.max() + 1)
            if name in ("walk", "idle", "sit_front"):
                bottom = max(bottom, ys.max() + 1)
    half = max(64 - x0, x1 - 64)  # 가운데(x=64)를 격자선에 둔 채 좌우 모두 들어가게
    return {"half": float(half), "top": float(y0), "bottom": float(bottom or y1), "low": float(y1)}


def line_color(frame, body):
    """몸 가장자리 띠에서 가장 흔한 색 = 원본 외곽선 색."""
    edge = body & ~erode(body, 2)
    px = frame[edge][:, :3].astype(np.int64) // 8
    if not len(px):
        return np.array([90, 60, 60])
    keys, n = np.unique(px[:, 0] * 65536 + px[:, 1] * 256 + px[:, 2], return_counts=True)
    key = int(keys[n.argmax()])
    return np.array([(key >> 16) * 8 + 4, ((key >> 8) & 255) * 8 + 4, (key & 255) * 8 + 4])


# ---------------------------------------------------------------- 얼굴 찾기
def dilate(mask, n=1):
    m = mask.copy()
    for _ in range(n):
        p = np.pad(m, 1)
        m = p[1:-1, 1:-1] | p[:-2, 1:-1] | p[2:, 1:-1] | p[1:-1, :-2] | p[1:-1, 2:]
    return m


def find_face(frame, body):
    """눈·볼 위치(원본 좌표). 색(draw_pets.EYE, EYE_LIGHT)과 모양으로 찾는다.
    반환: {"eyes": [{cx, cy, w, h, kind}], "blush": [...], "pixels": 도장이 대신할 원본 픽셀 마스크}"""
    rgb = frame[..., :3].astype(np.int32)
    inner = erode(body, 3)
    core = (np.abs(rgb - EYE_RGB).sum(-1) < 40) & inner
    dark = ((np.abs(rgb - EYE_RGB).sum(-1) < 60) | (np.abs(rgb - EYE_LIGHT_RGB).sum(-1) < 36)) & inner
    pixels = np.zeros(body.shape, bool)
    cands = []
    bridged = dilate(dark, 1) & inner  # 반짝이로 쪼개진 눈 조각을 잇는다
    for ys, xs in components(bridged):
        keep = dark[ys, xs]
        if not keep.any():
            continue
        ys, xs = ys[keep], xs[keep]
        w, h = int(xs.max() - xs.min() + 1), int(ys.max() - ys.min() + 1)
        area = len(ys)
        if area < 6 or w > 17 or h > 19 or core[ys, xs].mean() < 0.3:  # 외곽선 색 조각(날개 뿌리 등)은 눈이 아니다
            continue
        cands.append({"cx": (xs.min() + xs.max() + 1) / 2, "cy": (ys.min() + ys.max() + 1) / 2, "w": w, "h": h,
                      "area": area, "fill": area / (w * h), "ys": ys, "xs": xs})
    # 뜬 눈은 세로가 길고(반쯤 감은 어른 눈 포함), 감은 눈(‿ ^)은 납작하다
    opened = [c for c in cands if c["w"] >= 4 and c["area"] >= 12 and (
        (c["h"] >= 5 and c["h"] >= c["w"] * 0.58) or (c["h"] >= 4 and c["h"] >= c["w"] * 0.45 and c["fill"] >= 0.6))]
    eyes = []
    if opened:
        top = min(c["cy"] for c in opened)
        for c in opened:
            if c["cy"] - top <= 6:
                c["kind"] = "open"  # 어른의 반쯤 감은 눈도 칸이 모자라 그대로 옮기면 지저분하다 → 또렷한 뜬 눈
                eyes.append(c)
    else:
        # 가는 호는 축소 때 색이 옅어지므로 문턱을 넓혀 다시 찾는다 (외곽선 띠 안쪽, 납작한 것만)
        loose = (np.abs(rgb - EYE_RGB).sum(-1) < 120) & inner
        for ys, xs in components(loose):
            w, h = int(xs.max() - xs.min() + 1), int(ys.max() - ys.min() + 1)
            if len(ys) >= 6 and 5 <= w <= 15 and h <= 7:
                cands.append({"cx": (xs.min() + xs.max() + 1) / 2, "cy": (ys.min() + ys.max() + 1) / 2, "w": w, "h": h,
                              "area": len(ys), "fill": len(ys) / (w * h), "ys": ys, "xs": xs})
        arcs = [c for c in cands if 5 <= c["w"] <= 15 and c["h"] <= 8 and c["w"] >= c["h"] * 1.2]
        if arcs:
            top = min(c["cy"] for c in arcs)
            for c in arcs:
                if c["cy"] - top <= 3:
                    mid = np.abs(c["xs"] - c["cx"] + 0.5) <= 1.5
                    side = ~mid
                    happy = mid.any() and side.any() and c["ys"][mid].mean() < c["ys"][side].mean()
                    c["kind"] = "happy" if happy else "closed"
                    eyes.append(c)
    eyes = sorted(eyes, key=lambda c: c["cx"])[:2]
    for c in eyes:  # 반짝이(흰 점)까지 눈 전체를 도장 영역으로
        x0, x1 = int(c["xs"].min()), int(c["xs"].max()) + 1
        y0, y1 = int(c["ys"].min()), int(c["ys"].max()) + 1
        pixels[y0:y1, x0:x1] = True

    blush = []
    if eyes:
        pink = ((np.abs(rgb - BLUSH_RGB).sum(-1) < 28) | (np.abs(rgb - BLUSH_LINE_RGB).sum(-1) < 20)) & body
        ey = np.mean([e["cy"] for e in eyes])
        for ys, xs in components(pink):
            if len(ys) < 12:
                continue
            bx, by = (xs.min() + xs.max() + 1) / 2, (ys.min() + ys.max() + 1) / 2
            if by < ey - 1 or by > ey + 20:  # 토끼 귀 속 분홍 같은 건 제외: 눈 높이~바로 아래만
                continue
            if min(abs(bx - e["cx"]) for e in eyes) > 22:
                continue
            blush.append((bx, by))
            pixels[ys, xs] = True
        blush = sorted(blush)[:2]
    return {"eyes": eyes, "blush": blush, "pixels": pixels}


def thin_dark(frame, solid, line):
    """안쪽의 가는 어두운 선(앞발 경계, 입, 수염)만 찾는다 — 칸보다 가늘어 도트에서 얼룩이 된다.
    두꺼운 어두운 면(벌 줄무늬)은 남긴다."""
    rgb = frame[..., :3].astype(np.int32)
    dark = solid & ((np.abs(rgb - line).sum(-1) < 80) | (np.abs(rgb - EYE_RGB).sum(-1) < 70))
    return dark & ~dilate(erode(dark, 2), 2)


def eye_shape(eye, cell, size):
    """도장 크기 (칸). 아기의 큰 눈은 크게 남긴다 — 눈이 클수록 귀엽다."""
    kind = eye["kind"]
    if kind == "open":
        ew = int(np.clip(round(eye["w"] / cell), 2, 3 if size >= 32 else 2))
        eh = int(np.clip(round(eye["h"] / cell), 2, 4 if size >= 32 else 3))
        return ew, max(eh, ew)
    return (3, 2)


def stamp_eye(img, mask, gx, gy, kind, ew, eh, size):
    def put(x, y, c):
        if 0 <= x < size and 0 <= y < size:
            img[y, x] = c
            mask[y, x] = True

    d, s = EYE_DARK, EYE_SHINE
    if kind == "open":
        for i in range(ew):
            for j in range(eh):
                put(gx + i, gy + j, d)
        put(gx, gy, s)  # 왼쪽 위 반짝이
        if ew >= 3 and eh >= 3:
            put(gx + ew - 1, gy + eh - 1, EYE_IRIS)  # 큰 눈은 아래쪽 밝은 홍채
    elif kind == "lantern":  # 호박 세모 눈
        put(gx + 1, gy, LANTERN_PIX)
        for i in (0, 1, 2):
            put(gx + i, gy + 1, LANTERN_PIX)
    elif kind == "closed":  # ‿
        put(gx, gy, d)
        put(gx + 1, gy + 1, d)
        put(gx + 2, gy, d)
    elif kind == "happy":  # ^
        put(gx, gy + 1, d)
        put(gx + 1, gy, d)
        put(gx + 2, gy + 1, d)


def apply_face(img, mask, face, grid, size, boundary, symmetric_about=None):
    """찾은 얼굴을 칸 좌표로 옮겨 도장을 찍는다. 앞모습이면 눈·볼을 중심선에 대칭으로 맞춘다."""
    placed = []
    for e in face["eyes"]:
        ew, eh = eye_shape(e, grid.cell, size)
        gx, gy = grid.to_cell(e["cx"], e["cy"])
        placed.append([int(round(gx - ew / 2)), int(round(gy - eh / 2)), e["kind"], ew, eh])
    if len(placed) == 2:
        row = min(p[1] for p in placed)
        placed[0][1] = placed[1][1] = row
        if symmetric_about is not None:
            gap = max(symmetric_about - placed[0][0] - placed[0][3], placed[1][0] - symmetric_about, 1)
            placed[0][0] = symmetric_about - gap - placed[0][3]
            placed[1][0] = symmetric_about + gap
    centers = []
    for gx, gy, kind, ew, eh in placed:
        for _ in range(3):  # 외곽선을 파먹지 않게 안쪽으로 밀기
            cells = [(gy + j, gx + i) for j in range(eh) for i in range(ew)]
            hit = [c for c in cells if not (0 <= c[0] < size and 0 <= c[1] < size) or boundary[c] or not mask[c]]
            if not hit:
                break
            mx = np.mean([c[1] for c in hit]) - (gx + ew / 2 - 0.5)
            my = np.mean([c[0] for c in hit]) - (gy + eh / 2 - 0.5)
            gx -= int(np.sign(mx)) if abs(mx) > 0.2 else 0
            gy -= int(np.sign(my)) if abs(my) > 0.2 else 0
        stamp_eye(img, mask, gx, gy, kind, ew, eh, size)
        centers.append((gx, gy, ew, eh))
    if not centers:
        return

    def paint(x, y, col):
        if 0 <= x < size and 0 <= y < size and mask[y, x] and not boundary[y, x]:
            if tuple(img[y, x]) not in (EYE_DARK, EYE_SHINE, EYE_IRIS):
                img[y, x] = col

    # 볼: 눈 바깥 아래에 분홍 2칸(32) / 1칸(24) — 앞모습은 대칭
    n_pix = 2 if size >= 32 else 1
    if face.get("lantern_mouth"):  # 호박 입: 빛나는 스마일
        m = face["mouth"]
        gx, gy = grid.to_cell(m["cx"], m["cy"])
        cx = symmetric_about if symmetric_about is not None else int(round(gx))
        gy = max(int(round(gy)) - 1, centers[0][1] + centers[0][3] + 1)
        for i, j in ((-2, 0), (1, 0), (-1, 1), (0, 1)):
            paint(cx + i, gy + j, LANTERN_PIX)
        return
    if len(centers) == 2:
        (lx, ly, lw, lh), (rx, ry, rw, rh) = centers
        if face["blush"]:
            y = ly + lh
            for i in range(n_pix):
                paint(lx - 1 + i - (n_pix - 1), y, BLUSH_PIX)
                paint(rx + rw + i, y, BLUSH_PIX)
        if symmetric_about is not None:  # 앞모습 입: 가운데 2칸
            my = ly + lh
            paint(symmetric_about - 1, my, MOUTH_PIX)
            paint(symmetric_about, my, MOUTH_PIX)
    else:
        for bx, by in face["blush"]:
            gx, gy = grid.to_cell(bx, by)
            gx, gy = int(np.floor(gx)), max(int(np.floor(gy)), centers[0][1] + centers[0][3])
            for i in range(n_pix):
                paint(gx + i, gy, BLUSH_PIX)


# ---------------------------------------------------------------- 격자·칠하기
class Grid:
    """원본 좌표 → 칸 좌표. cell = 칸 한 변(원본 px), (ox, oy) = 칸 (0,0)의 원본 좌표."""

    def __init__(self, cell, ox, oy, w, h=None):
        self.cell, self.ox, self.oy, self.size = cell, ox, oy, w
        self.w, self.h = w, h or w

    @classmethod
    def for_sheet(cls, pet, stage, size, ref="HEAD"):
        """몸 중심(x=64)을 격자선에, 시트 바닥을 맨 아랫줄에 맞추고 시트 전체가 들어가게 키운다."""
        fit = sheet_fit(pet, stage, ref)
        cell = max(fit["half"] / (size / 2), (fit["low"] - fit["top"]) / size, FRAME / size * 0.70)
        return cls(cell, 64 - cell * size / 2, fit["low"] - cell * size, size)

    def to_cell(self, x, y):
        return (x - self.ox) / self.cell, (y - self.oy) / self.cell

    def indices(self, shape):
        ys = np.floor((np.arange(shape[0]) + 0.5 - self.oy) / self.cell).astype(int)
        xs = np.floor((np.arange(shape[1]) + 0.5 - self.ox) / self.cell).astype(int)
        return ys, xs


def boundary_of(mask):
    return mask & ~erode(mask, 1)


def silhouette(cov, thresh):
    """알파 문턱값 + 뾰족한 외톨이 칸 제거 + 1칸 구멍 메우기."""
    m = cov >= thresh
    p = np.pad(m, 1)
    nb4 = p[:-2, 1:-1].astype(int) + p[2:, 1:-1] + p[1:-1, :-2] + p[1:-1, 2:]
    return (m & (nb4 >= 1)) | (~m & (nb4 == 4))


def accumulate(frame, solid, grid):
    """칸 안에 드는 원본 픽셀의 (칸y, 칸x, 원본y, 원본x)."""
    yi, xi = grid.indices(frame.shape[:2])
    yy, xx = np.meshgrid(np.arange(frame.shape[0]), np.arange(frame.shape[1]), indexing="ij")
    gy, gx = yi[yy], xi[xx]
    ok = (gy >= 0) & (gy < grid.h) & (gx >= 0) & (gx < grid.w)
    return gy[ok], gx[ok], yy[ok], xx[ok]


def local_outline(frame, solid, grid, bnd, darken):
    """테두리 칸마다 그 자리 원본의 어두운 픽셀 색을 따라간다 (selective outline) — 벌 날개는 옅게, 몸은 진하게.
    비슷한 색은 묶어 색 수를 줄인다."""
    gy, gx, yy, xx = accumulate(frame, solid, grid)
    ok = solid[yy, xx]
    buckets = {}
    for a, b, y, x in zip(gy[ok].tolist(), gx[ok].tolist(), yy[ok].tolist(), xx[ok].tolist()):
        if bnd[a, b]:
            buckets.setdefault((a, b), []).append((y, x))
    colors = {}
    for key, pts in buckets.items():
        arr = np.array([frame[y, x, :3] for y, x in pts], float)
        order = arr.sum(1).argsort()
        colors[key] = arr[order[: max(1, len(order) // 3)]].mean(0)
    groups = []
    for key, c in sorted(colors.items(), key=lambda kv: kv[1].sum()):
        for g in groups:
            if np.abs(g[0] - c).sum() < 70:
                g[1].append(key)
                break
        else:
            groups.append([c, [key]])
    out = {}
    for rep, keys in groups:
        for k in keys:
            out[k] = (rep * darken).astype(np.uint8)
    return out


def mode_pixelize(frame, solid, grid, pal, ignore, thresh=0.42, darken=0.78, rare=0.30, outline=True):
    """칸마다 최빈색. ignore 픽셀(원본 외곽선 띠, 얼굴)은 표를 던지지 않는다. 드문 색은 가중."""
    w, h = grid.w, grid.h
    rgb = frame[..., :3].astype(np.int32)
    idx = nearest(rgb, pal)
    freq = np.bincount(idx[solid & ~ignore], minlength=len(pal)).astype(float) + 1
    weight = (freq.sum() / freq) ** rare
    gy, gx, yy, xx = accumulate(frame, solid, grid)
    n = np.zeros((h, w))
    np.add.at(n, (gy, gx), 1)
    cov = np.zeros((h, w))
    np.add.at(cov, (gy, gx), np.where(solid[yy, xx], frame[yy, xx, 3] / 255, 0.0))
    votes = np.zeros((h, w, len(pal)))
    v = solid[yy, xx] & ~ignore[yy, xx]
    np.add.at(votes, (gy[v], gx[v], idx[yy[v], xx[v]]), weight[idx[yy[v], xx[v]]])

    mask = silhouette(cov / np.maximum(n, 1), thresh)
    bnd = boundary_of(mask) if outline else np.zeros(mask.shape, bool)
    choice = votes.argmax(-1)
    has = votes.sum(-1) > 0
    # 표 없는 안쪽 칸은 이웃 색으로, 외톨이 얼룩은 지운다 (드문 색은 살린다)
    for _ in range(2):
        new = choice.copy()
        for y, x in zip(*np.nonzero(mask & ~bnd)):
            nb = [choice[j, i] for j, i in ((y - 1, x), (y + 1, x), (y, x - 1), (y, x + 1))
                  if 0 <= j < h and 0 <= i < w and mask[j, i] and not bnd[j, i] and has[j, i]]
            if not nb:
                continue
            top, cnt = Counter(nb).most_common(1)[0]
            if not has[y, x] or (choice[y, x] not in nb and cnt >= 3 and freq[choice[y, x]] > freq.sum() * 0.03):
                new[y, x] = top
                has[y, x] = True
        choice = new

    img = np.zeros((h, w, 4), np.uint8)
    img[mask, :3] = pal[choice[mask]].astype(np.uint8)
    img[mask, 3] = 255
    if outline:
        for (a, b), col in local_outline(frame, solid, grid, bnd, darken).items():
            img[a, b, :3] = col
    return img, mask, bnd


def stamp_z(img, gx, gy, size):
    z = ["###", "..#", ".#.", "###"]
    for j, row in enumerate(z):
        for i, ch in enumerate(row):
            if ch == "#" and 0 <= gy + j < size and 0 <= gx + i < size:
                img[gy + j, gx + i] = Z_PIX


def draw_fx(img, fx, frame, grid, size, anim):
    """몸에서 떨어진 효과. 잠잘 때의 zZ는 도트 z 도장, 그 밖에는 색만 옮긴다."""
    fx = [c for c in fx if len(c[0]) >= 6]
    if anim == "sleep":
        if fx:
            ys = np.concatenate([c[0] for c in fx])
            xs = np.concatenate([c[1] for c in fx])
            gx, gy = grid.to_cell(xs.mean(), ys.min())
            stamp_z(img, min(int(gx), size - 3), max(int(gy) - 1, 0), size)
        return
    yi, xi = grid.indices(frame.shape[:2])
    for ys, xs in fx:
        col = np.median(frame[ys, xs, :3], axis=0).astype(np.uint8)
        for y, x in zip(ys, xs):
            gy, gx = yi[y], xi[x]
            if 0 <= gy < size and 0 <= gx < size and img[gy, gx, 3] == 0:
                img[gy, gx, :3] = col
                img[gy, gx, 3] = 255


# ---------------------------------------------------------------- 접근법
def area_downscale(frame, grid):
    """알파 가중 면적 평균. cov = 칸의 알파 평균."""
    gy, gx, yy, xx = accumulate(frame, None, grid)
    a = frame[yy, xx, 3] / 255
    s = np.zeros((grid.h, grid.w, 3))
    c = np.zeros((grid.h, grid.w))
    n = np.zeros((grid.h, grid.w))
    np.add.at(s, (gy, gx), frame[yy, xx, :3] * a[:, None])
    np.add.at(c, (gy, gx), a)
    np.add.at(n, (gy, gx), 1)
    return s / np.maximum(c[..., None], 1e-6), c / np.maximum(n, 1)


def approach_a(pet, stage, anim, index, size, ref="HEAD"):
    frame = get_frame(pet, stage, anim, index, ref)
    rgb, cov = area_downscale(frame, Grid(FRAME / size, 0, 0, size))
    pal = species_palette(pet, ref)
    img = np.zeros((size, size, 4), np.uint8)
    keep = cov >= 0.5
    img[keep, :3] = pal[nearest(rgb[keep], pal)].astype(np.uint8)
    img[keep, 3] = 255
    return img


def approach_b(pet, stage, anim, index, size, ref="HEAD"):
    """면적 평균 색(탁함 있음) + 격자 맞춤 + 외곽선 + 눈·볼."""
    frame = get_frame(pet, stage, anim, index, ref)
    grid = Grid.for_sheet(pet, stage, size, ref)
    body, _fx = body_mask(frame)
    face = find_face(frame, body)
    masked = frame.copy()
    masked[~body] = 0
    rgb, cov = area_downscale(masked, grid)
    mask = silhouette(cov, 0.45)
    pal = species_palette(pet, ref)
    img = np.zeros((size, size, 4), np.uint8)
    img[mask, :3] = pal[nearest(rgb[mask], pal)].astype(np.uint8)
    img[mask, 3] = 255
    bnd = boundary_of(mask)
    img[bnd, :3] = (line_color(frame, body) * 0.82).astype(np.uint8)
    apply_face(img, mask, face, grid, size, bnd)
    return img


def split_zzz(frame, anim):
    """잘 때 zZ가 머리에 닿아 몸과 한 덩어리가 되면 도트에서 얼룩이 된다 — 몸에서 떼어낸다."""
    if anim != "sleep":
        return frame
    rgb = frame[..., :3].astype(np.int32)
    z = (np.abs(rgb - ZZZ_RGB).sum(-1) < 60) & (frame[..., 3] > 0)
    out = frame.copy()
    near = dilate(z, 2)
    out[near & (np.abs(rgb - ZZZ_RGB).sum(-1) < 240) & (rgb[..., 2] >= rgb[..., 0])] = 0
    return out


def find_face_pumpkin(frame, body):
    """호박은 얼굴이 빛나는 조각이라 색으로 따로 찾는다 (종별 손질 예시). 불이 꺼지면(‿) 조각 색의 호."""
    rgb = frame[..., :3].astype(np.int32)
    inner = erode(body, 3)
    lit = inner & ((np.abs(rgb - GLOW_Y_RGB).sum(-1) < 26) | (np.abs(rgb - GLOW_CORE_RGB).sum(-1) < 26))
    pieces = []
    for ys, xs in components(dilate(lit, 1) & inner):
        keep = lit[ys, xs]
        if keep.sum() >= 8:
            pieces.append((ys[keep], xs[keep]))
    carve_kind = None
    if len(pieces) < 2:
        carve = inner & (np.abs(rgb - CARVE_RGB).sum(-1) < 70)
        pieces = [c for c in components(dilate(carve, 1) & inner) if len(c[0]) >= 8]
        carve_kind = "closed"
    info = [{"cx": (x.min() + x.max() + 1) / 2, "cy": (y.min() + y.max() + 1) / 2, "w": int(x.max() - x.min() + 1),
             "h": int(y.max() - y.min() + 1), "ys": y, "xs": x} for y, x in pieces]
    info.sort(key=lambda c: c["cy"])
    eyes, mouth = [], None
    if info:
        top = info[0]["cy"]
        eyes = sorted([c for c in info if c["cy"] - top <= 6], key=lambda c: c["cx"])[:2]
        rest = [c for c in info if c["cy"] - top > 6]
        mouth = rest[-1] if rest else None
    pixels = np.zeros(body.shape, bool)
    for c in eyes + ([mouth] if mouth else []):
        c["kind"] = carve_kind or "lantern"
        pixels |= dilate(_box(body.shape, c), 2)
    return {"eyes": eyes, "blush": [], "pixels": pixels, "mouth": mouth, "lantern_mouth": mouth is not None and not carve_kind}


def _box(shape, c):
    m = np.zeros(shape, bool)
    m[int(c["ys"].min()):int(c["ys"].max()) + 1, int(c["xs"].min()):int(c["xs"].max()) + 1] = True
    return m


FACE_FINDERS = {"pumpkin": find_face_pumpkin}  # 종별 손질: 기본 눈 색이 아닌 종


def approach_c(pet, stage, anim, index, size, ref="HEAD"):
    frame = split_zzz(get_frame(pet, stage, anim, index, ref), anim)
    front = bool(load_manifest(pet, ref)["animations"][anim].get("front"))
    grid = Grid.for_sheet(pet, stage, size, ref)
    body, fx = body_mask(frame)
    face = FACE_FINDERS.get(pet, find_face)(frame, body)
    ignore = (body & ~erode(body, 3)) | face["pixels"] | thin_dark(frame, body, line_color(frame, body))
    pal = species_palette(pet, ref)
    img, mask, bnd = mode_pixelize(frame, body, grid, pal, ignore)
    apply_face(img, mask, face, grid, size, bnd, size // 2 if front else None)
    draw_fx(img, fx, frame, grid, size, anim)
    return img


APPROACHES = {"a": approach_a, "b": approach_b, "c": approach_c}


# ---------------------------------------------------------------- 펫이 아닌 그림 (집·밥그릇·손 음식·선물·물건)
def art_palette(frame, k=12):
    counts = Counter()
    px = frame[frame[..., 3] >= 200][:, :3].astype(np.int64) // 4
    keys, n = np.unique(px[:, 0] * 65536 + px[:, 1] * 256 + px[:, 2], return_counts=True)
    counts.update(dict(zip(keys.tolist(), n.tolist())))
    picks = []
    for key, _c in counts.most_common(3000):
        rgb = np.array([(key >> 16) * 4 + 2, ((key >> 8) & 255) * 4 + 2, (key & 255) * 4 + 2], float)
        if all(np.abs(rgb - p).sum() > 36 for p in picks):
            picks.append(rgb)
            if len(picks) == k:
                break
    return np.array(picks) if picks else np.zeros((1, 3))


def load_art(rel, ref="HEAD"):
    return np.array(Image.open(io.BytesIO(_git_show(ref, rel))).convert("RGBA"))


def pixelize_art(frame, dots_w, dots_h=None, fit=True, band=None, rare=0.30, k=12):
    """그림 전체를 dots_w×dots_h 칸에. fit=True면 그림 경계에 맞춰 키워 작은 격자를 꽉 채운다 (가운데·바닥 정렬).
    fit=False면 그림 칸을 그대로 나눈다 — 집처럼 문 좌표가 정해진 그림."""
    dots_h = dots_h or dots_w
    h, w = frame.shape[:2]
    solid = frame[..., 3] >= 60
    if fit and solid.any():
        ys, xs = np.nonzero(solid)
        bw, bh = xs.max() + 1 - xs.min(), ys.max() + 1 - ys.min()
        cell = max(bw / dots_w, bh / dots_h)
        grid = Grid(cell, (xs.min() + xs.max() + 1) / 2 - cell * dots_w / 2, ys.max() + 1 - cell * dots_h, dots_w, dots_h)
    else:
        cell = max(w / dots_w, h / dots_h)
        grid = Grid(cell, w / 2 - cell * dots_w / 2, h - cell * dots_h, dots_w, dots_h)
    band = band if band is not None else max(1, int(round(w / 128 * 3))) if w >= 96 else 1
    ignore = solid & ~erode(solid, band)
    img, _mask, _bnd = mode_pixelize(frame, solid, grid, art_palette(frame, k), ignore, thresh=0.40, rare=rare)
    return img


def over(back, front):
    out = Image.fromarray(back, "RGBA").copy()
    out.alpha_composite(Image.fromarray(front, "RGBA"))
    return np.array(out)


# ---------------------------------------------------------------- 미리보기
def font(px):
    for p in ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",):
        if Path(p).exists():
            return ImageFont.truetype(p, px)
    return ImageFont.load_default()


def up(arr, k):
    return Image.fromarray(arr, "RGBA").resize((arr.shape[1] * k, arr.shape[0] * k), Image.NEAREST)


def cell_image(arr, bg, side=FRAME):
    """표시 칸 하나. 도트가 균일하도록 정수배만 쓰고 가운데 둔다 (128/24 → 5배 = 120px)."""
    out = Image.new("RGBA", (side, side), bg)
    if arr.shape[0] >= side:
        out.alpha_composite(Image.fromarray(arr, "RGBA").resize((side, side), Image.LANCZOS) if arr.shape[0] != side
                            else Image.fromarray(arr, "RGBA"))
        return out
    k = side // max(arr.shape[:2])
    big = up(arr, k)
    out.alpha_composite(big, ((side - big.width) // 2, (side - big.height) // 2))
    return out


def species_block(pet, ref, variants):
    """종 하나: 행 = 원본·접근법, 열 = (아기 3동작, 어른 3동작) 밝은 배경 + 어두운 배경."""
    columns = [(st, an) for st in SHOW_STAGES for an in SHOW_ANIMS]
    rows = [("orig", None)] + variants
    label_w, head_h = 64, 44
    block = Image.new("RGBA", (label_w + FRAME * len(columns) * 2, head_h + FRAME * len(rows)), (255, 255, 255, 255))
    d = ImageDraw.Draw(block)
    d.text((6, 6), pet, fill=(30, 30, 40), font=font(20))
    for bi, bg in enumerate((LIGHT_BG, DARK_BG)):
        for ci, (st, an) in enumerate(columns):
            x = label_w + (bi * len(columns) + ci) * FRAME
            d.text((x + 6, 24), f"{st} {an}", fill=(70, 70, 90), font=font(13))
    for ri, (name, size) in enumerate(rows):
        y = head_h + ri * FRAME
        d.text((4, y + 54), name if size is None else f"{name}{size}", fill=(30, 30, 40), font=font(18))
        for bi, bg in enumerate((LIGHT_BG, DARK_BG)):
            for ci, (st, an) in enumerate(columns):
                x = label_w + (bi * len(columns) + ci) * FRAME
                arr = get_frame(pet, st, an, 0, ref) if size is None else APPROACHES[name](pet, st, an, 0, size, ref)
                block.alpha_composite(cell_image(arr, bg), (x, y))
    return block


# 펫이 화면 4px 도트(teenager 128px → 32칸)일 때, 화면 크기 ÷ 4 = 같은 밀도의 칸 수
HOUSES = ["house", "hive", "haunted", "xmas"]
ITEMS = [  # (제목, 경로, 화면 크기 w, h)
    ("bowl_food", "ui/effects/bowl_food.png", 56, 48), ("bowl_water", "ui/effects/bowl_water.png", 56, 48),
    ("flower", "ui/effects/flower.png", 56, 48),
    ("hand_apple", "ui/effects/hand_apple.png", 48, 48), ("hand_fish", "ui/effects/hand_fish.png", 48, 48),
    ("hand_honey", "ui/effects/hand_honey.png", 48, 48), ("hand_carrot", "ui/effects/hand_carrot.png", 48, 48),
    ("hand_milk", "ui/effects/hand_milk.png", 48, 48), ("hand_meal", "ui/effects/hand_meal.png", 48, 48),
    ("gift_acorn", "ui/gifts/bear_acorn.png", 40, 40), ("gift_comb", "ui/gifts/bee_honeycomb.png", 40, 40),
    ("gift_tulip", "ui/gifts/bee_tulip.png", 40, 40), ("gift_salmon", "ui/gifts/bear_salmon.png", 40, 40),
    ("acorn_shiny", "ui/gifts/bear_acorn_shiny.png", 40, 40),
    ("banana", "ui/objects/banana.png", 96, 96), ("can", "ui/objects/can.png", 96, 96),
    ("computer", "ui/objects/computer.png", 96, 96), ("sock", "ui/objects/sock.png", 96, 96),
    ("umbrella", "ui/objects/umbrella.png", 96, 96),
]
DOT = 4  # 화면 px / 도트 (teenager 기준)


def house_arrays(name, stage, dots, ref):
    back = load_art(f"ui/house/{name}_{stage}.png", ref)
    front = load_art(f"ui/house/{name}_{stage}_front.png", ref)
    comp = over(back, front)
    return comp, pixelize_art(comp, dots, fit=False, k=14)


def world_block(ref):
    """집·소품. 집: 원본 / 같은 밀도(teenager 32칸 = 4px). 소품: 원본 / 같은 밀도 / 한 단계 촘촘(3px 도트)."""
    W = 64 + 128 * 12
    sections = []

    # 집 4종 — 밝은·어두운 배경
    house_img = Image.new("RGBA", (W, 44 + 128 * 2), (255, 255, 255, 255))
    d = ImageDraw.Draw(house_img)
    d.text((6, 6), "houses (teenager, composite back+front): 32 dots = same 4px dot as the pet", fill=(30, 30, 40), font=font(20))
    for ri, label in enumerate(("orig", "dot4")):
        d.text((4, 44 + ri * 128 + 54), label, fill=(30, 30, 40), font=font(18))
    for ci, name in enumerate(HOUSES):
        comp, px = house_arrays(name, "teenager", 32, ref)
        for bi, bg in enumerate((LIGHT_BG, DARK_BG)):
            x = 64 + (bi * 4 + ci) * 128
            d.text((x + 6, 26), name, fill=(70, 70, 90), font=font(13))
            house_img.alpha_composite(cell_image(comp, bg), (x, 44))
            house_img.alpha_composite(cell_image(px, bg), (x, 44 + 128))
    sections.append(house_img)

    # 소품 — 2배로 키워 표시 (실제 크기 비교는 아래 장면에서)
    cols = 12
    rows_per = 3
    n_rows = (len(ITEMS) + cols - 1) // cols
    item_img = Image.new("RGBA", (W, (44 + 128 * rows_per) * n_rows), (255, 255, 255, 255))
    d = ImageDraw.Draw(item_img)
    for i, (name, rel, sw, sh) in enumerate(ITEMS):
        art = load_art(rel, ref)
        ox = 64 + (i % cols) * 128
        oy = (i // cols) * (44 + 128 * rows_per)
        fit = art.shape[1] < 96  # 물건(128 캔버스)은 여백까지 그대로 나눠 실제 크기를 지킨다
        dots4 = pixelize_art(art, max(4, sw // 4), max(4, sh // 4), fit=fit)
        dots3 = pixelize_art(art, max(4, round(sw / 3)), max(4, round(sh / 3)), fit=fit)
        d.text((ox + 4, oy + 26), f"{name} {sw // 4}x{sh // 4}", fill=(70, 70, 90), font=font(12))
        bg = LIGHT_BG if i % 2 == 0 else DARK_BG
        disp = Image.new("RGBA", (128, 128), bg)
        k = min(128 // art.shape[1], 128 // art.shape[0]) or 1
        scaled = Image.fromarray(art, "RGBA").resize((art.shape[1] * k, art.shape[0] * k), Image.LANCZOS)
        disp.alpha_composite(scaled, ((128 - scaled.width) // 2, (128 - scaled.height) // 2))
        item_img.alpha_composite(disp, (ox, oy + 44))
        item_img.alpha_composite(cell_image(dots4, bg), (ox, oy + 44 + 128))
        item_img.alpha_composite(cell_image(dots3, bg), (ox, oy + 44 + 256))
        if i % cols == 0:
            for ri, label in enumerate(("orig", "dot4", "dot3")):
                d.text((4, oy + 44 + ri * 128 + 54), label, fill=(30, 30, 40), font=font(18))
    sections.append(item_img)

    # 장면 — 실제 화면 크기(1:1): 원본 세계 vs 픽셀 세계
    scene_h = 170
    scene = Image.new("RGBA", (W, 44 + scene_h * 2), (255, 255, 255, 255))
    d = ImageDraw.Draw(scene)
    d.text((6, 6), "scene at real screen size (teenager): every dot = 4 screen px", fill=(30, 30, 40), font=font(20))
    half = (W - 64) // 2
    for ri, label in enumerate(("orig", "pixel")):
        d.text((4, 44 + ri * scene_h + 70), label, fill=(30, 30, 40), font=font(18))
        for bi, bg in enumerate((LIGHT_BG, DARK_BG)):
            x0, y0 = 64 + bi * half, 44 + ri * scene_h
            ImageDraw.Draw(scene).rectangle((x0, y0, x0 + half - 1, y0 + scene_h - 1), fill=bg)
            ground = y0 + scene_h - 8

            def place(arr, x, k=1):
                im = up(arr, k) if k > 1 else Image.fromarray(arr, "RGBA")
                scene.alpha_composite(im, (x0 + x, ground - im.height))

            comp, px = house_arrays("house", "teenager", 32, ref)
            cat = get_frame("cat", "teenager", "walk", 0, ref)
            items = [("ui/effects/bowl_food.png", 56, 48), ("ui/effects/flower.png", 56, 48),
                     ("ui/gifts/bear_acorn.png", 40, 40), ("ui/effects/hand_apple.png", 48, 48),
                     ("ui/objects/sock.png", 96, 96)]
            if ri == 0:
                place(comp, 6)
                place(cat, 134)
            else:
                place(px, 6, DOT)
                place(approach_c("cat", "teenager", "walk", 0, 32, ref), 134, DOT)
            x = 264
            for rel, sw, sh in items:
                art = load_art(rel, ref)
                if ri == 0:
                    if art.shape[1] != sw:
                        art = np.array(Image.fromarray(art, "RGBA").resize((sw, sh), Image.LANCZOS))
                    place(art, x)
                else:
                    place(pixelize_art(art, sw // DOT, sh // DOT, fit=art.shape[1] < 96), x, DOT)
                x += sw + 8
    sections.append(scene)

    total = sum(s.height for s in sections) + 24 * (len(sections) - 1)
    out = Image.new("RGBA", (W, total), (255, 255, 255, 255))
    y = 0
    for s in sections:
        out.alpha_composite(s, (0, y))
        y += s.height + 24
    return out


def make_preview(out, ref="HEAD", pets=SHOW_PETS, ncols=2, world=True):
    variants = [("a", 32), ("b", 32), ("c", 32), ("a", 24), ("b", 24), ("c", 24)]
    blocks = [species_block(p, ref, variants) for p in pets]
    if world:
        blocks.append(world_block(ref))
    gap = 24
    bw = max(b.width for b in blocks)
    rows = [blocks[i:i + ncols] for i in range(0, len(blocks), ncols)]
    heights = [max(b.height for b in r) for r in rows]
    sheet = Image.new("RGBA", (ncols * bw + (ncols - 1) * gap, sum(heights) + gap * (len(rows) - 1)), (255, 255, 255, 255))
    y = 0
    for r, h in zip(rows, heights):
        for i, b in enumerate(r):
            sheet.alpha_composite(b, (i * (bw + gap), y))
        y += h + gap
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    sheet.convert("RGB").save(out, optimize=True)
    return sheet.size


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--preview", action="store_true", help="비교 시트를 만든다")
    ap.add_argument("--pets", default=",".join(SHOW_PETS), help="--preview에 넣을 종 (쉼표)")
    ap.add_argument("--no-world", action="store_true", help="--preview에서 집·소품 묶음을 뺀다")
    ap.add_argument("--pet")
    ap.add_argument("--stage", default="adult", choices=STAGES)
    ap.add_argument("--anim", default="walk")
    ap.add_argument("--frame", type=int, default=0)
    ap.add_argument("--size", type=int, default=32)
    ap.add_argument("--approach", default="c", choices=sorted(APPROACHES))
    ap.add_argument("--ref", default="HEAD", help="시트를 읽을 git ref")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    if args.preview:
        out = args.out or ROOT / "tools/preview/pixel_proto.png"
        print(out, make_preview(out, args.ref, args.pets.split(","), world=not args.no_world))
        return
    if not args.pet:
        ap.error("--pet 또는 --preview가 필요합니다")
    arr = APPROACHES[args.approach](args.pet, args.stage, args.anim, args.frame, args.size, args.ref)
    Image.fromarray(arr, "RGBA").save(args.out or f"{args.pet}_{args.stage}_{args.anim}_{args.frame}.png")


if __name__ == "__main__":
    main()
