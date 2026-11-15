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


def stepper(value):
    return f'<span class="stepper"><button>{ph("minus")}</button><span>{value}</span><button>{ph("plus")}</button></span>'


def editing():
    sens = [T("settings.sens_" + k) for k in ("low", "normal", "high")]
    return [
        group("settings.playback", [
            row("clock-clockwise", "settings.skip", "settings.skip_hint", stepper("5 s")),
            row("sign-out", "settings.stop_out", "settings.stop_out_hint", switch(True)),
            row("cpu", "settings.hwdec", "settings.hwdec_hint", switch(True)),
        ]),
        group("cmd.strip", [
            row("brackets-curly", "settings.pad", "settings.pad_hint", stepper("0.5 s")),
            row("pulse", "settings.sensitivity", "settings.sensitivity_hint", seg(sens, 1)),
        ]),
    ]


def about():
    link = lambda icon, key, url: row(icon, key, None, f'<span class="url mono">{url}{ph("arrow-up-right")}</span>')
    hero = f"""<div class="card hero"><img class="mark" src="assets/mark.png"><div class="hero-text">
      <span class="wordmark">C H Y K</span><em>{T("settings.made_by", name="zgv2")}</em></div>
      <div class="grow"></div><span class="ver mono">v1.3.2</span></div>"""
    comp = lambda icon, name, ver: (f'<div class="srow"><span class="what">{ph(icon)}<span>{name}'
                                    f'<em>{T("settings.component_hint")}</em></span></span>'
                                    f'<span class="ctl mono dimv">{ver}</span></div>')
    return [
        f'<section class="sgroup">{hero}</section>',
        group("settings.components", [
            comp("film-reel", "ffmpeg", "7.1.1"),
            comp("play-circle", "libmpv", "0.40.0"),
        ]),
        group("settings.links", [
            link("globe", "settings.website", "chyk.zgv2.dev"),
            link("github-logo", "settings.source", "github.com/zgv2-dev/chyk"),
            link("bug", "settings.report", "github.com/zgv2-dev/chyk/issues"),
            row("scales", "settings.license", None, '<span class="url mono">GPL-3.0-or-later</span>'),
        ]),
    ]


def settings_window(page="general"):
    nav = "".join(
        f'<button class="{"on" if key == page else ""}">{ph(icon)}{T("settings." + key)}</button>'
        for icon, key in PAGES)
    body = {"general": general, "export": export, "editing": editing, "about": about}[page]()
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
    <div class="sw-page {page}">
      <div class="sw-title"><b>{ph(icon)}{T("settings." + page)}</b><div class="grow"></div>
        {"" if page == "about" else f'<button class="btn ghost">{ph("arrow-counter-clockwise")}{T("settings.reset")}</button>'}</div>
      {"".join(body)}
    </div>
  </div>
</div>"""
