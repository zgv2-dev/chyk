"""Setup and uninstall: a small window Chyk.exe opens instead of the editor when it starts from
outside an install (e.g. Downloads). A rail on the left (mark, rain, steps), the page on the right,
buttons along the bottom. Options are the settings window's own switches, set to their first values."""
import random

from mock_base import T, ph
from mock_settings import switch

SIZE = (820, 520)
STEPS = ["welcome", "options", "install", "done"]
PATH = r"C:\Users\Olena\AppData\Local\Programs\Chyk"


def rain():
    glyphs = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉ0123456789:.=*+<>"
    rnd = random.Random(3)
    cols = []
    for c in range(6, 200, 18):
        if rnd.random() < 0.35:
            continue
        chars = "".join(f"<b>{rnd.choice(glyphs)}</b>" for _ in range(rnd.randint(5, 16)))
        cols.append(f'<span style="left:{c}px;top:{rnd.randint(-120, 360)}px;opacity:{rnd.uniform(.2, .7):.2f}">{chars}</span>')
    return f'<div class="su-rain">{"".join(cols)}</div>'


def rail(step, uninstall=False):
    if uninstall:
        steps = f'<li class="on"><i></i>{T("setup.uninstall", "Uninstall")}</li>'
    else:
        at = STEPS.index(step)
        steps = "".join(
            f'<li class="{"on" if i == at else "done" if i < at else ""}">'
            f'<i>{ph("check") if i < at else ""}</i>{T("setup.step_" + s)}</li>'
            for i, s in enumerate(STEPS))
    return f"""<aside class="su-rail">{rain()}
  <div class="su-brand"><img class="mark" src="assets/mark.png"><span class="wordmark">CHYK</span></div>
  <ol class="su-steps">{steps}</ol>
  <span class="su-ver mono">v1.3.2 · x64</span></aside>"""


def window(step, page, foot, uninstall=False):
    title = T("setup.title_uninstall" if uninstall else "setup.title", version="1.3.2")
    return f"""<div class="suwin">
  <div class="tw-head"><b>{title}</b><div class="grow"></div>
    <button class="cap">{ph("minus")}</button><button class="cap x">{ph("x")}</button></div>
  <div class="su-main">{rail(step, uninstall)}
    <div class="su-page {step}">{page}</div></div>
  <div class="su-foot">{foot}</div></div>"""


def primary(icon, key):
    return f'<button class="primary">{T(key)}{ph(icon)}</button>'


def welcome():
    page = f"""
  <h1>{T("setup.welcome")}</h1>
  <p class="specs mono">{T("setup.specs", version="1.3.2", size="182")}</p>
  <div class="loc"><span class="lbl">{T("setup.location")}</span>
    <span class="path-box mono">{PATH}</span>
    <button class="btn">{ph("folder-open")}{T("setup.change")}</button></div>"""
    foot = f"""<button class="btn ghost">{ph("lightning")}{T("setup.just_run")}</button>
  <div class="grow"></div>{primary("arrow-right", "setup.next")}"""
    return window("welcome", page, foot)


def options():
    def row(icon, key, on):
        return f'<div class="srow"><span class="what">{ph(icon)}<span>{T(key)}</span></span><span class="ctl">{switch(on)}</span></div>'
    page = f"""
  <h1>{T("setup.options")}</h1>
  <div class="card">
    {row("cursor-click", "settings.explorer", True)}
    {row("film-strip", "setup.assoc", False)}
    {row("desktop", "setup.desktop", False)}
    {row("arrow-circle-up", "settings.check_updates", True)}
  </div>"""
    foot = f"""<button class="btn ghost">{ph("arrow-left")}{T("setup.back")}</button>
  <div class="grow"></div>{primary("download-simple", "setup.install")}"""
    return window("options", page, foot)


def installing():
    log = [("ok", "setup.log_copy", {"path": r"…\Programs\Chyk\Chyk.exe"}),
           ("ok", "setup.log_start", {}),
           ("warn", "setup.log_menu_classic", {}),
           ("run", "setup.log_uninstall", {})]
    icon = {"ok": ph("check"), "warn": ph("warning"), "run": "<i class=\"dot\"></i>"}
    rows = "".join(f'<div class="ll {k}">{icon[k]}<span>{T(key, **a)}</span></div>' for k, key, a in log)
    page = f"""
  <h1>{T("setup.installing")}</h1>
  <div class="su-progress"><div class="track"><i style="width:82%"></i></div><span class="mono">82%</span></div>
  <div class="card su-log mono">{rows}</div>"""
    foot = '<div class="grow"></div><button class="btn" disabled>' + T("setup.cancel") + "</button>"
    return window("install", page, foot)


def done():
    page = f"""
  <div class="done-hero"><div class="ring">{ph("check")}</div>
    <h1>{T("setup.done")}</h1></div>
  <div class="card">
    <div class="srow"><span class="what">{ph("trash")}<span>{T("setup.delete_download")}
      <em class="mono">C:\\Users\\Olena\\Downloads\\Chyk-windows-x64.exe</em></span></span>
      <span class="ctl">{switch(True)}</span></div>
  </div>"""
    foot = f"""<button class="btn ghost">{T("setup.close")}</button>
  <div class="grow"></div>{primary("play", "setup.open")}"""
    return window("done", page, foot)


def uninstall():
    items = [("app-window", "setup.rm_app"), ("cursor-click", "setup.rm_menu"),
             ("film-strip", "setup.rm_assoc"), ("folder-simple", "setup.rm_cache")]
    rows = "".join(f'<div class="fact">{ph(i)}<span>{T(k)}</span></div>' for i, k in items)
    page = f"""
  <h1>{T("setup.uninstall_q")}</h1>
  <div class="card facts tight">{rows}</div>
  <div class="card"><div class="srow"><span class="what">{ph("sliders-horizontal")}<span>{T("setup.keep_settings")}</span></span><span class="ctl">{switch(True)}</span></div></div>"""
    foot = f"""<button class="btn ghost">{T("setup.cancel")}</button>
  <div class="grow"></div><button class="btn danger big">{ph("trash")}{T("setup.uninstall_btn")}</button>"""
    return window("uninstall", page, foot, uninstall=True)


def maintain():
    """Chyk is already installed: the welcome page becomes a choice. Reinstall is picked here."""
    def opt(key, sub, on):
        return (f'<div class="srow su-radio{" on" if on else ""}"><span class="what">{ph("radio-button", fill=on) if on else ph("circle")}'
                f'<span>{T(key)}<em>{T(sub)}</em></span></span></div>')
    page = f"""
  <h1>{T("setup.installed_title")}</h1>
  <p class="specs mono">v1.3.2 · {PATH}</p>
  <div class="card">
    {opt("setup.repair", "setup.repair_sub", False)}
    {opt("setup.reinstall", "setup.reinstall_sub", True)}
    {opt("setup.remove", "setup.uninstall_sub", False)}
  </div>
  <div class="card"><div class="srow"><span class="what">{ph("sliders-horizontal")}<span>{T("setup.keep_settings")}</span></span><span class="ctl">{switch(True)}</span></div></div>"""
    foot = f"""<button class="btn ghost">{T("setup.cancel")}</button>
  <div class="grow"></div>{primary("arrow-right", "setup.next")}"""
    return window("welcome", page, foot)


SCREENS = {"setup-maintain": maintain, "setup-welcome": welcome, "setup-options": options, "setup-installing": installing,
           "setup-done": done, "setup-uninstall": uninstall}
