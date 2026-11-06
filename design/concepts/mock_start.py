"""Start screen: digital rain behind the drop target and the shortcut legend."""
import random

from mock_base import LIGHTS, LOGO, W, T, ph


def rain():
    glyphs = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜ0123456789:.=*+<>"
    cols = []
    for c in range(0, W, 22):
        if random.random() < 0.45:
            continue
        n = random.randint(6, 26)
        top = random.randint(-200, 500)
        chars = "".join(f"<b>{random.choice(glyphs)}</b>" for _ in range(n))
        cols.append(f'<span style="left:{c}px;top:{top}px;opacity:{random.uniform(.25, 1):.2f}">{chars}</span>')
    return f'<div class="rain">{"".join(cols)}</div>'


# ---------------------------------------------------------------- start screen

def start():
    return f"""
<header class="top">
  {LIGHTS}{LOGO}<span class="wordmark">CHYK</span><span class="version">v1.2.0</span>
</header>
<main class="empty">
  {rain()}
  <div class="center">
    <div class="drop">
      <i class="c tl"></i><i class="c tr"></i><i class="c bl"></i><i class="c br"></i>
      <div class="glyph">{ph("film-slate")}</div>
      <div class="text"><div class="label">{T("open.drop")}</div><div class="sub">{T("open.browse")}</div></div>
    </div>
    <div class="legend">
      <span><kbd>⌘O</kbd>{ph("folder-open")}</span>
      <span><kbd>Space</kbd>{ph("play")}</span>
      <span><kbd>←</kbd><kbd>→</kbd>{ph("film-strip")}</span>
      <span><kbd>I</kbd><kbd>O</kbd>{ph("scissors")}</span>
      <span><kbd>C</kbd>{ph("crop")}</span>
      <span><kbd>,</kbd><kbd>.</kbd>{ph("gauge")}</span>
      <span><kbd>⌘S</kbd>{ph("export")}</span>
      <span><kbd>⌘W</kbd>{ph("x-circle")}</span>
    </div>
  </div>
</main>"""
