"""Popovers hanging from header buttons: export settings and file info."""
from mock_base import FILE, T, kbd, ph, tip


def seg(options, on, keys=False):
    return '<div class="seg">' + "".join(
        f'<button class="{"on" if i == on else ""}">{o}{f"<kbd>{i + 1}</kbd>" if keys else ""}</button>'
        for i, o in enumerate(options)) + "</div>"


def toggle(icon, key, hint, on, shortcut):
    return f"""
    <div class="row toggle"><span class="what">{ph(icon)}<span>{T(key)}<em>{T(hint)}</em></span></span>
      {kbd(shortcut)}<span class="switch {"on" if on else ""}"><i></i></span></div>"""


def export_pop():
    quality = [T("quality." + q) for q in ("high", "medium", "low", "compress")]
    return f"""
<div class="pop export" data-anchor=".primary">
  <div class="pop-head">
    <div><b>{T("export.title")}</b><span class="sub">rooftop_take3-edited.mp4</span></div>
    <button class="ib" {tip("common.close")}>{ph("x")}</button>
  </div>
  <div class="rows">
    <div class="row"><span class="label">{T("export.format")}</span>{seg(["MP4", "MOV", "MKV"], 0)}</div>
    <div class="row"><span class="label">{T("export.quality")}</span>{seg(quality, 1)}</div>
    <p class="note">{T("quality.medium_hint", "No visible loss · HEVC, VideoToolbox")}</p>
    <div class="row"><span class="label">{T("export.resolution")}</span>
      <button class="select">1080p<em>≈ 18.4 MB</em>{ph("caret-up-down")}</button></div>
    {toggle("crosshair", "export.precise", "export.precise_hint", False, "P")}
    {toggle("speaker-slash", "export.mute", "export.mute_hint", False, "M")}
  </div>
  <div class="summary">
    <span>{ph("timer")}0:12.600</span><span>{ph("hard-drives")}≈ 18.4 MB</span>
    <span>{ph("cube")}{T("export.reencode")} · HEVC</span>
  </div>
  <div class="pop-foot">
    <button class="btn ghost">{T("common.cancel")}{kbd("esc")}</button>
    <button class="primary">{ph("download-simple")}{T("export.save")}{kbd("⌘", "S")}</button>
  </div>
</div>"""


def metadata():
    def section(icon, key, rows):
        body = "".join(f'<div class="kv"><span>{T(k)}</span><b>{v}</b>{ph("copy")}</div>' for k, v in rows)
        return f'<section><h4>{ph(icon)}{T(key)}</h4>{body}</section>'

    return f"""
<div class="pop info" data-anchor=".info.on">
  <div class="pop-head">
    <div><b>{FILE}</b><span class="sub">~/Movies/Shoots</span></div>
    <button class="ib" {tip("meta.copy")}>{ph("copy")}</button>
    <button class="ib" {tip("common.close")}>{ph("x")}</button>
  </div>
  <div class="sections">
    {section("file", "meta.file", [("meta.container", "QuickTime / MOV"), ("meta.size", "84.2 MB"),
                                   ("meta.duration", "0:42.000"), ("meta.bitrate", "16.0 Mb/s"),
                                   ("meta.device", "iPhone 16 Pro")])}
    {section("film-strip", "meta.video", [("meta.codec", "H.264 · High"), ("meta.resolution", "1920×1080 · 16:9"),
                                          ("meta.frame_rate", "59.94 fps"), ("meta.color", "BT.709 · SDR")])}
    {section("waveform", "meta.audio", [("meta.codec", "AAC · LC"), ("meta.channels", "stereo · 48 kHz")])}
  </div>
</div>"""
