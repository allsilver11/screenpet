"""배포 파일을 만들 때만 (CI, 빌드 직전): ui/pets의 PNG를 무손실 WebP로 바꾼다. 저장소에는 PNG 원본이 그대로 남는다.
python tools/pack_webp.py [ui 폴더] — 그 자리에서 바꾸므로 체크아웃을 커밋하지 말 것."""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
MARK = 'const PET_ART_EXT = "png";'


def pack(ui):
    ui = Path(ui)
    common = ui / "common.js"
    text = common.read_text(encoding="utf-8")
    if text.count(MARK) != 1:
        raise SystemExit(f"{common}: expected exactly one {MARK!r}")
    before = after = count = 0
    for png in sorted((ui / "pets").glob("*/*.png")):
        webp = png.with_suffix(".webp")
        with Image.open(png) as im:
            im.convert("RGBA").save(webp, lossless=True, method=6, exact=True)
        before += png.stat().st_size
        after += webp.stat().st_size
        count += 1
        png.unlink()
    if count == 0:
        raise SystemExit(f"{ui / 'pets'}: no PNG to pack")
    common.write_text(text.replace(MARK, 'const PET_ART_EXT = "webp";'), encoding="utf-8", newline="")
    print(f"packed {count} files: {before / 1e6:.1f} MB -> {after / 1e6:.1f} MB")
    return count, before, after


if __name__ == "__main__":
    pack(sys.argv[1] if len(sys.argv) > 1 else ROOT / "ui")
