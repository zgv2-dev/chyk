"""Keyboard-first overlays: command palette, shortcut sheet, close prompt, context menu, toast."""
from mock_base import FILE, T, kbd, ph

# (group, [(icon, key, English, shortcut keys)]): every command the palette and sheet list.
COMMANDS = [
    ("cmd.playback", "Playback", [
        ("play", "cmd.play", "Play / pause", ("Space",)),
        ("caret-left", "cmd.frame_back", "Previous frame", ("←",)),
        ("caret-right", "cmd.frame_next", "Next frame", ("→",)),
        ("clock-counter-clockwise", "cmd.back", "Back 5 seconds", ("⇧", "←")),
        ("clock-clockwise", "cmd.forward", "Forward 5 seconds", ("⇧", "→")),
        ("gauge", "cmd.faster", "Faster / slower", (".", ",")),
    ]),
    ("cmd.trim", "Trim", [
        ("sign-in", "cmd.set_in", "Set start here", ("I",)),
        ("sign-out", "cmd.set_out", "Set end here", ("O",)),
        ("arrow-line-left", "cmd.go_in", "Go to start", ("⇧", "I")),
        ("arrow-line-right", "cmd.go_out", "Go to end", ("⇧", "O")),
        ("selection-slash", "cmd.clear", "Clear selection", ("X",)),
    ]),
    ("cmd.edit", "Crop & Strip", [
        ("crop", "cmd.crop", "Crop", ("C",)),
        ("frame-corners", "cmd.preset", "Crop preset", ("1", "–", "6")),
        ("magic-wand", "cmd.strip", "Strip inactive frames", ("S",)),
        ("eye-slash", "cmd.zone", "Ignore an area", ("A",)),
        ("arrows-clockwise", "cmd.rescan", "Rescan", ("R",)),
        ("arrow-u-up-left", "cmd.undo", "Undo / redo", ("⌘", "Z")),
    ]),
    ("cmd.view", "View", [
        ("magnifying-glass-plus", "cmd.zoom", "Zoom video in / out", ("+", "−")),
        ("corners-in", "cmd.fit", "Fit video", ("0",)),
        ("arrows-out-line-horizontal", "cmd.timeline", "Zoom timeline", ("[", "]")),
        ("arrows-in-line-horizontal", "cmd.to_sel", "Timeline to selection", ("\\",)),
    ]),
    ("cmd.file", "File & export", [
        ("export", "cmd.export", "Export…", ("⌘", "S")),
        ("info", "cmd.info", "File info", ("⌘", "I")),
        ("folder-open", "cmd.open", "Open video…", ("⌘", "O")),
        ("x-circle", "cmd.close", "Close video", ("⌘", "W")),
        ("keyboard", "cmd.keys", "Keyboard shortcuts", ("?",)),
    ]),
]


def log_window():
    """Read-only ffmpeg log in its own OS window (drawn here over the editor): every command Chyk ran
    and what it printed, newest last. It closes with the main window, not the other way round."""
    def entry(at, kind, state, cmd, out=""):
        return (f'<div class="entry"><div class="eh"><em>{at}</em><span class="kind {kind}">{kind}</span>{state}</div>'
                f'<pre class="cmd"><b>$</b> {cmd}</pre>{out}</div>')
    ok = lambda s: f'<span class="st ok">{ph("check")}{s}</span>'
    src = "~/Movies/Shoots/rooftop_take3.mov"
    entries = [
        entry("14:02:11", "thumbnails", ok("12 × 0.04 s"),
              f"ffmpeg -v error -nostdin -ss 3.500 -i {src} -frames:v 1 -an -sn -vf scale=96:72:force_original_aspect_ratio=increase,crop=96:72 -f rawvideo -pix_fmt rgba -"),
        entry("14:02:30", "estimate", f'<span class="st err">{ph("x")}exit 1</span>',
              f"ffmpeg -hide_banner -nostdin -y -v error -ss 4.630 -to 7.130 -i {src} … -c:v hevc_videotoolbox -q:v 50 …/T/chyk-est.mkv",
              '<pre class="out err">[hevc_videotoolbox] Error: cannot create compression session: -12902\nFalling back to libx265</pre>'),
        entry("14:02:41", "estimate", ok("4 samples · 6.8 s · 1.46 MB/s"),
              f"ffmpeg -hide_banner -nostdin -y -v error -ss 4.630 -to 7.130 -i {src} … -c:v libx265 -crf 26 -preset medium …/T/chyk-est.mkv"),
        entry("14:03:02", "export", '<span class="st run"><i></i>64% · 1.96×</span>',
              f"ffmpeg -hide_banner -nostdin -y -v error -progress pipe:1 -nostats -stats_period 0.1 -ss 5.880 -to 18.480 -i {src} "
              "-vf crop=1208:906:356:87 -c:v libx265 -crf 26 -preset medium -maxrate 16021k -bufsize 32042k -tag:v hvc1 -c:a copy "
              "-map_chapters -1 ~/Movies/Shoots/rooftop_take3-edited.mp4",
              '<pre class="out">frame=452 fps=118.2 out_time=00:00:07.533 total_size=11796480 speed=1.96x progress=continue<span class="cur"></span></pre>'),
    ]
    kinds = ["all", "export", "scan", "estimate", "thumbnails"]
    seg = "".join(f'<button class="{"on" if k == "all" else ""}">{T("log." + k, k.capitalize())}</button>' for k in kinds)
    return f"""
<div class="termwin">
  <div class="tw-head"><div class="lights small"><i></i><i></i><i></i></div>
    <b>{T("log.title", "ffmpeg log")}</b><em>rooftop_take3.mov · {T("log.readonly", "read-only")}</em>
    <div class="grow"></div><div class="seg">{seg}</div>
    <button class="btn ghost">{ph("copy")}{T("meta.copy")}</button></div>
  <div class="tw-body">{"".join(entries)}</div>
</div>"""


def shortcuts():
    cols = ""
    for gkey, gname, items in COMMANDS:
        body = "".join(f'<div class="sc">{ph(i)}<span>{T(k, en)}</span>{kbd(*keys)}</div>' for i, k, en, keys in items)
        cols += f'<section><h5>{T(gkey, gname)}</h5>{body}</section>'
    return f"""
<div class="scrim"></div>
<div class="sheet">
  <div class="sheet-head"><b>{ph("keyboard")}{T("cmd.keys", "Keyboard shortcuts")}</b>
    <div class="filter">{ph("magnifying-glass")}<span>{T("cmd.filter", "Filter")}</span></div>{kbd("esc")}</div>
  <div class="cols">{cols}</div>
</div>"""


def confirm():
    chips = "".join(f'<span class="chip">{ph(i)}{t}</span>' for i, t in
                    [("scissors", "0:05.880 – 0:18.480"), ("crop", "4:3"), ("gauge", "1.5×")])
    return f"""
<div class="scrim"></div>
<div class="modal">
  <h3>{ph("warning")}{T("unsaved.title")}</h3>
  <p>{T("unsaved.list", "These edits haven't been exported:")}</p>
  <div class="chips">{chips}</div>
  <div class="actions">
    <button class="btn ghost">{T("common.cancel")}{kbd("esc")}</button>
    <div class="grow"></div>
    <button class="btn danger">{T("unsaved.close")}{kbd("⌘", "⌫")}</button>
    <button class="primary">{ph("export")}{T("export.button")}{kbd("⏎")}</button>
  </div>
</div>"""


def context_menu():
    def item(icon, key, en, keys, cls=""):
        return f'<div class="mi {cls}">{ph(icon)}<span>{T(key, en)}</span>{kbd(*keys)}</div>'
    return f"""
<div class="menu" style="left:420px;top:540px">
  {item("sign-in", "cmd.set_in", "Set start here", ("I",), "on")}
  {item("sign-out", "cmd.set_out", "Set end here", ("O",))}
  <hr>{item("arrows-in-line-horizontal", "cmd.to_sel", "Timeline to selection", ("\\",))}
  {item("selection-slash", "cmd.clear", "Clear selection", ("X",))}
  <hr>{item("image", "cmd.frame", "Save this frame…", ("⌘", "⇧", "E"))}
</div>"""


def toast():
    return f"""
<div class="toast">{ph("check-circle", True)}
  <div><b>{T("status.saved")} · 18.1 MB</b><span>rooftop_take3-edited.mp4 · 78% {T("toast.smaller", "smaller")}</span></div>
  <button class="btn ghost">{T("toast.reveal", "Show in Finder")}{kbd("⌘", "⇧", "R")}</button>
  <button class="ib">{ph("x")}</button>
</div>"""
