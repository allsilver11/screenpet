"""AI로 만든 동작별 프레임 PNG들을 앱 규격 시트로 합친다.

사용법:
    python tools/pack_sheet.py <프레임폴더> ui/pets/<id> --name 고양이 --walk-speed 60

프레임 폴더에는 `<동작>_<번호>.png` (번호는 0부터) 파일이 있어야 한다.
동작별 프레임 수는 sprite_spec.ANIMS의 기본값보다 많아도 된다(연속된 번호만큼 읽음).
모든 프레임은 투명 배경이어야 하며, 공통 배율로 축소돼 128x128 안에 바닥·가운데 정렬된다.
"""

import argparse
import json
import re
from pathlib import Path

from PIL import Image

from sprite_spec import ANIMS, FRAME, manifest


def _load_frames(frames_dir):
    frames = {}
    for anim, default_n, *_ in ANIMS:
        imgs = []
        i = 0
        while (frames_dir / f"{anim}_{i}.png").exists():
            path = frames_dir / f"{anim}_{i}.png"
            img = Image.open(path).convert("RGBA")
            if img.getchannel("A").getbbox() is None:
                raise ValueError(f"{path} 가 완전히 투명합니다 (빈 프레임)")
            imgs.append(img)
            i += 1
        # 첫 빠진 번호 뒤에 남은 번호가 있으면 조용히 버리지 않고 오류
        pattern = re.compile(rf"^{re.escape(anim)}_(\d+)\.png$")
        present = [int(m.group(1)) for p in frames_dir.iterdir() if (m := pattern.match(p.name))]
        gaps = sorted(n for n in present if n > i)
        if gaps:
            raise ValueError(
                f"{frames_dir / f'{anim}_{i}.png'} 가 없습니다: {anim}_{gaps[0]}.png 가 있어 번호가 빠졌습니다 (0부터 빠짐없이)"
            )
        if not imgs:
            raise FileNotFoundError(f"{frames_dir / f'{anim}_0.png'} 가 없습니다 (기본 {default_n}장 권장)")
        sizes = {img.size for img in imgs}
        if len(sizes) > 1:
            raise ValueError(f"{anim} 동작의 프레임 캔버스 크기가 서로 다릅니다: {sorted(sizes)}")
        frames[anim] = imgs
    return frames


def pack(frames_dir, out_dir, name, walk_speed):
    frames_dir, out_dir = Path(frames_dir), Path(out_dir)
    frames = _load_frames(frames_dir)
    # 동작 안에서는 모든 프레임에 같은 자르기 박스(합집합)를 써서 움직임을 보존한다
    cropped = {}
    for a, imgs in frames.items():
        boxes = [img.getchannel("A").getbbox() for img in imgs]
        union = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
        cropped[a] = [img.crop(union) for img in imgs]

    # 모든 프레임에 같은 배율 → 프레임마다 크기가 들쭉날쭉하지 않음
    biggest = max(max(c.width, c.height) for cs in cropped.values() for c in cs)
    scale = FRAME / biggest

    cols = max(len(cs) for cs in cropped.values())
    sheet = Image.new("RGBA", (FRAME * cols, FRAME * len(ANIMS)), (0, 0, 0, 0))
    for row, (anim, *_) in enumerate(ANIMS):
        for col, c in enumerate(cropped[anim]):
            w, h = max(1, round(c.width * scale)), max(1, round(c.height * scale))
            small = c.resize((w, h), Image.LANCZOS)
            sheet.paste(small, (col * FRAME + (FRAME - w) // 2, row * FRAME + FRAME - h), small)

    out_dir.mkdir(parents=True, exist_ok=True)
    sheet.save(out_dir / "sheet.png")
    counts = {a: len(cs) for a, cs in cropped.items()}
    (out_dir / "pet.json").write_text(
        json.dumps(manifest(name, walk_speed, counts), ensure_ascii=False, indent=2), encoding="utf-8"
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("frames_dir")
    ap.add_argument("out_dir")
    ap.add_argument("--name", required=True)
    ap.add_argument("--walk-speed", type=float, default=60)
    args = ap.parse_args()
    pack(args.frames_dir, args.out_dir, args.name, args.walk_speed)


if __name__ == "__main__":
    main()
