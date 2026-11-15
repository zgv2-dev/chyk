"""Settings in their own OS window, laid out like Windows Terminal's: categories on the left, the
chosen page on the right as cards of rows (what it does on the left, its control on the right).
Changes apply right away, so there's no Save bar; the settings file is one click away."""
from mock_base import T, kbd, ph
from mock_popovers import seg

PAGES = [("gear-six", "general"), ("export", "export"), ("scissors", "editing"), ("info", "about")]


def switch(on):
    return f'<span class="switch {"on" if on else ""}"><i></i></span>'


def select(text, extra=""):
    return f'<button class="select">{text}{extra}{ph("caret-up-down")}</button>'


def row(icon, key, hint, control, hint_args=None):
    hint = T(hint, **(hint_args or {})) if hint else ""
    return (f'<div class="srow"><span class="what">{ph(icon)}<span>{T(key)}<em>{hint}</em></span></span>'
            f'<span class="ctl">{control}</span></div>')


def group(key, rows, hint=None):
    note = f'<p class="gnote">{T(hint)}</p>' if hint else ""
    return f'<section class="sgroup"><h4>{T(key)}</h4>{note}<div class="card">{"".join(rows)}</div></section>'


def general():
    lang = T("settings.lang_system", lang="English")
    return [
        group("settings.interface", [
            row("translate", "settings.language", "settings.language_hint", select(lang)),
        ]),
        group("settings.opening", [
            row("copy-simple", "settings.another", "settings.another_hint",
                seg([T("settings.ask"), T("open.replace"), T("open.new_window")], 0)),
            row("cursor-click", "settings.explorer", "settings.explorer_hint", switch(True)),
        ]),
        group("settings.updates", [
            row("arrow-circle-up", "settings.check_updates", "settings.check_updates_hint", switch(True)),
            row("seal-check", "settings.version", "settings.up_to_date",
                f'<button class="btn">{ph("arrows-clockwise")}{T("settings.check_now")}</button>',
                ) .replace("{version}", "1.3.2"),
        ]),
    ]


def export():
    quality = [T("quality." + q) for q in ("high", "medium", "low", "compress")]
    example = '<b class="mono">rooftop_take3-edited.mp4</b>'
    return [
        group("settings.defaults", [
            row("file-video", "export.format", None, seg(["MP4", "MOV", "MKV"], 0)),
            row("sliders-horizontal", "export.quality", "quality.medium_hint", seg(quality, 1)),
            row("frame-corners", "export.resolution", None, select(T("settings.res_original"))),
            row("crosshair", "export.precise", "export.precise_hint", switch(False)),
            row("speaker-slash", "export.mute", "export.mute_hint", switch(False)),
        ], hint="settings.defaults_hint"),
        group("settings.saving", [
            row("folder-open", "settings.save_to", None,
                seg([T("settings.save_video"), T("settings.save_last")], 0)),
            row("textbox", "settings.suffix", "settings.suffix_hint",
                '<span class="field mono">-edited</span>', {"example": example}),
            row("bell-simple", "settings.sound", "settings.sound_hint", switch(True)),
        ]),
    ]


def settings_window(page="general"):
    nav = "".join(
        f'<button class="{"on" if key == page else ""}">{ph(icon)}{T("settings." + key)}</button>'
        for icon, key in PAGES)
    body = {"general": general, "export": export}[page]()
    icon = dict((k, i) for i, k in PAGES)[page]
    return f"""
<div class="setwin">
  <div class="tw-head"><div class="lights small"><i></i><i></i><i></i></div>
    <b>{T("settings.title")}</b><em>{T("settings.instant")}</em><div class="grow"></div>{kbd("⌘", ",")}</div>
  <div class="sw-main">
    <nav class="sw-nav">
      <div class="filter">{ph("magnifying-glass")}<em>{T("settings.search")}</em></div>
      {nav}
      <div class="grow"></div>
      <button class="sw-file">{ph("file-code")}{T("settings.open_file")}</button>
    </nav>
    <div class="sw-page">
      <div class="sw-title"><b>{ph(icon)}{T("settings." + page)}</b><div class="grow"></div>
        <button class="btn ghost">{ph("arrow-counter-clockwise")}{T("settings.reset")}</button></div>
      {"".join(body)}
    </div>
  </div>
</div>"""
