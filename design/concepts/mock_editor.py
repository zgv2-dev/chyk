"""Editor screen: header, stage with the crop or strip overlay, and the transport deck."""
from mock_base import LIGHTS, LOGO, T, ph, tip


def header(mode, export_open=False, info_open=False):
    return f"""
<header class="top">
  {LIGHTS}{LOGO}
  <div class="file">
    <span class="name">rooftop_take3.mov</span>
    <span class="meta"><span>{ph("film-strip")}H.264</span><span>{ph("frame-corners")}1920×1080</span><span>{ph("timer")}0:42.000</span></span>
  </div>
  <button class="ib info {"on" if info_open else ""}" {tip("tip.info")}>{ph("info")}</button>
  <button class="ib close" {tip("tip.close_video")}>{ph("x-circle")}</button>
  <nav class="modes">
    <button class="mode {"on" if mode == "crop" else ""}">{ph("crop")}{T("mode.crop")}</button>
    <button class="mode {"on" if mode == "strip" else ""}">{ph("magic-wand")}{T("mode.strip")}</button>
  </nav>
  <div class="grow"></div>
  <button class="primary {"active" if export_open else ""}" {tip("tip.export")}>{ph("export")}{T("export.button")}</button>
</header>"""


def stripbar():
    return f"""
<div class="stripbar">
  <button class="ghost" {tip("tip.add_zone")}>{ph("eye-slash")}{T("strip.add_zone")}</button>
  <button class="ib on" {tip("strip.edit_zones")}>{ph("pencil-simple")}</button>
  <button class="ib" {tip("strip.clear")}>{ph("trash")}</button>
  <span class="sep"></span>
  <span class="pad" {tip("tip.pad")}>{ph("arrows-horizontal")}<span class="field">0.50 {T("unit.s")}</span></span>
  <span class="sep"></span>
  <span class="kept">{ph("scissors")}<span class="bar"><i style="width:62%"></i></span>0:26.040 <em>/ 0:42.000</em></span>
  <button class="accent">{ph("arrows-clockwise")}{T("strip.rescan")}</button>
</div>"""


def stage(mode):
    if mode == "crop":
        overlay = f"""
    <div class="crop"><i class="h tl"></i><i class="h tr"></i><i class="h bl"></i><i class="h br"></i></div>
    <div class="aspect"><button>{T("crop.free")}</button><button>{T("crop.original")}</button><button>16:9</button><button>9:16</button><button>1:1</button><button class="on">4:3</button></div>"""
    elif mode == "strip":
        overlay = stripbar() + """
    <div class="zone sel" style="left:76%;top:17%;width:18%;height:11%"><i class="k tl"></i><i class="k tr"></i><i class="k bl"></i><i class="k br"></i></div>
    <div class="zone" style="left:3%;top:84%;width:24%;height:10%"></div>"""
    else:
        overlay = ""
    return f"""
<main class="stage">
  <div class="video">
    <img src="assets/frame.jpg">{overlay}
    <div class="zoom"><button>{ph("minus")}</button><span>100%</span><button>{ph("plus")}</button><button>{ph("corners-in")}</button></div>
  </div>
</main>"""


def deck(mode):
    thumbs = "".join(f'<img src="assets/film_{i:02d}.jpg">' for i in range(1, 13))
    if mode == "strip":
        # Whole clip selected; red gaps are the stretches strip will cut.
        box = (4, 92)
        gaps = "".join(f'<i class="gap" style="left:{a}%;width:{b - a}%"></i>' for a, b in [(13, 24), (37, 41), (55, 71), (83, 92)])
        playhead = 46
    else:
        box, gaps, playhead = (14, 44), "", 30
    left, width = box
    return f"""
<footer class="deck">
  <div class="transport">
    <button class="play" {tip("tip.play")}><i class="ph-fill ph-play"></i></button>
    <button class="ib" {tip("tip.back")}>{ph("clock-counter-clockwise")}</button>
    <button class="ib" {tip("tip.forward")}>{ph("clock-clockwise")}</button>
    <div class="tc"><b>0:04.880</b><span>0:42.000</span></div>
    <button class="speed">{ph("gauge")}1×{ph("caret-down")}</button>
    <div class="grow"></div>
    <div class="range">
      <span>{ph("sign-in")}{"0:00.000" if mode == "strip" else "0:02.400"}</span>
      <span>{ph("sign-out")}{"0:42.000" if mode == "strip" else "0:08.120"}</span>
      <span>{ph("ruler")}{"0:42.000" if mode == "strip" else "0:05.720"}</span>
    </div>
    <div class="tzoom">
      <button class="ib">{ph("magnifying-glass-minus")}</button>
      <button class="ib">{ph("magnifying-glass-plus")}</button>
      <button class="ib">{ph("arrows-in-line-horizontal")}</button>
      <button class="ib on">{ph("arrows-out-line-horizontal")}</button>
    </div>
  </div>
  <div class="film" style="--a:{left}%;--w:{width}%;--p:{playhead}%">
    <div class="thumbs">{thumbs}</div>
    {gaps}
    <div class="dim l"></div><div class="dim r"></div>
    <div class="box"><i class="g l"></i><i class="g r"></i></div>
    <div class="playhead"></div>
  </div>
</footer>"""
