#!/usr/bin/env python3
"""Generate the Chyk "Phosphor" UI mockups and render PNGs with headless Chrome.

Usage: python3 design/concepts/build.py
"""
import random
import shutil
import subprocess

import mock_base
from mock_base import HERE, H, LANGS, W, T, tooltip
from mock_editor import deck, stage, status
from mock_header import header
from mock_overlays import confirm, context_menu, log_window, shortcuts, toast
from mock_popovers import export_pop, metadata
from mock_start import start

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
random.seed(7)
CSS = "\n".join((HERE / f).read_text(encoding="utf-8") for f in ("base.css", "editor.css", "overlays.css"))
MONO = "SpaceMono, Menlo, monospace"

# (UI font, numbers/timecode font). Space Mono for both, decided 2026-11-11 over a UI sans; another
# candidate can go here and renders to renders/fonts/<name>/.
FONTS = {"space-mono": (MONO, MONO)}


SMALL = (980, 460)  # the app's minimum window size


def editor(panel=None, extra="", small=False, **head):
    return header(panel, small=small, **head) + stage(panel) + deck(panel) + status() + extra


def screens():
    """name: (body, window size, body class)."""
    exporting = (f'<span class="ok">●</span>{T("export.exporting", "Exporting")} rooftop_take3-edited.mp4'
                 f'<em>·</em>64%<em>·</em>{T("export.left", "0:12 left")}')
    big = {
        "start": start(),
        "editor": editor(extra=tooltip(".speed", T("cmd.faster", "Faster / slower"), ",", ".")),
        "crop": editor("crop"),
        "strip": header("strip") + stage("strip") + deck("strip") + status(),
        "metadata": editor(pop="info", extra='<div class="scrim light"></div>' + metadata()),
        "export": editor(pop="export", extra='<div class="scrim light"></div>' + export_pop()),
        "shortcuts": editor(extra=shortcuts()),
        "confirm": editor(extra=confirm()),
        "exporting": header(exporting=True, update=None) + stage() + deck() + status(exporting, busy=True),
        "log": (header(exporting=True, update=None) + stage() + deck()
                + status(exporting, log=True, busy=True) + log_window()),
        "menu": editor(extra=context_menu() + toast(), update="ready"),
    }
    small = {
        "small-editor": editor(small=True),
        "small-crop": editor("crop", small=True),
        "small-strip": header("strip", small=True) + stage("strip") + deck("strip") + status(),
    }
    return {**{k: (v, (W, H), "") for k, v in big.items()}, **{k: (v, SMALL, "small") for k, v in small.items()}}


# Pins tooltips over their anchors and points each popover's arrow at its button.
PLACE = """<script>
addEventListener("load", () => {
const box = s => document.querySelector(s).getBoundingClientRect();
for (const t of document.querySelectorAll(".tooltip[data-for]")) {
  const a = box(t.dataset.for), r = t.getBoundingClientRect();
  t.style.left = `${a.left + a.width / 2 - r.width / 2}px`;
  t.style.top = t.classList.contains("top") ? `${a.top - r.height - 8}px` : `${a.bottom + 8}px`;
}
for (const p of document.querySelectorAll(".pop[data-anchor]")) {
  const a = box(p.dataset.anchor);
  if (p.classList.contains("info")) p.style.left = `${Math.max(12, a.left + a.width / 2 - 64)}px`;
  p.style.setProperty("--ax", `${a.left + a.width / 2 - p.getBoundingClientRect().left - 6}px`);
}
});
</script>"""


def page(lang, fonts, body, cls=""):
    ui, mono = fonts
    return f"""<!DOCTYPE html><html lang="{lang}"><head><meta charset="utf-8">
<link rel="stylesheet" href="assets/phosphor.css"><link rel="stylesheet" href="assets/phosphor-fill.css">
<style>:root {{ --font: {ui}; --mono: {mono}; }}{CSS}</style></head><body class="{cls}"><div class="app">{body}</div>{PLACE}</body></html>"""


def main():
    # The mockups (and the i18n editor's server) load the logo from their own assets/.
    (HERE / "assets").mkdir(exist_ok=True)
    shutil.copyfile(HERE.parent.parent / "assets/mark.png", HERE / "assets/mark.png")
    out = HERE / "renders"
    out.mkdir(exist_ok=True)
    for old in list(out.glob("*.png")) + list(out.glob("fonts/*/*.png")) + list(HERE.glob("*.html")):
        if old.name != "index.html":
            old.unlink()
    for lang, strings in LANGS.items():
        mock_base.STRINGS = strings
        suffix = "" if lang == "en" else f"-{lang}"
        for name, (body, size, cls) in screens().items():
            for font, fonts in FONTS.items():
                # The Space Mono pages are the design itself; the i18n editor previews them.
                main = font == "space-mono"
                html = HERE / (f"phosphor-{name}{suffix}.html" if main else f"font-{font}-{name}{suffix}.html")
                html.write_text(page(lang, fonts, body, cls), encoding="utf-8")
                png = out / (f"phosphor-{name}{suffix}.png" if main else f"fonts/{font}/{name}{suffix}.png")
                png.parent.mkdir(parents=True, exist_ok=True)
                render(html, png, size)


def render(html, png, size=(W, H)):
    png.unlink(missing_ok=True)
    run = subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                          f"--window-size={size[0]},{size[1]}", "--force-device-scale-factor=2",
                          "--virtual-time-budget=3000", f"--screenshot={png}", html.as_uri()],
                         capture_output=True, text=True)
    # Headless Chrome sometimes crashes in teardown after writing the file; only a missing PNG fails.
    if not png.exists():
        raise RuntimeError(f"{png.name}: Chrome exited {run.returncode}\n{run.stderr[-600:]}")
    print(png.relative_to(HERE))


if __name__ == "__main__":
    main()
