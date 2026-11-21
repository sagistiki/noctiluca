"""Makes every icon file from the 1024 render of assets/icon/noctiluca.svg:
the Mac's .icns (all sizes, by iconutil) and the Android launcher icons.

    python3 docs/brand/make_icons.py icon.png
"""
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
icon = Image.open(sys.argv[1]).convert("RGBA").resize((1024, 1024), Image.LANCZOS)

out = ROOT / "assets/icon"
icon.save(out / "icon_1024.png")

# The .icns macOS wants: every size, made by iconutil from an .iconset.
iconset = out / "Noctiluca.iconset"
iconset.mkdir(exist_ok=True)
for s in (16, 32, 128, 256, 512):
    icon.resize((s, s), Image.LANCZOS).save(iconset / f"icon_{s}x{s}.png")
    icon.resize((s * 2, s * 2), Image.LANCZOS).save(iconset / f"icon_{s}x{s}@2x.png")
subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(out / "Noctiluca.icns")], check=True)
shutil.rmtree(iconset)

# Android: the tile alone (Apple's margins off), at each density.
tile = icon.crop((100, 100, 924, 924))
res = ROOT / "android/app/src/main/res"
for folder, s in (("mdpi", 48), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)):
    tile.resize((s, s), Image.LANCZOS).save(res / f"mipmap-{folder}/ic_launcher.png")
