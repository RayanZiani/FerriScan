"""Draws the FerriScan icon (a treemap in a rounded square) into apps/desktop/assets/icon.ico.

Run: python scripts/generate-icon.py   (needs Pillow; the .ico is committed, this is only the source)
"""
from PIL import Image, ImageDraw

SIZE = 256
img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle((8, 8, SIZE - 8, SIZE - 8), radius=44, fill=(15, 17, 21, 255))
pad, gap = 36, 8
x0, y0, x1, y1 = pad, pad, SIZE - pad, SIZE - pad
mid_x = x0 + (x1 - x0) * 0.58
mid_y = y0 + (y1 - y0) * 0.55
tiles = [
    ((x0, y0, mid_x - gap / 2, y1), (91, 140, 255)),
    ((mid_x + gap / 2, y0, x1, mid_y - gap / 2), (251, 146, 60)),
    ((mid_x + gap / 2, mid_y + gap / 2, x1 - (x1 - mid_x) * 0.45, y1), (74, 222, 128)),
    ((x1 - (x1 - mid_x) * 0.45 + gap, mid_y + gap / 2, x1, y1), (167, 139, 250)),
]
for box, color in tiles:
    d.rounded_rectangle(box, radius=10, fill=color + (255,))
img.save(
    "apps/desktop/assets/icon.ico",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
