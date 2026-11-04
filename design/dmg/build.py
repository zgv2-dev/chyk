#!/usr/bin/env python3
"""Glint DMG window background (640x400 pt, @1x and @2x) and a Finder-like preview.

Usage: python3 design/dmg/build.py
Outputs background.png / background@2x.png (used by scripts/package-macos.sh) and preview.png.
"""
import random
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
W, H = 640, 400
# Icon centers, also used by scripts/package-macos.sh (dmgbuild icon_locations).
APP, APPS, ICON = (160, 186), (480, 186), 128
# Finder draws labels black in light mode and white in dark mode. This plate's luminance sits
# between them (~4.4:1 against both), so the label reads either way.
PLATE = "#6a7a70"


def rain():
    random.seed(11)
    out = []
    for x in list(range(18, 120, 22)) + list(range(530, 630, 22)):
        if random.random() < 0.45:
            continue
        top, n = random.randint(-60, 260), random.randint(4, 9)
        for i in range(n):
            a = 0.05 + 0.12 * i / n
            out.append(f'<text x="{x}" y="{top + i * 19}" fill="#4dff9b" fill-opacity="{a:.2f}" '
                       f'font-size="14" font-family="Menlo, monospace" text-anchor="middle">'
                       f'{random.choice("0123456789ABCDEF:=+*<>")}</text>')
    return "".join(out)


def background():
    (ax, ay), (bx, _) = APP, APPS
    x0, x1 = ax + ICON / 2 + 18, bx - ICON / 2 - 18
    plates = "".join(
        f'<rect x="{cx - 66}" y="{cy + ICON / 2 + 4}" width="132" height="24" rx="12" fill="{PLATE}"/>'
        for cx, cy in (APP, APPS)
    )
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}">
<defs>
  <radialGradient id="core" cx="0.5" cy="0.47" r="0.6">
    <stop offset="0" stop-color="#0a2416"/><stop offset="1" stop-color="#000"/>
  </radialGradient>
  <pattern id="dots" width="20" height="20" patternUnits="userSpaceOnUse">
    <circle cx="10" cy="10" r="1" fill="#4dff9b" fill-opacity="0.07"/>
  </pattern>
  <filter id="glow" filterUnits="userSpaceOnUse" x="0" y="0" width="{W}" height="{H}">
    <feGaussianBlur stdDeviation="4" result="b"/>
    <feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge>
  </filter>
</defs>
<rect width="{W}" height="{H}" fill="url(#core)"/>
<rect width="{W}" height="{H}" fill="url(#dots)"/>
{rain()}
<g filter="url(#glow)" stroke="#4dff9b" stroke-width="4" stroke-linecap="round" fill="none">
  <path d="M{x0},{ay} H{x1 - 6}" stroke-dasharray="2 12"/>
  <path d="M{x1 - 16},{ay - 13} L{x1},{ay} L{x1 - 16},{ay + 13}" stroke-linejoin="round"/>
</g>
{plates}
</svg>"""


def render(page: Path, png: Path, w, h, scale):
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    f"--window-size={w},{h}", f"--force-device-scale-factor={scale}",
                    f"--screenshot={png}", page.as_uri()], check=True, capture_output=True)


def main():
    svg = background()
    (HERE / "background.svg").write_text(svg)
    page = HERE / ".background.html"
    page.write_text(f'<html><body style="margin:0">{svg}</body></html>')
    render(page, HERE / "background.png", W, H, 1)
    render(page, HERE / "background@2x.png", W, H, 2)
    page.unlink()

    # Preview: a Finder window in light and dark mode, labels as Finder would draw them.
    app_icon = HERE.parent.parent / "assets/glint.png"
    apps_icon = HERE / ".apps-preview.png"
    if not apps_icon.exists():
        subprocess.run(["sips", "-s", "format", "png",
                        "/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/ApplicationsFolderIcon.icns",
                        "--out", str(apps_icon)], check=True, capture_output=True)

    def window(mode):
        label = "#000" if mode == "light" else "#fff"
        chrome = "#e8e8e8" if mode == "light" else "#2b2b2b"
        title = "#333" if mode == "light" else "#ddd"
        items = ""
        for (cx, cy), img, name in ((APP, app_icon, "Glint"), (APPS, apps_icon, "Applications")):
            items += (f'<img src="{img.as_uri()}" style="position:absolute;left:{cx - ICON / 2}px;top:{cy - ICON / 2}px;'
                      f'width:{ICON}px;height:{ICON}px">'
                      f'<div style="position:absolute;left:{cx - 70}px;top:{cy + ICON / 2 + 8}px;width:140px;'
                      f'text-align:center;color:{label};font:13px -apple-system">{name}</div>')
        return (f'<div style="width:{W}px;border-radius:10px;overflow:hidden;box-shadow:0 20px 50px rgba(0,0,0,.5)">'
                f'<div style="height:28px;background:{chrome};display:flex;align-items:center;gap:8px;padding:0 12px;'
                f'font:13px -apple-system;color:{title}">'
                + "".join(f'<i style="width:12px;height:12px;border-radius:50%;background:{c}"></i>'
                          for c in ("#ff5f57", "#febc2e", "#28c840"))
                + f'<span style="flex:1;text-align:center;margin-right:56px">Glint</span></div>'
                f'<div style="position:relative;width:{W}px;height:{H}px;'
                f'background:url({(HERE / "background@2x.png").as_uri()}) 0 0/{W}px {H}px">{items}</div></div>')

    preview = HERE / ".preview.html"
    preview.write_text(f'<html><body style="margin:0;padding:30px;background:#9aa3a0;display:flex;gap:30px">'
                       f'{window("light")}{window("dark")}</body></html>')
    render(preview, HERE / "preview.png", 2 * W + 90, H + 28 + 60, 1)
    preview.unlink()
    print(HERE / "preview.png")


if __name__ == "__main__":
    main()
