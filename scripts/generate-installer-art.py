"""Draws the installer artwork into installer/assets (committed; this is only the source).

Run: python scripts/generate-installer-art.py   (needs Pillow)
Inno Setup: wizard-large.bmp (164x314), wizard-small.bmp (55x55).
WiX: banner.bmp (493x58), dialog.bmp (493x312); license.rtf is made from LICENSE.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

OUT = Path("installer/assets")
OUT.mkdir(parents=True, exist_ok=True)
NAVY = (15, 17, 21)
PANEL = (21, 25, 34)
TILES = [(91, 140, 255), (251, 146, 60), (74, 222, 128), (167, 139, 250), (244, 114, 182), (45, 212, 191)]
BACKSLASH = chr(92)


def font(size, bold=False):
    name = "segoeuib.ttf" if bold else "segoeui.ttf"
    try:
        return ImageFont.truetype(f"C:/Windows/Fonts/{name}", size)
    except OSError:
        return ImageFont.load_default()


def treemap(draw, box, seed):
    """A small deterministic mosaic inside box."""
    x0, y0, x1, y1 = box
    gap = 4
    w, h = x1 - x0, y1 - y0
    cut = int(w * (0.5 + 0.08 * (seed % 3)))
    mid = int(h * 0.58)
    rects = [
        (x0, y0, x0 + cut - gap, y1),
        (x0 + cut, y0, x1, y0 + mid - gap),
        (x0 + cut, y0 + mid, x0 + cut + int((x1 - x0 - cut) * 0.55) - gap, y1),
        (x0 + cut + int((x1 - x0 - cut) * 0.55), y0 + mid, x1, y1),
    ]
    for i, r in enumerate(rects):
        draw.rounded_rectangle(r, radius=5, fill=TILES[(i + seed) % len(TILES)])


def vertical_gradient(size, top, bottom):
    img = Image.new("RGB", size, top)
    px = img.load()
    for y in range(size[1]):
        t = y / max(1, size[1] - 1)
        row = tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
        for x in range(size[0]):
            px[x, y] = row
    return img


def side_panel(img, seed):
    d = ImageDraw.Draw(img)
    treemap(d, (16, 24, 148, 150), seed)
    d.text((16, 176), "FerriScan", font=font(24, True), fill=(232, 235, 240))
    d.text((16, 210), "Fast, private\ndisk space\nanalysis", font=font(13), fill=(154, 164, 180), spacing=4)


# Inno Setup large side image
img = vertical_gradient((164, 314), NAVY, PANEL)
side_panel(img, 0)
img.save(OUT / "wizard-large.bmp")

# Inno Setup small header image
img = Image.new("RGB", (55, 55), NAVY)
treemap(ImageDraw.Draw(img), (8, 8, 47, 47), 1)
img.save(OUT / "wizard-small.bmp")

# WiX banner (top strip of the inner dialogs)
img = Image.new("RGB", (493, 58), (255, 255, 255))
treemap(ImageDraw.Draw(img), (440, 8, 485, 50), 2)
img.save(OUT / "banner.bmp")

# WiX welcome / exit dialog background (left panel 164 px wide, rest white)
img = Image.new("RGB", (493, 312), (255, 255, 255))
panel = vertical_gradient((164, 312), NAVY, PANEL)
side_panel(panel, 3)
img.paste(panel, (0, 0))
img.save(OUT / "dialog.bmp")

# WiX needs the license as RTF: escape the three RTF control characters, one paragraph per line.
text = Path("LICENSE").read_text(encoding="utf-8").replace("\r\n", "\n")
for ch in (BACKSLASH, "{", "}"):
    text = text.replace(ch, BACKSLASH + ch)
body = (BACKSLASH + "par\n").join(text.split("\n"))
header = BACKSLASH.join(["{", "rtf1", "ansi", "deff0{", "fonttbl{", "f0 Segoe UI;}}", "f0", "fs20 "])
Path(OUT / "license.rtf").write_text(header + body + "}", encoding="ascii", errors="replace")
print("installer art written to", OUT)
