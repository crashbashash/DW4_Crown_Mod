#!/usr/bin/env python3
"""Render public/crown.svg into desktop icons.

Requires librsvg's rsvg-convert and Pillow. Run: python3 tools/gen_icon.py
"""
from __future__ import annotations
import io
from pathlib import Path
import subprocess
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "public" / "crown.svg"
ICONS = ROOT / "crates" / "dw4vhp-tauri" / "icons"


def main() -> None:
    rendered = subprocess.check_output([
        "rsvg-convert", "--width", "1024", "--height", "1024", str(SOURCE),
    ])
    source = Image.open(io.BytesIO(rendered)).convert("RGBA")
    source.resize((256, 256), Image.Resampling.LANCZOS).save(ICONS / "icon.png")
    sizes = (16, 24, 32, 48, 64, 128, 256)
    source.save(ICONS / "icon.ico", format="ICO", sizes=[(s, s) for s in sizes])
    print("Rendered 256px PNG and 16–256px Windows ICO from public/crown.svg")


if __name__ == "__main__":
    main()
