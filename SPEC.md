# Chyk technical spec

Chyk opens a video, trims, crops, strips inactive stretches or retimes it, and saves the result with
as little re-encoding as possible. It runs on macOS, Windows and Linux. Feature summary: `features.md`.
UI: the keyboard-first "Phosphor" design. The mockups in `design/concepts/` (`python3 design/concepts/build.py`
renders them) match the app: `docs/superpowers/specs/2026-11-11-keyboard-first-ui-design.md`.

## Stack

- **Rust nightly**, pinned in `rust-toolchain.toml`.
- **egui / eframe 0.36** with the `glow` (OpenGL) backend. eframe exposes the GL context and
  `get_proc_address`, which mpv's render API needs.
- **libmpv** (`libmpv2`) for playback: hardware decoding, exact seeks, frame stepping, any format mpv reads.
- **ffmpeg CLI** (`std::process::Command`) for filmstrip thumbnails, the strip scan and all exports.
  No ffprobe: mpv reports duration, size, rotation, aspect and codec.
- `rfd` (native dialogs), `egui-phosphor` (icons), `sys-locale` (OS language).

## Source layout

```
src/
  main.rs     eframe setup, window options, file (or `--restore` session) from argv
  app/        the editor window; state in mod.rs, one file per part:
              setup and look (frame, theme, style, buttons, primary, action_buttons, segmented, controls,
              focus, dropdown, marks, format),
              keys (keys/ table, commands, shortcuts, sheet/ the ? sheet), history (undo/redo),
              behavior (file, edit, view, scan, export, estimate, poll, session, save_frame),
              header and stage (header, header_modes, update_button, bars, strip_bar, stage, crop_view, zones),
              overlays (popover, pops, pop_export, pop_summary, modal, modal_chips, toast, trim_menu),
              status bar, start screen and recent files (status_bar, start, drop_box, legend, backdrop,
              recent_list, recent_store),
              ffmpeg log window (log_window, log_head, log_entry, log_text),
              transport and trim bar (deck, deck_controls, ruler, trim, trim_drag, trim_paint, trim_strip)
  meta.rs     file info for the info popover, read from mpv properties (helpers in meta/)
  player.rs   libmpv + GL render context; player/paint.rs: zoom-aware FBO, blit into the egui frame
  ffmpeg/     export arguments (args, cut, encode), runner with progress/cancel (run, graph), thumbnails,
              single frame (frame), command log (log, log/buffer)
  recent.rs   recent-files list and thumbnail cache; reveal.rs: show a file in the file manager
  strip/      activity scan (run), ignore-zone mask and padding math (pixels)
  crop.rs     crop rect math (normalized), even-pixel conversion
  zoom.rs     video and timeline zoom/pan math
  chrome.rs   in-header window controls and edge resizing (Windows/Linux); chrome/macos.rs
  i18n.rs     language detection, t!/tn! lookups
  update/     self-update: release check (check), download, swap and relaunch (swap), install kind
              and cache folder (paths)
  bundle.rs   Windows only: unpacks embedded libmpv + ffmpeg
assets/       icon, fonts, Linux .desktop, i18n/*.lang
scripts/      package-macos.sh, package-windows.ps1 (+ windows/), package-linux.sh, make-samples.sh,
              i18n-editor.py (+ i18n-editor/)
```

Every source file stays under 200 lines. Tests sit next to their module (`x/tests.rs` or `x/tests/`).

## Video rendering

- mpv renders into an FBO each UI pass (gating on mpv's update flag misses the frame after a paused seek),
  and an egui paint callback blits the visible part into the video rect.
- The FBO is sized to what the zoomed view needs, capped at 4096 px and the GL max texture size, so zoomed
  crop handles land on real source pixels.
- mpv's update and wakeup callbacks request repaints; nothing polls.
- Properties (`time-pos`, `pause`, size, aspect, rotation, `eof-reached`) are observed, not read per frame,
  because reads block while mpv decodes a seek. Seeks are exact; while one is in flight only the latest
  target is kept.

## Editing model

All rects are normalized to the displayed (rotated) frame: (0,0) top-left, (1,1) bottom-right.

- **Trim:** `(in, out)` seconds, minimum length 0.1 s. Playback stops at `out`; play at the end or outside
  the selection restarts from `in`. The trim bar shows 12 thumbnails for the visible timeline range,
  regenerated (one ffmpeg seek per tile, 4 at a time), debounced 140 ms. With a crop applied they show
  only the cropped part, regenerated when the Crop panel closes or the crop is removed. Holding a handle still for 0.45 s zooms the timeline 10×
  around it; releasing restores the previous view. Timeline zoom stops at a 0.25 s window.
- **Crop:** presets Free, Original, 16:9, 9:16, 1:1, 4:3. Aspect presets fit a new centered rect; Free keeps
  the current rect and only drops the lock. Export converts to even `[w, h, x, y]` pixels (yuv420).
- **Video zoom:** 1–16×, around the pointer or the center. While cropping, zoom and pan are clamped so the
  whole crop rect stays in view.
- **Speed:** one value, 0.25–4×, for preview (mpv `speed`) and export. Resets to 1× on open.
- **Close:** ⌘/Ctrl+W or the header × returns to the start screen; a running export keeps going.

## Strip inactive frames

- Scan: ffmpeg decodes the first video stream (`-skip_frame noref`: frames nothing references are
  skipped, same detections, ~25% faster; 2× cores decoder threads, max 16) at 10 fps, grayscale, scaled to about 320×180 pixels
  worth of area (320×180 for 16:9) but never above the source's own pixels.
  A pixel changed if its value moved by more than 10; a frame is active if at least 6 unmasked pixels changed.
  Ignore zones mask pixels out of the comparison. Each change keeps the sample before it.
- Keep list: active intervals expanded by the padding (seconds, 0 allowed), merged, clamped to the trim.
- While scanning, the scanner sends new or changed intervals every 100 ms; the timeline hatches the
  stretches already known to be cut (holding back the last `pad` before the scan front) and marks the
  front with a green line trailing a fading green wake whose length and glow pulse (~0.5 Hz). The
  line is animated per frame at the measured scan speed (EMA of the 100 ms reports), eased toward
  `last report + speed × elapsed`, capped at 0.2 s of scanning past the last report and never moving
  back; the hatching follows it, limited to what the scanner has confirmed. The same `Glide` drives
  the scan and export progress bars (ffmpeg reports every 100 ms via `-stats_period 0.1`; the lead
  is the measured report interval, at least 0.2 s). Bar fills are painted full-size and clipped to
  the progress width, so 0–2% is a sliver, not a rounded blob.
- With a crop applied, ffmpeg cuts it out before scaling, so the crop alone gets the pixel budget: a
  thin 982×86 strip of a 1080p frame is compared at 810×70 instead of the 163×14 it would be inside
  a whole-frame sample (where a ticking seconds digit changed too few pixels to count). Ignore zones
  are mapped into the crop. A scan remembers its crop: opening Strip after the crop changed rescans
  automatically.
- The scan covers the whole file once; trimming only changes which part of it is used. Results are kept in
  memory per file and set of ignored areas for the session, so turning Strip off and on, switching modes
  or reopening the file reuses them, and a running scan continues in the background. Only Rescan (or a
  change to the ignored areas, followed by Scan) scans again.

## Export

- **Stream copy** when nothing needs re-encoding (no crop, resize, strip, retime or precise trim, quality High):
  `-ss IN -to OUT -i SRC -map 0 -dn -c copy -avoid_negative_ts make_zero`. The start may snap to the
  previous keyframe.
- **Re-encode** otherwise: first video stream, all audio (copied unless filtered), subtitles when timing
  is preserved. Filters, in order: crop, scale, strip `select` plus a gap-closing `setpts`, retime `setpts=PTS/s`.
  - Strip timestamps shift each kept interval back by the time cut before it. Frames are not renumbered,
    so variable-frame-rate video (screen recordings) stays in sync with `aselect` + `asetpts=N/SR/TB` audio.
  - Strip exports don't decode what they cut: kept intervals are grouped into runs (gaps under 10 s
    always join; then the smallest gaps until at most `max_inputs` runs: 32, fewer for long paths so
    the repeated path fits Windows' command line). Each run is its own input, `-ss first -to last -i`,
    with its own select/setpts (and aselect when the source has audio, read from `ffmpeg -i`); `concat`
    joins them, then retime applies. On a 12-minute 1080p60 HEVC recording keeping 131 s: 50.6 s with
    one input, 11.3 s with 8, same 7879 frames (`cargo test real_file -- --ignored`, see its doc).
  - Retime audio uses chained `atempo` (each 0.5–2.0), pitch-preserving.
  - Strip and retime drop subtitles and chapters, since their timing no longer matches.
- **Never bigger than the source:** every re-encode adds `-maxrate`/`-bufsize` (1×/2×) at the
  source's video bitrate (stream, else file, read from `ffmpeg -i`), and re-encoded audio never goes
  above the source's. A capped output that still ends up bigger than the source (encoders whose
  constant-quality modes ignore `-maxrate`, e.g. QSV, AMF) is redone once at a plain bitrate scaled
  to fit. On the 776 kb/s test recording a full-frame re-encode went from ~3 Mb/s to ~520 kb/s.
  VideoToolbox gets no cap: under a rate limit it ignores `-q:v` (on a 107 Mb/s 4K recording every
  quality came out the same ~25 Mb/s), so it relies on the redo alone.
- **Quality:** High / Medium / Low / Compress map to per-encoder settings (CRF, CQ, `-q:v`, or fixed
  bitrate). Compress uses CRF 32 (x264) / 34 (x265) with `-preset slow`, AAC 96 kb/s, and tries software
  encoders first.
- **Resolution:** Original, 4K, 2K, 1080p, 720p, 480p, 360p, 144p set the short side of the cropped
  frame (2160/1440/1080/…), so portrait video scales the same; aspect kept, both sides even, `scale=W:H`.
  A preset equal to the frame size is a no-op and doesn't block stream copy. Larger presets upscale.
- **Size estimate** (≈ in the popover and next to each resolution): a stream copy is the source size
  times the kept share of its duration. A re-encode is measured: 300 ms after the settings last
  changed, 4 samples of 2.5 s spread over the kept time are encoded in the background like the export
  (same arguments and encoder, a seek per sample), and their bytes per second scale to the output
  length; a change of settings stops a running measurement. Other resolutions scale the measured one
  by (pixels ratio)^0.75 (0.71–0.79 measured from 4K to 144p). On a 4K screen recording at 1080p the
  guess was within 1% for every quality over its first minute and 5% over the whole 5 minutes, after
  ~10 s of sampling (`cargo test real_file_estimate -- --ignored`, see its doc).
- **Encoders:** parsed once from `ffmpeg -encoders`. HEVC sources try HEVC encoders first, then H.264:
  VideoToolbox, NVENC, QSV, AMF, libx264/libx265, Media Foundation, libopenh264. A failing encoder falls
  through to the next. HEVC in MP4/MOV gets `-tag:v hvc1`.
- **Containers:** MP4, MOV, MKV (default: the source's). MKV→MP4/MOV converts text subtitles to
  `mov_text` and drops attachments; MP4→MKV converts to SRT.
- Interval sums (`select`, `aselect`, and the gap shifts in `setpts`) are written as balanced trees,
  `(a+b)+(c+d)`: ffmpeg's expression parser rejects a flat chain past 100 terms. Error messages keep
  ffmpeg's last 3 stderr lines, each cut to 160 characters; the error toast wraps within the window.
- **Limits:** no cap on kept segments. A filter graph over 24 000 characters goes in a temp file
  (`-/filter_complex`, ffmpeg 7+) so Windows' 32 KiB command line holds; tested with 3000 segments.
  Speed is validated as 0.25–4×.
- **Safety:** the save dialog defaults to `<name>-edited|compressed|stripped[-2x].<ext>` next to the source;
  the source itself is refused. Progress comes from `-progress pipe:1`; cancel kills ffmpeg and deletes the
  partial file, as does a failure. Errors show ffmpeg's last three stderr lines.

## UI

Design: `docs/superpowers/specs/2026-11-11-keyboard-first-ui-design.md`. ⌘ means Ctrl on Windows/Linux.

- Palette: black canvas with a faint green center and a dot grid; panels `#030805`, controls `#07130c`,
  hairlines `#12301f`, phosphor green `#4dff9b` for everything active, red `#ff5c78` for cuts and ignored
  areas. Every button is drawn by one painter (`button`/`Look`), so states look the same everywhere.
  macOS tags the window sRGB, so colors (UI and video) are converted to the display like a browser does,
  instead of landing oversaturated on wide-gamut panels.
- App icon: the green trim box cut in two, its halves tilted apart, with white scissors in the cut
  ("chyk" is the sound of scissors). `design/icon/snip.py`; `design/icon/build.py` renders it into
  `assets/chyk.png` and `assets/chyk.ico`.

### Keys

One table (`app/keys/table.rs`) feeds the key handler (`commands`), the key caps, the tooltips and the `?`
sheet, so they can't disagree. A single key never fires while a text field has focus. Bindings with more
modifiers are matched first (⇧I before I); Shift and Alt must match exactly, except for `?`.

| Group | Keys |
|---|---|
| Playback | Space play/pause · ←/→ frame · ⇧←/⇧→ ±5 s · `,` `.` speed (with or without ⇧) |
| Trim | I / O set start / end at the playhead · ⇧I / ⇧O go to start / end · X clear the selection (whole file) |
| Crop | C open/close the panel · 1–6 presets · ⇧C remove crop |
| Strip | S open/close the panel (turns Strip on) · A add an ignored area · Z edit areas · ⌫ / Del delete the selected area · R scan/rescan · ⇧S turn Strip off |
| Edit | ⌘Z undo · ⇧⌘Z redo (also Ctrl+Y on Windows/Linux) |
| View | `+` `=` `-` `0` video zoom · `[` `]` timeline zoom · `\` timeline to the selection · ⇧`\` whole timeline |
| File | ⌘O open · ⌘S export settings, again saves · ⌘I info · ⌘W close video · ⌘⇧E save the frame at the playhead · `?` shortcut sheet · ⌘⇧R show the last saved export or frame in the file manager · ⌘L ffmpeg log · ⌘1–⌘5 open a recent file (start screen) |
| Export popover | P precise trim · M remove audio · Esc cancel · ⌘S save · Tab/⇧Tab move focus · ←/→ change a focused segmented control |
| Dialogs | ⏎ the safe primary action · ⌘⌫ the action that discards work · Esc cancel |

- Key caps are one shared painter (header buttons, tooltips, popover, dialogs, sheet, context menu, start
  legend, status bar): ⌘ ⇧ ⌥ on macOS, Ctrl/Shift/Alt elsewhere. Every control with a key has a tooltip
  with its caps; no icon-only control lacks a tooltip. Padding, the update button and secondary dialog
  buttons have no key (Tab reaches them).
- `?` sheet: overlay with every binding in three columns, a filter that matches the English or the
  current-language name, Esc or `?` closes. Rows are one per command; Shift variants and `=` beside `+`
  fold into the first chord.
- Focus: Tab and ⇧Tab move a green focus ring. Popovers and dialogs (`egui::Modal`) trap Tab inside; Space
  and arrows go to a focused popover control instead of the player. ⏎ in a dialog answers the primary
  button only when no button has focus.

### Layout

- Top to bottom: header (52 pt), stage, transport row, ruler and trim bar, status bar (26 pt).
  Nothing sits on the video. Opening or closing Crop or Strip never moves the stage.
- Header: mark (equal space on both sides), Crop/Strip with key caps (C, S) and a green dot when an effect
  is applied with its panel closed, then one slot showing the file (name, codec · size · length, info ⌘I,
  close ⌘W) or, while a panel is open, its bar (crop ratios and Remove crop; strip: Ignore area, Edit
  areas with count, padding stepper, kept share, Rescan, Turn off), then the update button and Export
  (⌘S caps). Panel bars carry no caps; their keys are in tooltips. Opening one panel closes the other;
  closing a panel (its button, Esc) keeps its effect.
- Compact rules: each header section and the transport row measure their full width when drawn; if it
  doesn't fit, the frame is discarded and redrawn compact (strip: icon-only buttons, short bars, no total;
  transport: no total time, no zoom-fit button). Key caps show by measured slack in the header's middle,
  with 24 pt of hysteresis so they don't flicker at the boundary; the file facts give way first. The
  update button is icon-only beside a panel bar or when short of room.
- While exporting, Export becomes a fixed-width pill (so the header doesn't jitter) with percent, time
  left (`—` until enough is done to tell) and cancel; its caps are dropped from the width budget.
- Small windows (minimum 980 × 460 pt): under 520 pt tall the trim bar is 34 pt instead of 52 pt; the
  ruler keeps its height so its labels clear the transport row.
- Deck: play, ±5 s, timecode, speed pill (no chevron, menu as wide as the pill, 22 pt rows), video zoom,
  then IN / OUT / LEN as labeled fields (text labels, ms precision), then timeline zoom. Every group is
  `DECK_H` (30 pt) tall on one center line.
- Time ruler above the trim bar: minor and labeled major ticks (`m:ss` / `h:mm:ss`, one fixed decimal
  count per step) from the first candidate step whose major ticks sit at least 60 pt apart; minor ticks
  under 8 pt apart merge into major. It follows timeline zoom. The playhead carries its timecode.
- A zoomed timeline shows a thin scrollbar in the margin under the trim bar (above the status bar): thumb
  = visible range, tick = playhead; pressing beside the thumb centers the view, dragging moves it; hover
  for zoom % and range. Scrolling over the trim bar: vertical zooms around the pointer, horizontal pans;
  each event uses only its larger axis.
- Scrolling over the video zooms around the pointer (`exp(0.0015 · Δy)`, ~1.2× per notch), like pinch and
  ⌘/Ctrl+wheel. With the Crop panel open the whole frame shows with the selection box; once closed the
  preview frames only the cropped part.
- Trim context menu (right-click on the trim bar): Set start here, Set end here (the clicked time, not the
  playhead; taken on the click's own frame since the pointer moves before a row is chosen), Timeline to
  selection, Clear selection, Save this frame… (the clicked time). Rows show their keys.

### Status bar and toasts

- Status bar, left to right: ffmpeg log button (⌘L; a dot while any ffmpeg runs), the state (`Ready ·
  Lossless copy`, `Ready · Re-encode · <reason>` naming what forces it, `Scanning 42%`, `Exporting <name> ·
  64% · 0:12 left`), then `? Shortcuts` and the version. The start screen shows log, version and credit,
  `? Shortcuts`.
- Toasts replace the old bottom-center pill: bottom-right above the status bar, icon, message, optional
  action, ×, at most 520 pt wide and never wider than the window. Confirmations fade out after 4 s (400 ms
  fade); errors (red, ffmpeg's tail) stay until dismissed. The saved toast shows the size and how much
  smaller, plus "Show in Finder" / "Show in Explorer" / "Show in folder" (⌘⇧R); ⌘⇧R reveals the last
  saved export or frame. Reveal: macOS `open -R`, Windows `explorer /select,"path"` (argument passed raw,
  Explorer parses its own line), Linux `xdg-open` on the folder (no portable "select").

### Dialogs and popovers

- Closing the window, quitting (⌘Q is pointed at the window's `performClose:`, so it asks too) or closing
  the video with edits (trim, crop, strip with areas and padding, speed) that differ from the last load or
  successful export asks. The dialog lists only what changed, as chips (trim range, crop preset or Free,
  Strip with area count, speed). Export (⏎, opens the popover), Close/Quit anyway (⌘⌫), Cancel (Esc).
- Opening a file (drop or ⌘O) while a video is open asks: Open in new window (⏎; inside an AppImage the
  `$APPIMAGE` file is relaunched), Replace current (⌘⌫; its edits are lost), Cancel (Esc).
- Popovers (export settings, file info) hang from their button over a dimmed window; Esc or a click outside
  closes them. ⌘S opens export settings, and pressing it again saves. Export settings are label/control
  rows (Format, Quality, Resolution) with one equal-width segmented control each; a one-line note under
  Quality says what it means and which encoder will run (e.g. VideoToolbox, x264); toggles show their
  keys (P, M); a summary gives output length, ≈ size and `Lossless copy` or `Re-encode · codec`; footer
  Cancel (Esc) and Save as… (⌘S). On short windows the rows scroll between the fixed title and footer. The
  resolution list opens over its box with the chosen row on it; Esc closes the list first.
- Export progress and the done sound are handled in `App::logic`, which eframe still runs while the window
  is covered or minimized (it skips `ui` then), so the Dock badge / taskbar bar keep moving.

### Start screen

- A few streams of digital rain (deterministic in time, ~30 fps, only on this screen) fade out behind the
  content. Content: drop target (dashed border, no corner brackets, glows when a file is held over it,
  grays out for a non-video) with an Open video… button (⌘O), the Recent list, and a labeled key legend.
  The update button sits at the title bar's end.
- Recent list: up to 5 existing files (existence checks cached), 46 pt rows with thumbnail, name,
  codec · size · length and when opened; ↑/↓ move, ⏎ or click opens, ⌘1–⌘5 open directly. A file without a
  cached thumbnail shows a film icon.

### Undo, recent files, frame save, log

- Undo/redo (`app/history`) covers trim (in/out), crop (rect, preset, remove), Strip on/off, ignored
  areas (add, move, resize, delete, clear), padding and speed; not playback, zoom, open panels or export
  settings. Edits are recorded at rest: a drag, typed value or scrub ends in the first frame that sees a
  different state, so each gesture is one step. 100 steps; the history is cleared when another file opens;
  not available while a file loads (it still belongs to the previous one).
- Recent files (`src/recent.rs`): the last 10 opened, one line each `opened\tdur\tWxH\tcodec\tpath` (unix
  seconds; broken lines skipped) in `~/Library/Application Support/Chyk/recent`, `%APPDATA%\Chyk\recent`
  or `~/.config/chyk/recent`; newest first, one per path; written beside and renamed so a crash can't lose
  the list. The first filmstrip tile is cached as the thumbnail (raw RGBA named by a hash of the path, in
  the update cache folder's `recent/`) when the tiles cover the whole file and no crop is applied.
- Save this frame (⌘⇧E, or the menu row): one PNG of the chosen frame as previewed (crop applied, no
  scaling) via ffmpeg `-ss T -frames:v 1` (`ffmpeg/frame.rs`), run on a thread. The dialog defaults to
  `<name>-<h-m-s.ms>.png` next to the source; the source can't be overwritten; a failed save deletes the
  partial file.
- ffmpeg log (`ffmpeg/log.rs`, `log/buffer.rs`): every ffmpeg Chyk spawns goes through `ffmpeg::command`
  and is logged, including the thumbnails whose stderr was discarded before. An entry is one job (a
  filmstrip refresh with its tiles, scan, size estimate with its samples and encoder fallbacks, export
  with fallbacks and the oversize redo, frame save), holding every command it ran with its result; encoder
  and source probes (`-encoders`, `-version`, `ffmpeg -i`) are kind "probe". Limits: 200 entries, 200
  output lines per run, 1000 characters per line (rest becomes `…`); a new filmstrip refresh replaces the
  previous finished one. Exports show live progress (the latest line, not every one).
- The log shows in a separate deferred OS window (egui viewport) "Chyk — ffmpeg log", toggled by the
  status bar button or ⌘L. Read-only, selectable, with a kind filter (kept when closed) and Copy of the
  visible entries. It draws on its own passes at most every 100 ms for log changes, and only when
  something changed. Closing the main window closes it; closing it turns the button off. Background
  threads (mpv, export, thumbnails, scan, frame save, estimate, updater, Wayland drop) wake the main
  window with `crate::wake`, which repaints the root viewport: `Context::request_repaint` wakes whichever
  viewport is mid-pass, so with the log open a wake that landed during a log pass went to the log, and
  the main window fell to 20–47 fps with 235 ms gaps during playback (56–60 fps with `wake`).

## Updates

Design: `docs/superpowers/specs/2026-11-06-updater-design.md`.

- On launch (release builds only) a background thread runs the system `curl` against
  `releases/latest`: the redirect names the newest tag, and a `HEAD` on this system's release file
  confirms it exists and gives its size. Offline or without curl: nothing shows.
- A header button left of Export (on the start screen, at the title bar's end), Export's height,
  label over the version tag: Update available → progress chip with cancel → Update now.
  On a Linux .deb/.rpm/AUR install it opens the release page instead of downloading.
- Downloads go to the cache (`~/Library/Caches/Chyk/update/`, `%LOCALAPPDATA%\Chyk\update\`,
  `~/.cache/chyk/update/`) as `.part`, renamed when complete, and survive restarts.
- Update now: swap (macOS: mount the DMG, `codesign --verify` the app, rename bundles; Windows:
  rename the running exe aside; AppImage: rename over), write the session file, relaunch with
  `--restore`, close without the unexported-changes prompt. A finished download is also swapped in
  silently when Chyk quits (`on_exit`).
- Never downloads or installs without a click on the button.

## Window chrome

- macOS: full-size content view with a transparent titlebar. The native traffic lights are moved to the
  vertical center of the 52 pt header each frame (titlebar container resized, buttons repositioned via
  objc2), since AppKit resets them on every relayout. The window is not `movable`: AppKit would move it
  on any drag in the top ~28 pt before egui sees it (stealing drags from header controls); empty
  header space moves it through egui's `StartDrag` instead.
- Windows/Linux: undecorated window. Chyk draws minimize/maximize/close on the side the desktop uses
  (GNOME `button-layout`, KDE `kwinrc`, default right) and handles 5 px edge resizing.
- The whole header drags the window (controls on it keep their clicks); double-click maximizes.

## Languages

- Text lives in `assets/i18n/en.lang` and `uk.lang` (`key = value`, `{name}` placeholders, plural keys
  `.one/.few/.many/.other`) and is compiled in. Ukrainian when the OS locale is `uk*`, else English;
  `CHYK_LANG=uk|en` overrides. Missing or empty keys fall back to English.
- `python3 scripts/i18n-editor.py` edits both files in the browser with a live mockup preview
  (127.0.0.1 only, same-origin saves, writes only the two `.lang` files).
- Fonts: Space Mono for English, Fira Code for Ukrainian (Space Mono has no Cyrillic) and for Cyrillic
  file names in the English UI. Phosphor icons come right after the text fonts in the fallback chain,
  ahead of egui's emoji font, which reuses some of Phosphor's code points.

## Packaging

| OS | Script | Output | libmpv | ffmpeg |
|----|--------|--------|--------|--------|
| macOS | `scripts/package-macos.sh [version]` | `dist/Chyk.app` + drag-to-Applications `dist/Chyk-macos-<arch>.dmg` (background from `design/dmg/build.py`, laid out by Finder via AppleScript) | Homebrew dylibs via `dylibbundler`, ad-hoc signed | Homebrew, next to the binary |
| Windows | `scripts/package-windows.ps1` | single `dist/Chyk-windows-x64.exe` | shinchiro DLL embedded by `build.rs` | BtbN static, embedded |
| Linux | `scripts/package-linux.sh` | `dist/Chyk-linux-<arch>.AppImage` (portable), `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE); Arch: `packaging/arch/PKGBUILD` (AUR `chyk-bin`) | AppImage: bundled by linuxdeploy; packages: the distro's | AppImage and .rpm: BtbN static; .deb and Arch: the distro's |

- Release file names carry no version, so `releases/latest/download/<name>` always serves the newest
  build. The landing page (`site/`, published by `.github/workflows/pages.yml`) links to those URLs.
- Windows unpacks the embedded files to `%LOCALAPPDATA%\Chyk\<build-id>` on first run.
- ffmpeg is found next to the Chyk binary, else on PATH.
- CI (`.github/workflows/ci.yml`) tests and packages on all three OSes for every push, and publishes a
  GitHub release for `v*` tags.
- Licensing: Chyk is GPL-3.0-or-later (`LICENSE`). The ffmpeg builds are GPL (for libx264); ffmpeg runs as a separate process and its source must
  be offered alongside. libmpv is LGPL and linked dynamically.

## Testing

`cargo test` runs the pure math tests (crop, zoom, padding, plural rules), argument-builder tests, i18n file
checks, and ffmpeg round trips that need `ffmpeg`/`ffprobe` on PATH: copy and crop exports, compress size,
strip timing (including variable frame rate), retime durations, scan accuracy, and ignore zones.
UI logic is tested without a window: the key table and matching, undo history, dialog keys and chips,
header caps hysteresis, ruler steps, status bar text, recent store, log buffer and text, and the
file-manager commands.

## Known limits

- Bitmap subtitles (PGS, VobSub) can't be converted to MP4/MOV, so such exports fail.
- Quitting during an export can leave a partial file.
- No rotate/flip yet; no smart-cut (frame-exact lossless trims).
