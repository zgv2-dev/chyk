"""Editor body: the stage (video with the crop or strip overlay), the deck and the status bar."""
from mock_base import T, kbd, ph, tip

DUR = 42.0


def stage(panel=None):
    if panel == "crop":
        overlay = """
    <div class="crop"><i class="h tl"></i><i class="h tr"></i><i class="h bl"></i><i class="h br"></i>
      <i class="h t"></i><i class="h b"></i><i class="h l"></i><i class="h r"></i>
      <span class="badge">1208 × 906 · 4:3</span></div>"""
    elif panel == "strip":
        overlay = f"""
    <div class="zone sel" style="left:76%;top:7%;width:19%;height:11%"><span>{T("strip.zone", "Ignored")}</span>
      <i class="k tl"></i><i class="k tr"></i><i class="k bl"></i><i class="k br"></i></div>
    <div class="zone" style="left:3%;top:84%;width:24%;height:10%"><span>{T("strip.zone", "Ignored")}</span></div>"""
    else:
        overlay = ""
    return f'<main class="stage"><div class="video"><img src="assets/frame.jpg">{overlay}</div></main>'


def ruler():
    ticks = "".join(
        f'<i class="{"major" if s % 5 == 0 else ""}" style="left:{s / DUR * 100:.3f}%">'
        f'{f"<b>0:{s:02d}</b>" if s % 5 == 0 else ""}</i>'
        for s in range(0, int(DUR) + 1))
    return f'<div class="ruler">{ticks}</div>'


def deck(panel=None, playing=False):
    thumbs = "".join(f'<img src="assets/film_{i:02d}.jpg">' for i in range(1, 13))
    if panel == "strip":
        # Whole clip selected; red stretches are what Strip cuts.
        (a, w), playhead = (0, 100), 46
        gaps = "".join(f'<i class="gap" style="left:{x}%;width:{y - x}%"></i>'
                       for x, y in [(9, 21), (37, 41), (55, 71), (83, 92)])
        sel = ("0:00.000", "0:42.000", "0:42.000")
    else:
        (a, w), playhead, gaps = (14, 30), 30, ""
        sel = ("0:05.880", "0:18.480", "0:12.600")
    play = ph("pause", True) if playing else ph("play", True)
    return f"""
<footer class="deck">
  <div class="transport">
    <button class="play" {tip("tip.play")}>{play}</button>
    <button class="ib back" {tip("tip.back")}>{ph("clock-counter-clockwise")}</button>
    <button class="ib fwd" {tip("tip.forward")}>{ph("clock-clockwise")}</button>
    <div class="tc"><b>0:12.600</b><em>/ 0:42.000</em></div>
    <span class="vsep"></span>
    <button class="pill speed" {tip("tip.speed")}>{ph("gauge")}1×</button>
    <div class="group zoom"><button>{ph("minus")}</button><span>100%</span><button>{ph("plus")}</button></div>
    <div class="grow"></div>
    <div class="group range">
      <span><em>{T("range.in", "IN")}</em>{sel[0]}</span>
      <span><em>{T("range.out", "OUT")}</em>{sel[1]}</span>
      <span><em>{T("range.len", "LEN")}</em>{sel[2]}</span>
    </div>
    <div class="group tzoom">
      <button>{ph("magnifying-glass-minus")}</button><button>{ph("magnifying-glass-plus")}</button>
      <button>{ph("arrows-in-line-horizontal")}</button><button class="on">{ph("arrows-out-line-horizontal")}</button>
    </div>
  </div>
  {ruler()}
  <div class="film" style="--a:{a}%;--w:{w}%;--p:{playhead}%">
    <div class="thumbs">{thumbs}</div>{gaps}
    <div class="dim l"></div><div class="dim r"></div>
    <div class="box"><i class="g l"></i><i class="g r"></i></div>
    <div class="playhead"><span>0:12.600</span></div>
  </div>
</footer>"""


def status(left=None, log=False, busy=False):
    """Bottom bar: the ffmpeg log button, state, and the shortcut sheet hint."""
    left = left or f'<span class="ok">●</span>{T("status.ready", "Ready")}<em>·</em>{T("status.reencode_crop", "Re-encode · 4:3 crop")}'
    dot = '<i class="busy"></i>' if busy else ""
    return f"""
<div class="status">
  <button class="term {"on" if log else ""}" {tip("tip.log", "ffmpeg log")}>{ph("terminal-window")}{dot}</button>
  <span class="left">{left}</span>
  <div class="grow"></div>
  <span class="hint">{kbd("?")}{T("status.shortcuts", "Shortcuts")}</span>
  <span class="ver">v1.2.6</span>
</div>"""
