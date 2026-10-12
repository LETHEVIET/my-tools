"""Regenerate Windows and iPhone app icons from the SVG-rendered master PNG.

Requires Pillow only when regenerating assets. The released Windows app
embeds desktop/assets/my-tools.ico and needs no Python at runtime/build time.

To refresh the master from its SVG on Windows, open desktop/assets/my-tools.svg
in a browser and save at 1024 x 1024, or use headless Edge:

  msedge --headless --window-size=1024,1024 --screenshot=desktop/assets/my-tools.png file:///C:/.../desktop/assets/my-tools.svg
"""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "desktop" / "assets"

source = Image.open(ASSETS / "my-tools.png").convert("RGBA")
if source.size != (1024, 1024):
    raise SystemExit(f"Expected 1024x1024 SVG screenshot, got {source.size}")

# The SVG has a comfortably inset rounded square. Crop and mask its frame
# so Windows taskbar/tray and iPhone icons show a real transparent silhouette.
image = source.crop((104, 104, 920, 920)).resize((1024, 1024), Image.Resampling.LANCZOS)
mask = Image.new("L", (1024, 1024))
draw = ImageDraw.Draw(mask)
draw.rounded_rectangle((0, 0, 1023, 1023), radius=218, fill=255)
image.putalpha(mask)

image.save(ASSETS / "my-tools-1024.png", optimize=True)
image.save(
    ASSETS / "my-tools.ico",
    format="ICO",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)

for size in (192, 512):
    image.resize((size, size), Image.Resampling.LANCZOS).save(
        ROOT / "web" / "icons" / f"icon-{size}.png", optimize=True
    )
print("Generated desktop/assets/my-tools.ico and matching web icons")
