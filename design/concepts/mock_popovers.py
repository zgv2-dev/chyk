"""Export and file info popovers."""
from mock_base import EN, T, ph, tip


def seg(icon, label, options, on):
    btns = "".join(f'<button class="{"on" if o == on else ""}">{T(o) if o in EN else o}</button>' for o in options)
    return f'<div class="row"><label>{ph(icon)}{T(label)}</label><div class="seg">{btns}</div></div>'


def toggle(icon, label, on):
    return f'<div class="row tog"><label>{ph(icon)}{T(label)}</label><span class="switch {"on" if on else ""}"><i></i></span></div>'


def export_pop():
    return f"""
<div class="scrim"></div>
<aside class="pop export">
  <div class="pop-head">{ph("export")}{T("export.title")}<div class="grow"></div><button class="ib">{ph("x")}</button></div>
  {seg("package", "export.format", ["MP4", "MOV", "MKV"], "MOV")}
  {seg("sliders-horizontal", "export.quality", ["quality.high", "quality.medium", "quality.low", "quality.compress"], "quality.high")}
  {toggle("crosshair", "export.precise", True)}
  {toggle("speaker-slash", "export.mute", False)}
  <div class="summary">
    <span>{ph("timer")}0:05.720</span>
    <span>{ph("crop")}1440×1080</span>
    <span>{ph("cpu")}{T("export.reencode")} · VideoToolbox</span>
  </div>
  <button class="primary wide">{ph("download-simple")}{T("export.save")}</button>
</aside>"""


def kv(rows):
    return "".join(f"<dt>{T(k) if k in EN else k}</dt><dd>{v}</dd>" for k, v in rows)


def metadata():
    return f"""
<div class="scrim"></div>
<aside class="pop meta-pop">
  <div class="pop-head">{ph("info")}<span>rooftop_take3.mov</span><div class="grow"></div><button class="ib" {tip("meta.copy")}>{ph("copy")}</button><button class="ib">{ph("x")}</button></div>
  <section><h3>{ph("file")}{T("meta.file")}</h3><dl>{kv([
      ("meta.container", "QuickTime / MOV"), ("meta.size", "248.6 MB"), ("meta.duration", "0:42.000"),
      ("meta.bitrate", "47.3 Mb/s"), ("meta.created", "2026-10-28 18:42"), ("meta.device", "iPhone 15 Pro"),
  ])}</dl></section>
  <section><h3>{ph("film-strip")}{T("meta.video")}</h3><dl>{kv([
      ("meta.codec", "H.264 High · L4.2"), ("meta.resolution", "1920×1080 · 16:9"), ("meta.frame_rate", "29.97 fps · VFR"),
      ("meta.pixels", "yuv420p · 8-bit"), ("meta.color", "BT.709 · SDR"), ("meta.bitrate", "45.1 Mb/s"), ("meta.rotation", "0°"),
  ])}</dl></section>
  <section><h3>{ph("waveform")}{T("meta.audio")}</h3><dl>{kv([
      ("meta.codec", "AAC LC"), ("meta.channels", "Stereo · 48 kHz"), ("meta.bitrate", "256 kb/s"), ("meta.language", "English"),
  ])}</dl></section>
  <section><h3>{ph("subtitles")}{T("meta.subtitles")}</h3><dl><dt>{T("meta.track", n=1)}</dt><dd>mov_text · English</dd></dl></section>
</aside>"""
