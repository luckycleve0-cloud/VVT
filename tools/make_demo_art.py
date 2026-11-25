import os
from PIL import Image, ImageDraw

out = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "demo-art"))
os.makedirs(out, exist_ok=True)

# --- карта: тёмный данж с комнатами и сеткой ---
W, H = 1400, 1050
img = Image.new("RGB", (W, H), (44, 40, 38))
d = ImageDraw.Draw(img)

# пол
d.rectangle([60, 60, W - 60, H - 60], fill=(78, 70, 60))
# комнаты
rooms = [
    (90, 90, 430, 380, (96, 84, 68)),
    (470, 90, 900, 320, (88, 80, 70)),
    (940, 90, 1310, 500, (92, 76, 64)),
    (90, 420, 520, 760, (84, 78, 66)),
    (560, 360, 900, 760, (100, 88, 70)),
    (940, 540, 1310, 960, (86, 74, 62)),
    (90, 800, 520, 960, (92, 82, 68)),
    (560, 800, 900, 960, (88, 80, 70)),
]
for r in rooms:
    d.rectangle(r[:4], fill=r[4], outline=(40, 36, 32), width=6)
    # пол внутри чуть темнее
    d.rectangle([r[0] + 14, r[1] + 14, r[2] - 14, r[3] - 14], outline=(60, 54, 46), width=3)

# колонны/декор
for cx, cy in [(300, 230), (700, 560), (1100, 700), (300, 880), (760, 180)]:
    d.ellipse([cx - 26, cy - 26, cx + 26, cy + 26], fill=(58, 52, 46), outline=(30, 28, 26), width=4)

# сетка 70 px
for x in range(0, W, 70):
    d.line([(x, 0), (x, H)], fill=(70, 64, 56), width=1)
for y in range(0, H, 70):
    d.line([(0, y), (W, y)], fill=(70, 64, 56), width=1)

img.save(os.path.join(out, "map.jpg"), quality=88)

# --- токены: «портреты» ---
palettes = [
    ((212, 180, 140), (120, 70, 40), (60, 30, 20)),   # гоблин
    ((170, 200, 230), (80, 110, 150), (30, 40, 60)),  # агент
    ((230, 190, 170), (140, 90, 80), (70, 40, 40)),   # алиса
]
for index, (bg, mid, dark) in enumerate(palettes, start=1):
    size = 256
    t = Image.new("RGB", (size, size), bg)
    td = ImageDraw.Draw(t)
    # плечи и голова — чтобы круглая обрезка была видна
    td.ellipse([48, 40, 208, 200], fill=mid)
    td.ellipse([88, 70, 168, 150], fill=(235, 215, 195))
    td.ellipse([104, 96, 118, 110], fill=dark)
    td.ellipse([138, 96, 152, 110], fill=dark)
    td.arc([112, 118, 144, 142], start=20, end=160, fill=dark, width=4)
    td.rectangle([30, 190, 226, 256], fill=dark)
    t.save(os.path.join(out, f"token{index}.jpg"), quality=90)

print("saved to", out)
