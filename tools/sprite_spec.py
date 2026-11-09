"""스프라이트 시트 규격 — 앱(src/manifest.rs REQUIRED_ANIMS)과 같은 순서를 유지한다."""

import math

FRAME = 128  # 그림 좌표의 한 변 (draw_*는 모두 이 좌표로 그린다). 시트 한 칸의 실제 px는 frame_px
# roll 공: 이 점을 축으로 웹뷰가 회전시킨다 (ui/style.css transform-origin 50% 68.75%)
ROLL_CENTER = (64, 88)
ROLL_RADIUS = 38
ROLL_SPEED_MULT = 2.0  # brain behavior Move::Roll.speed_mult()

# (동작, 기본 프레임 수, fps, 반복, 앞모습)
ANIMS = [
    ("walk", 4, 8, True, False),
    ("run", 4, 12, True, False),
    ("roll", 2, 4, True, False),
    ("idle", 2, 2, True, False),
    ("look", 4, 3, True, False),
    ("sleep", 2, 1, True, False),
    ("hop", 3, 10, False, False),
    ("fall", 2, 8, True, False),
    ("dragged", 2, 4, True, False),
    ("land", 2, 10, False, False),
    ("sit_front", 4, 4, True, True),
    ("wave", 4, 6, True, True),
    ("tilt", 3, 3, True, True),
    ("belly", 4, 4, True, True),
    ("stretch", 3, 3, False, True),
    ("heart", 3, 4, True, True),
    ("tickle", 3, 8, True, True),
    ("annoyed", 2, 2, True, False),
    ("hungry", 3, 3, True, True),
    ("eat", 4, 6, True, True),
    ("level_up", 3, 6, False, True),
]

# 성장 단계 (Rust care::Stage) — 종마다 sheet_<stage>.png
STAGES = ["baby", "kinder", "child", "teenager", "adult"]
# 화면 표시 배율 (Rust Stage::scale)
STAGE_SCALE = {"baby": 0.45, "kinder": 0.60, "child": 0.78, "teenager": 1.00, "adult": 1.15}
# 청소년 때 이미 128px보다 크게 보이는 동물 (pet.json stage_size >= 1) — 어른과 함께 2배 시트
HIRES_TEENAGER = {"santa", "dino_green", "dino_purple", "dino_yellow", "giraffe"}


def sheet_128(path):
    """시험용: 시트(가로 4칸)를 128 칸으로 읽는다 — 그림 좌표로 재는 검사는 칸 크기와 상관없다. 2배 칸은 2×2 평균이라 1px 여백도 그대로."""
    from PIL import Image
    im = Image.open(path)
    return im.reduce(im.width // (4 * FRAME)) if im.width > 4 * FRAME else im


def frame_px(pid, stage):
    """동글 시트 한 칸의 px: 크게 보이는 단계는 벡터에서 2배로 다시 그린다 (화면 크기는 그대로 — 앱은 칸을 표시 크기에 맞춘다).
    pet.json frame_sizes에 적히고, head·neck·neck_stretch 표도 이 칸의 px로 적는다."""
    return FRAME * 2 if stage == "adult" or (stage == "teenager" and pid in HIRES_TEENAGER) else FRAME


# 특정 동물만 가진 동작 (Rust manifest::OPTIONAL_ANIMS) — 필수 동작 행 뒤에 이어 붙인다
OPTIONAL_ANIMS = [
    ("arch", 3, 6, True, False),
    ("huff", 3, 4, True, True),
    # 뒷모습 (고양이·산타): front=False지만 앱은 반전하지 않는다 (Rust Task 4b에서 반전 안 함으로 처리)
    ("climb", 4, 6, True, False),
    ("slide", 2, 8, True, False),
    # 개구리: 크게 뛰기 (옆모습, 한 번) — 작은 폴짝은 필수 동작 hop
    ("bigjump", 4, 8, False, False),
    # 기린: 구르는 대신 목 늘이기 (옆모습, 어? · 깜빡 · ✧) — 늘어나는 목은 앱이 층(neck_<stage>.png)으로 그린다
    ("neckstretch", 3, 2, True, False),
    # 모든 동물: 위를 올려다보기 (옆모습, 고개 들기 · 쭉 · ✧) — 머리 위에 든 음식, 가끔 쉬다가·걷다가, 커서가 머리 위에 머물 때
    ("lookup", 3, 3, True, False),
    # 공룡: 불 뿜기 (옆모습, 흡 · 퐁 · 화르륵 · 흐뭇 — 한 번) · 크앙 (옆모습, 고개 들고 소리 물결) · 제자리 쿵쾅쿵쾅 (옆모습, 먼지 퐁퐁)
    ("fire", 4, 4, False, False),
    ("roar", 3, 4, True, False),
    ("stomp", 4, 6, True, False),
    # 레서판다: 깜짝! 뒷발로 벌떡 서서 두 팔 번쩍 (앞모습, 한 번 — 마지막 칸으로 잠깐 굳는다)
    ("surprise", 3, 8, False, True),
]

# 말풍선 대사 (spec §2.6) — 먹이·성장 대사(hungry/full/level_up)는 모두 같다
_CARE_SPEECH = {"hungry": ["hungry...", "food?"], "full": ["full~"], "level_up": ["I grew up!!"]}
# 공룡 셋 (불 뿜기·크앙·쿵쾅 말풍선 roar/fire/stomp)
_DINO_SPEECH = {"chatter": ["rawr!", "♪"], "tickle": ["hehe!"], "grumpy": ["grrr~"], "roar": ["RAWR!", "grrr~"], "fire": ["fwoosh~"],
                "stomp": ["stomp!"], **_CARE_SPEECH}
SPEECH = {
    "cat": {"chatter": ["meow!", "nya~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], "hiss": ["hiss!"], "ignore": ["..."],
            **_CARE_SPEECH},
    "squirrel": {"chatter": ["chk chk!", "nuts!", "zoom!"], "tickle": ["hehe!"], "grumpy": ["chk-chk!"], **_CARE_SPEECH},
    "persian": {"chatter": ["mrr...", "*yawn*"], "tickle": ["...mrr~"], "grumpy": ["hmph."], "ignore": ["...", "zzz..."], **_CARE_SPEECH},
    "rabbit": {"chatter": ["boing!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "frog": {"chatter": ["ribbit!", "hop hop!", "ribbit~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "pumpkin": {"chatter": ["trick or treat!", "boo~", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "ghost": {"chatter": ["boo!", "ooo~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "duck": {"chatter": ["quack!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "giraffe": {"chatter": ["hmm~", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "rudolph": {"chatter": ["jingle~", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "santa": {"chatter": ["ho ho ho!", "merry~"], "tickle": ["ho ho!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "snowman": {"chatter": ["brrr~", "snow!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "bird": {"chatter": ["chirp!", "whoosh!", "tweet~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "penguin": {"chatter": ["waddle~", "wheee!", "pip pip!"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "redpanda": {"chatter": ["kyu kyu~", "peekaboo!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], "surprise": ["!!", "eep!"],
                 **_CARE_SPEECH},
    **{pid: dict(_DINO_SPEECH) for pid in ("dino_yellow", "dino_green", "dino_purple")},
}
DEFAULT_SPEECH = {"chatter": ["♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH}

# 성격 (pet.json personality) — 없는 동물은 적지 않는다
PERSONALITY = {"cat": "aloof", "persian": "aloof", "squirrel": "playful", "frog": "playful", "pumpkin": "playful", "ghost": "friendly",
               "duck": "playful", "giraffe": "friendly", "bird": "playful", "penguin": "playful", "redpanda": "playful",
               "rudolph": "friendly", "santa": "friendly", "snowman": "playful",
               "dino_yellow": "playful", "dino_green": "friendly", "dino_purple": "friendly"}

# 움직임 (pet.json locomotion) — 나는 동물만 적는다 (없으면 걷는다)
LOCOMOTION = {"frog": "hop", "ghost": "fly", "bird": "fly"}

# 빠른 나는 동물 (pet.json swift) — 더 자주 내려앉아 쉬고, 친구 화면으로는 "whoosh!" 쌩 (코어 PetConfig::swift)
SWIFT = {"bird"}

# 사는 집 그림 (pet.json house) — 보통 집은 적지 않는다. 앱 ui/house/<house>_<stage>.png
HOUSE = {"pumpkin": "haunted", "ghost": "haunted", "rudolph": "xmas", "santa": "xmas", "snowman": "xmas"}

# 걷기·달리기 토막 중 깡총거리는 몫 (pet.json hop_share) — 걷는 동물 중 토끼 절반, 호박은 늘 통통 (개구리는 locomotion hop이라 늘 폴짝)
HOP_SHARE = {"rabbit": 0.5, "squirrel": 0.35, "pumpkin": 1.0, "rudolph": 0.3, "snowman": 0.5}

# roll을 돌리지 않는 동물 (pet.json roll에 spin 없음) — 산타는 구르는 대신 썰매를 타고, 눈사람은 배로 미끄러지고, 기린은 목을 늘이고, 펭귄은 배로 쭉 미끄러진다
NO_SPIN = {"santa", "snowman", "giraffe", "penguin"}

# 가끔 머리 위에 작은 눈구름을 부르는 동물 (pet.json snow_cloud)
SNOW_CLOUD = {"snowman"}

# 좋아하는 음식 (pet.json favorite_food)
FAVORITE_FOOD = {
    "cat": {"emoji": "🐟", "name": "생선"},
    "persian": {"emoji": "🥛", "name": "우유"},
    "squirrel": {"emoji": "🌰", "name": "도토리"},
    "rabbit": {"emoji": "🥕", "name": "당근"},
    "frog": {"emoji": "🪰", "name": "파리"},
    "pumpkin": {"emoji": "🍬", "name": "사탕"},
    "ghost": {"emoji": "🍭", "name": "막대사탕"},
    "duck": {"emoji": "🍞", "name": "빵"},
    "giraffe": {"emoji": "🍃", "name": "나뭇잎"},
    "rudolph": {"emoji": "🍎", "name": "사과"},
    "santa": {"emoji": "🥛", "name": "우유"},
    "snowman": {"emoji": "🍧", "name": "빙수"},
    "dino_yellow": {"emoji": "🍉", "name": "수박"},
    "dino_green": {"emoji": "🥦", "name": "브로콜리"},
    "dino_purple": {"emoji": "🍇", "name": "포도"},
    "bird": {"emoji": "🌾", "name": "모이"},
    "penguin": {"emoji": "🦐", "name": "새우"},
    "redpanda": {"emoji": "🎋", "name": "대나무잎"},
}


def roll_spin(walk_speed):
    """구르기 속도에서 미끄러짐 없는 회전 속도 (deg/s)."""
    return round(walk_speed * ROLL_SPEED_MULT / ROLL_RADIUS * 180 / math.pi, 1)


def manifest(name, walk_speed, frame_counts, speech=None, favorite_food=None, stages=None, stage_size=None, frame_sizes=None):
    """frame_counts: {동작: 프레임 수} — 선택 동작은 들어 있을 때만 포함.
    행 번호: 필수 ANIMS 순서, 그 뒤 frame_counts에 있는 OPTIONAL_ANIMS 순서. speech: 말풍선 대사 (없으면 DEFAULT_SPEECH).
    favorite_food: {"emoji", "name"} · stages: sheet_<stage>.png가 있는 단계 목록 ·
    stage_size: {단계: 종 배율} — 공통 단계 배율(STAGE_SCALE)에 곱한다 (셋 다 있을 때만 적는다) ·
    frame_sizes: {단계: 칸 px} — FRAME과 다른 단계만 (없는 단계는 frame_size)."""
    animations = {}
    rows = list(ANIMS) + [o for o in OPTIONAL_ANIMS if o[0] in frame_counts]
    for row, (anim, _, fps, loop, front) in enumerate(rows):
        spec = {"row": row, "frames": frame_counts[anim], "fps": fps}
        if not loop:
            spec["loop"] = False
        if front:
            spec["front"] = True
        if anim == "roll":
            spec["spin"] = roll_spin(walk_speed)
        animations[anim] = spec
    m = {"name": name, "frame_size": FRAME}
    if frame_sizes:
        m["frame_sizes"] = frame_sizes
    m.update({"walk_speed": walk_speed, "animations": animations, "speech": speech or DEFAULT_SPEECH})
    if favorite_food:
        m["favorite_food"] = favorite_food
    if stages:
        m["stages"] = list(stages)
    if stage_size:
        m["stage_size"] = {st: stage_size[st] for st in (stages or STAGES)}
    return m
