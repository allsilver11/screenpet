"""스프라이트 시트 규격 — 앱(src/manifest.rs REQUIRED_ANIMS)과 같은 순서를 유지한다."""

FRAME = 64

# (동작, 기본 프레임 수, fps, 반복)
ANIMS = [
    ("walk", 4, 8, True),
    ("idle", 2, 2, True),
    ("look", 4, 4, True),
    ("sleep", 2, 1, True),
    ("hop", 3, 10, False),
    ("fall", 2, 8, True),
    ("dragged", 2, 4, True),
    ("land", 2, 10, False),
]


def manifest(name, walk_speed, frame_counts):
    """frame_counts: {동작: 프레임 수}. 행 번호는 ANIMS 순서."""
    animations = {}
    for row, (anim, _, fps, loop) in enumerate(ANIMS):
        spec = {"row": row, "frames": frame_counts[anim], "fps": fps}
        if not loop:
            spec["loop"] = False
        animations[anim] = spec
    return {"name": name, "frame_size": FRAME, "walk_speed": walk_speed, "animations": animations}
