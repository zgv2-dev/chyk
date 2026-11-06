#!/usr/bin/env python3
"""Generate the Glint "Phosphor" UI mockups and render PNGs with headless Chrome.

Usage: python3 design/concepts/build.py
"""
import random
import subprocess

import mock_base
from mock_base import HERE, H, LANGS, W
from mock_editor import deck, header, stage
from mock_popovers import export_pop, metadata
from mock_start import start

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
random.seed(7)
CSS = "\n" + (HERE / "mockup.css").read_text(encoding="utf-8")

# Font candidates. Space Mono is the current design; Menlo stands in for egui's Hack for Cyrillic.
FONTS = {
    "space-mono": "SpaceMono, Menlo, monospace",
    "fira-mono": "'Fira Mono', monospace",
    "play": "Play, sans-serif",
    "didact-gothic": "'Didact Gothic', sans-serif",
}


def screens():
    return {
        "start": start(),
        "editor": header("crop") + stage("crop") + deck("crop"),
        "strip": header("strip") + stage("strip") + deck("strip"),
        "metadata": header("crop", info_open=True) + stage("crop") + deck("crop") + metadata(),
        "export": header("crop", export_open=True) + stage("crop") + deck("crop") + export_pop(),
    }


# Points each popover's arrow at the center of the button that opened it.
ARROWS = """<script>
for (const pop of document.querySelectorAll(".pop")) {
  const anchor = document.querySelector(pop.classList.contains("export") ? ".primary.active" : ".info.on");
  const p = pop.getBoundingClientRect(), a = anchor.getBoundingClientRect();
  pop.style.setProperty("--ax", `${a.left + a.width / 2 - p.left - 6}px`);
}
</script>"""


def page(lang, family, body):
    return f"""<!DOCTYPE html><html lang="{lang}"><head><meta charset="utf-8">
<link rel="stylesheet" href="assets/phosphor.css"><link rel="stylesheet" href="assets/phosphor-fill.css">
<style>:root {{ --font: {family}; }}{CSS}</style></head><body><div class="app">{body}</div>{ARROWS}</body></html>"""


def main():
    out = HERE / "renders"
    out.mkdir(exist_ok=True)
    for old in list(out.glob("*.png")) + list(HERE.glob("*.html")):
        if old.name != "index.html":
            old.unlink()
    for old in out.glob("fonts/*/*.png"):
        old.unlink()
    for lang, strings in LANGS.items():
        mock_base.STRINGS = strings
        suffix = "" if lang == "en" else f"-{lang}"
        for name, body in screens().items():
            for font, family in FONTS.items():
                # The Space Mono pages are the design itself; the editor previews them.
                main = font == "space-mono"
                html = HERE / (f"phosphor-{name}{suffix}.html" if main else f"font-{font}-{name}{suffix}.html")
                html.write_text(page(lang, family, body), encoding="utf-8")
                png = out / (f"phosphor-{name}{suffix}.png" if main else f"fonts/{font}/{name}{suffix}.png")
                png.parent.mkdir(parents=True, exist_ok=True)
                render(html, png)


def render(html, png):
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    f"--window-size={W},{H}", "--force-device-scale-factor=2",
                    "--virtual-time-budget=3000", f"--screenshot={png}", html.as_uri()],
                   check=True, capture_output=True)
    print(png.relative_to(HERE))

if __name__ == "__main__":
    main()
