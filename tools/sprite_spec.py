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
]


# 특정 동물만 가진 동작 (Rust manifest::OPTIONAL_ANIMS) — 필수 동작 행 뒤에 이어 붙인다
OPTIONAL_ANIMS = [
    ("arch", 3, 6, True, False),
    ("wag", 4, 8, True, True),
]

# 말풍선 대사 (spec §2.6)
SPEECH = {
    "cat": {"chatter": ["meow!", "nya~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], "hiss": ["hiss!"]},
    "dog": {"chatter": ["woof!", "arf!"], "tickle": ["hehe!"], "grumpy": ["hmph!"]},
    "rabbit": {"chatter": ["boing!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"]},
    "beaver": {"chatter": ["chomp!", "hmm~"], "tickle": ["hehe!"], "grumpy": ["hmph!"]},
}
DEFAULT_SPEECH = {"chatter": ["♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"]}


def roll_spin(walk_speed):
    """구르기 속도에서 미끄러짐 없는 회전 속도 (deg/s)."""
    return round(walk_speed * ROLL_SPEED_MULT / ROLL_RADIUS * 180 / math.pi, 1)


def manifest(name, walk_speed, frame_counts, speech=None):
    """frame_counts: {동작: 프레임 수} — 선택 동작은 들어 있을 때만 포함.
    행 번호: 필수 ANIMS 순서, 그 뒤 frame_counts에 있는 OPTIONAL_ANIMS 순서. speech: 말풍선 대사 (없으면 DEFAULT_SPEECH)."""
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
    return {"name": name, "frame_size": FRAME, "walk_speed": walk_speed, "animations": animations,
            "speech": speech or DEFAULT_SPEECH}
