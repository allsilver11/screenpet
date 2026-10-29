"""스프라이트 시트 규격 — 앱(src/manifest.rs REQUIRED_ANIMS)과 같은 순서를 유지한다."""

import math

FRAME = 128
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

# 성장 단계 (Rust care::Stage) — 종마다 sheet_<stage>.png, sheet.png는 teenager와 같은 파일
STAGES = ["baby", "kinder", "child", "teenager", "adult"]
# 화면 표시 배율 (Rust Stage::scale)
STAGE_SCALE = {"baby": 0.45, "kinder": 0.60, "child": 0.78, "teenager": 1.00, "adult": 1.15}


# 특정 동물만 가진 동작 (Rust manifest::OPTIONAL_ANIMS) — 필수 동작 행 뒤에 이어 붙인다
OPTIONAL_ANIMS = [
    ("arch", 3, 6, True, False),
    ("wag", 4, 8, True, True),
    ("huff", 3, 4, True, True),
    # 뒷모습 (고양이·곰): front=False지만 앱은 반전하지 않는다 (Rust Task 4b에서 반전 안 함으로 처리)
    ("climb", 4, 6, True, False),
    ("slide", 2, 8, True, False),
    # 꿀벌: 콕! 침 쏘기 (앞모습, 한 번)
    ("sting", 3, 8, False, True),
]

# 말풍선 대사 (spec §2.6) — 먹이·성장 대사(hungry/full/level_up)는 모두 같다
_CARE_SPEECH = {"hungry": ["hungry...", "food?"], "full": ["full~"], "level_up": ["yay!", "♪♪"]}
SPEECH = {
    "cat": {"chatter": ["meow!", "nya~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], "hiss": ["hiss!"], "ignore": ["..."],
            **_CARE_SPEECH},
    "dog": {"chatter": ["woof!", "arf!"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "rabbit": {"chatter": ["boing!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "bear": {"chatter": ["grr~", "hmm~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "bee": {"chatter": ["bzz~", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], "sting": ["콕!"], **_CARE_SPEECH},
}
DEFAULT_SPEECH = {"chatter": ["♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH}

# 성격 (pet.json personality) — 없는 동물은 적지 않는다
PERSONALITY = {"cat": "aloof", "bee": "friendly"}

# 움직임 (pet.json locomotion) — 나는 동물만 적는다 (없으면 걷는다)
LOCOMOTION = {"bee": "fly"}

# 좋아하는 음식 (pet.json favorite_food)
FAVORITE_FOOD = {
    "cat": {"emoji": "🐟", "name": "생선"},
    "dog": {"emoji": "🦴", "name": "뼈다귀"},
    "rabbit": {"emoji": "🥕", "name": "당근"},
    "bear": {"emoji": "🍯", "name": "꿀"},
    "bee": {"emoji": "🌸", "name": "꽃꿀"},
}


def roll_spin(walk_speed):
    """구르기 속도에서 미끄러짐 없는 회전 속도 (deg/s)."""
    return round(walk_speed * ROLL_SPEED_MULT / ROLL_RADIUS * 180 / math.pi, 1)


def manifest(name, walk_speed, frame_counts, speech=None, favorite_food=None, stages=None, stage_size=None):
    """frame_counts: {동작: 프레임 수} — 선택 동작은 들어 있을 때만 포함.
    행 번호: 필수 ANIMS 순서, 그 뒤 frame_counts에 있는 OPTIONAL_ANIMS 순서. speech: 말풍선 대사 (없으면 DEFAULT_SPEECH).
    favorite_food: {"emoji", "name"} · stages: sheet_<stage>.png가 있는 단계 목록 ·
    stage_size: {단계: 종 배율} — 공통 단계 배율(STAGE_SCALE)에 곱한다 (셋 다 있을 때만 적는다)."""
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
    m = {"name": name, "frame_size": FRAME, "walk_speed": walk_speed, "animations": animations,
         "speech": speech or DEFAULT_SPEECH}
    if favorite_food:
        m["favorite_food"] = favorite_food
    if stages:
        m["stages"] = list(stages)
    if stage_size:
        m["stage_size"] = {st: stage_size[st] for st in (stages or STAGES)}
    return m
