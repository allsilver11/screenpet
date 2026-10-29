"""앱 아이콘 원본 생성: python tools/make_icon.py → tools/icon.png
이후: npx tauri icon tools/icon.png (src-tauri/icons/ 생성)"""

from pathlib import Path

from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.ellipse([40, 40, S - 40, S - 40], fill=(255, 190, 90, 255))
paw = (120, 70, 40, 255)
d.ellipse([312, 470, 712, 840], fill=paw)
for cx, cy in [(300, 380), (430, 270), (594, 270), (724, 380)]:
    d.ellipse([cx - 80, cy - 95, cx + 80, cy + 95], fill=paw)
img.save(Path(__file__).resolve().parent / "icon.png")
