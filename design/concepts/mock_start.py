"""Start screen: digital rain behind the drop target, recent files and the key hints."""
import random

from mock_base import LIGHTS, LOGO, W, T, kbd, ph, tip
from mock_header import update_button


def rain():
    glyphs = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜ0123456789:.=*+<>"
    cols = []
    for c in range(0, W, 22):
        if random.random() < 0.55:
            continue
        n = random.randint(6, 22)
        top = random.randint(-200, 500)
        chars = "".join(f"<b>{random.choice(glyphs)}</b>" for _ in range(n))
        cols.append(f'<span style="left:{c}px;top:{top}px;opacity:{random.uniform(.15, .6):.2f}">{chars}</span>')
    return f'<div class="rain">{"".join(cols)}</div>'


RECENT = [("film_03", "rooftop_take3.mov", "H.264 · 1920×1080 · 0:42", "2 h"),
          ("film_07", "standup-2026-11-10.mp4", "HEVC · 3840×2160 · 5:15", "yesterday"),
          ("film_10", "bug-repro-scroll.mov", "H.264 · 2880×1800 · 1:08", "Nov 7")]


def start():
    recent = "".join(
        f'<div class="recent-row {"on" if i == 0 else ""}"><img src="assets/{img}.jpg">'
        f'<span class="nm"><b>{name}</b><em>{meta}</em></span><span class="when">{when}</span>{kbd("⌘", str(i + 1))}</div>'
        for i, (img, name, meta, when) in enumerate(RECENT))
    hints = [(("Space",), "cmd.play", "Play"), (("I", "O"), "cmd.trim", "Trim"), (("C",), "mode.crop", None),
             (("S",), "mode.strip", None), (("⌘", "S"), "export.button", None), (("?",), "status.shortcuts", "Shortcuts")]
    legend = "".join(f'<span>{kbd(*k)}{T(key, en)}</span>' for k, key, en in hints)
    return f"""
<header class="top start">
  {LIGHTS}{LOGO}<span class="wordmark">CHYK</span>
  <div class="grow"></div>
  {update_button("downloading")}
</header>
<main class="empty">
  {rain()}
  <div class="center">
    <div class="drop">
      <div class="glyph">{ph("film-slate")}</div>
      <div class="label">{T("open.drop")}</div>
      <div class="sub">MP4 · MOV · MKV · WebM · AVI</div>
      <button class="btn open">{ph("folder-open")}{T("open.button", "Open video…")}{kbd("⌘", "O")}</button>
    </div>
    <div class="recent">
      <h5>{T("open.recent", "Recent")}<span class="grow"></span><em>{kbd("↑", "↓")} {kbd("⏎")}</em></h5>
      {recent}
    </div>
    <div class="legend">{legend}</div>
  </div>
</main>
<div class="status"><button class="term" {tip("tip.log", "ffmpeg log")}>{ph("terminal-window")}</button>
  <span class="left">v1.2.6 · {T("credit", name="zgv2")}</span><div class="grow"></div>
  <span class="hint">{kbd("?")}{T("status.shortcuts", "Shortcuts")}</span></div>"""
