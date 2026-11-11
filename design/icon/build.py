#!/usr/bin/env python3
"""Chyk app icon concepts: SVG sources, 1024 px PNG renders, and a contact sheet at real sizes.

Usage: python3 design/icon/build.py
"""
import random
import struct
import subprocess
from pathlib import Path

from common import DEFS, G, G2, INK, TILE
from snip import mark, snip

HERE = Path(__file__).resolve().parent
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def brackets(inset, arm, width, radius, color=G):
    """Four rounded corner brackets of a square from `inset` to 1024-`inset`."""
    a, b = inset, 1024 - inset
    paths = []
    for (x, y, dx, dy) in [(a, a, 1, 1), (b, a, -1, 1), (a, b, 1, -1), (b, b, -1, -1)]:
        paths.append(
            f"M{x},{y + dy * arm} L{x},{y + dy * radius} "
            f"Q{x},{y} {x + dx * radius},{y} L{x + dx * arm},{y}"
        )
    d = " ".join(paths)
    return (f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{width}" '
            f'stroke-linecap="round" stroke-linejoin="round" filter="url(#glow)"/>')


def play(cx, cy, size, color):
    """Rounded play triangle, optically centered."""
    h = size
    w = size * 0.9
    x0 = cx - w * 0.38
    pts = f"{x0},{cy - h / 2} {x0 + w},{cy} {x0},{cy + h / 2}"
    return (f'<polygon points="{pts}" fill="{color}" stroke="{color}" stroke-width="{size * 0.16}" '
            f'stroke-linejoin="round"/>')


def viewfinder():
    return TILE + brackets(260, 150, 44, 46) + f"""
<circle cx="512" cy="512" r="58" fill="{G}" filter="url(#glow)"/>
<circle cx="512" cy="512" r="22" fill="#eafff3"/>"""


def peek():
    # Almond eye from two arcs; the pupil is a play button.
    return TILE + brackets(214, 110, 30, 34, G2) + f"""
<path d="M262,512 Q512,262 762,512 Q512,762 262,512 Z" fill="#06150d" stroke="{G}" stroke-width="34"
      stroke-linejoin="round" filter="url(#glow)"/>
<circle cx="512" cy="512" r="112" fill="{G}" filter="url(#glow)"/>
{play(512, 512, 104, INK)}"""


def trim_box():
    # Quick Look-style trim box: thick handles with grips, bars top and bottom, a playhead.
    frames = "".join(
        f'<rect x="{318 + i * 98}" y="402" width="88" height="220" rx="10" fill="{G}" fill-opacity="{0.10 + 0.05 * (i % 2)}"/>'
        for i in range(4)
    )
    return TILE + f"""
{frames}
<g filter="url(#glow)">
  <path d="M308,372 h408 a54,54 0 0 1 54,54 v172 a54,54 0 0 1 -54,54 h-408 a54,54 0 0 1 -54,-54 v-172 a54,54 0 0 1 54,-54 Z
           M318,402 v220 h388 v-220 Z" fill="{G}" fill-rule="evenodd"/>
</g>
<g stroke="{INK}" stroke-width="9" stroke-linecap="round">
  <line x1="277" y1="476" x2="277" y2="548"/><line x1="295" y1="476" x2="295" y2="548"/>
  <line x1="729" y1="476" x2="729" y2="548"/><line x1="747" y1="476" x2="747" y2="548"/>
</g>
<rect x="463" y="340" width="14" height="344" rx="7" fill="#ffffff" filter="url(#glow)"/>"""


def signal():
    random.seed(4)
    glyphs = "0123456789ABCDEF:=+*<>"
    cols = []
    for x in range(132, 900, 46):
        if random.random() < 0.35:
            continue
        top = random.randint(60, 600)
        n = random.randint(5, 12)
        for i in range(n):
            y = top + i * 50
            fade = 0.05 + 0.13 * (i / n)
            fill = "#d9ffe9" if i == n - 1 else G
            cols.append(f'<text x="{x}" y="{y}" fill="{fill}" fill-opacity="{fade:.2f}" font-size="40" '
                        f'font-family="Menlo, monospace" text-anchor="middle">{random.choice(glyphs)}</text>')
    return TILE + f'<g clip-path="url(#tile)">{"".join(cols)}</g>' + brackets(250, 140, 40, 44) + f"""
<circle cx="512" cy="512" r="150" fill="#030805" fill-opacity="0.85"/>
<circle cx="512" cy="512" r="150" fill="none" stroke="{G}" stroke-width="10" stroke-opacity="0.5"/>
{play(520, 512, 150, G)}"""


CONCEPTS = {
    "a-viewfinder": ("Viewfinder", viewfinder),
    "b-peek": ("Peek", peek),
    "c-trim-box": ("Trim box", trim_box),
    "d-signal": ("Signal", signal),
    "e-snip": ("Snip", snip),
}
APP = "e-snip"  # the concept shipped as the app icon
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)


def render(html_path, png, w, h, transparent=True):
    args = [CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars", f"--window-size={w},{h}",
            "--force-device-scale-factor=1", f"--screenshot={png}", html_path.as_uri()]
    if transparent:
        args.insert(1, "--default-background-color=00000000")
    subprocess.run(args, check=True, capture_output=True)


def write_assets(png):
    """assets/chyk.png (window, macOS and Linux icons) and assets/chyk.ico (Windows), from `png`."""
    assets = HERE.parent.parent / "assets"
    (assets / "chyk.png").write_bytes(png.read_bytes())
    images = []
    for s in ICO_SIZES:
        small = png.with_name(f"{png.stem}-{s}.png")
        subprocess.run(["sips", "-z", str(s), str(s), str(png), "--out", str(small)], check=True,
                       capture_output=True)
        images.append((s, small.read_bytes()))
    # ICO: header, one 16-byte entry per image (256 is written as 0), then the PNGs themselves.
    head = struct.pack("<HHH", 0, 1, len(images))
    offset, entries = len(head) + 16 * len(images), b""
    for s, data in images:
        entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    (assets / "chyk.ico").write_bytes(head + entries + b"".join(d for _, d in images))
    print(assets / "chyk.png", assets / "chyk.ico")


def main():
    out = HERE / "renders"
    out.mkdir(exist_ok=True)
    for slug, (_, draw) in CONCEPTS.items():
        svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">{DEFS}{draw()}</svg>'
        (HERE / f"{slug}.svg").write_text(svg)
        page = HERE / f".{slug}.html"
        page.write_text(f'<html><body style="margin:0;background:transparent">{svg}</body></html>')
        render(page, out / f"{slug}.png", 1024, 1024)
        page.unlink()
        print(out / f"{slug}.png")
    write_assets(out / f"{APP}.png")

    # assets/mark.png: the icon without its tile, cropped to a 5:4 box, for the app's header.
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="192 276 640 512" width="120" height="96">'
           f'{DEFS}{mark()}</svg>')
    page = HERE / ".mark.html"
    page.write_text(f'<html><body style="margin:0;background:transparent">{svg}</body></html>')
    render(page, HERE.parent.parent / "assets" / "mark.png", 120, 96)
    page.unlink()

    # Contact sheet: each concept large, plus Dock/Finder sizes on dark and light backgrounds.
    rows = ""
    for slug, (name, _) in CONCEPTS.items():
        img = f"renders/{slug}.png"
        smalls = "".join(f'<img src="{img}" width="{s}" height="{s}">' for s in (128, 64, 32, 16))
        rows += f"""<section><h2>{name}</h2><img src="{img}" width="300" height="300">
<div class="sizes dark">{smalls}</div><div class="sizes light">{smalls}</div></section>"""
    sheet = HERE / "index.html"
    sheet.write_text(f"""<!DOCTYPE html><html><head><meta charset="utf-8"><title>Chyk icon concepts</title><style>
body {{ margin: 0; padding: 28px; background: #101010; color: #cfeedd; font: 14px Menlo, monospace;
       display: grid; grid-template-columns: repeat(5, 1fr); gap: 20px; }}
section {{ display: flex; flex-direction: column; align-items: center; gap: 12px; }}
h2 {{ font-size: 15px; margin: 0; color: #4dff9b; font-weight: 400; }}
.sizes {{ display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-radius: 10px; }}
.dark {{ background: #1e1e1e; }} .light {{ background: #ececec; }}
</style></head><body>{rows}</body></html>""")
    render(sheet, out / "sheet.png", 1750, 720, transparent=False)
    print(out / "sheet.png")


if __name__ == "__main__":
    main()
