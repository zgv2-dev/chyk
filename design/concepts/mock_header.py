"""Header: window controls, mark, Crop/Strip, then the file or the open panel's bar (so opening a panel
never changes the video's height), the update button and Export."""
from mock_base import FILE, LIGHTS, LOGO, T, kbd, ph, tip


def header(panel=None, pop=None, exporting=False, update="available", small=False):
    def mode(name, icon, key):
        on = "on" if panel == name else ""
        dot = '<i class="dot"></i>' if name == "crop" and panel != "crop" else ""
        return f'<button class="mode {on}">{ph(icon)}{T("mode." + name)}{dot}{"" if small else kbd(key)}</button>'

    if exporting:
        action = f"""
  <div class="progress-pill"><span class="fill" style="width:64%"></span>
    {ph("export")}<b>64%</b><em>{T("export.left", "0:12 left")}</em>
    <button class="x" {tip("export.cancel")}>{ph("x")}</button></div>"""
    else:
        on = "active" if pop == "export" else ""
        action = (f'<button class="primary {on}" {tip("tip.export")}>{ph("export")}{T("export.button")}'
                  f'{"" if small else kbd("⌘", "S")}</button>')
    slot = bar(panel, small) if panel else f"""
  <div class="file"><span class="name">{FILE}</span>{"" if small else '<span class="meta">H.264 · 1920×1080 · 0:42</span>'}</div>
  <button class="ib info {"on" if pop == "info" else ""}" {tip("tip.info")}>{ph("info")}</button>
  <button class="ib" {tip("tip.close_video")}>{ph("x-circle")}</button>"""
    return f"""
<header class="top">
  {LIGHTS}{LOGO}
  <nav class="modes">{mode("crop", "crop", "C")}{mode("strip", "magic-wand", "S")}</nav>
  <span class="vsep"></span>{slot}
  <div class="grow"></div>
  {update_button(update, small or panel is not None)}{action}
</header>"""


def update_button(state, small=False):
    """Update available → downloading (progress, cancel) → Update now; label over the version tag.
    Icon only on small windows and while a panel's bar takes the header."""
    if not state:
        return ""
    if state == "downloading":
        return f"""<div class="update dl"><span class="fill" style="width:42%"></span>{ph("arrow-circle-down")}
  <span class="two"><b>42%</b><em>v1.2.7</em></span><button class="x" {tip("tip.update_cancel")}>{ph("x")}</button></div>"""
    icon, key, cls = ("arrow-circle-up", "update.available", "") if state == "available" else ("arrow-clockwise", "update.now", "ready")
    label = "" if small else f'<span class="two"><b>{T(key)}</b><em>v1.2.7</em></span>'
    return f'<button class="update {cls} {"icon" if small else ""}" {tip("tip.update_available", "Download v1.2.7")}>{ph(icon)}{label}</button>'


def bar(panel, small=False):
    """The open panel's controls. Keys live in tooltips and the ? sheet, so the bar fits the header."""
    if panel == "crop":
        presets = [T("crop.free"), T("crop.original"), "16:9", "9:16", "1:1", "4:3"]
        chips = "".join(f'<button class="{"on" if label == "4:3" else ""}">{label}</button>' for label in presets)
        return f"""
  <div class="bar"><div class="seg">{chips}</div><span class="sep"></span>
    <button class="btn ghost" {tip("crop.reset")}>{ph("arrow-counter-clockwise")}{"" if small else T("crop.reset")}</button></div>"""
    label = (lambda key, new=None: "") if small else T
    return f"""
  <div class="bar">
    <button class="btn ghost" {tip("tip.add_zone")}>{ph("eye-slash")}{label("strip.add_zone")}</button>
    <button class="btn ghost on" {tip("strip.edit_zones")}>{ph("pencil-simple")}<span class="count">2</span></button>
    <span class="sep"></span>
    <span class="pad" {tip("tip.pad")}>{ph("arrows-horizontal")}</span>
    <div class="stepper"><button>{ph("minus")}</button><span>0.50 {T("unit.s")}</span><button>{ph("plus")}</button></div>
    <span class="sep"></span>
    <span class="kept">{ph("scissors")}<span class="bar-fill"><i style="width:62%"></i></span>{"62%" if small else T("strip.keep", pct=62, time="0:26.040")}</span>
    <button class="btn accent" {tip("tip.scan")}>{ph("arrows-clockwise")}{label("strip.rescan")}</button>
    <button class="ib" {tip("strip.off")}>{ph("prohibit")}</button>
  </div>"""
