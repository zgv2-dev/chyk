# Chyk technical spec

Chyk opens a video, trims, crops, strips inactive stretches or retimes it, and saves the result with
as little re-encoding as possible. It runs on macOS, Windows and Linux. Feature summary: `features.md`.
UI: the "Phosphor" design, matching the mockups in `design/concepts/` (`python3 design/concepts/build.py` renders them).

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
              setup and look (frame, theme, style, buttons, controls, dropdown, marks, format),
              behavior (file, edit, view, scan, export, estimate, poll, shortcuts, session),
              header and stage (header, update_button, bars, strip_bar, stage, crop_view, zones),
              overlays and start screen (popover, pops, modal, status, start, legend, backdrop),
              transport and trim bar (deck, deck_controls, trim, trim_drag, trim_paint, trim_strip)
  meta.rs     file info for the info popover, read from mpv properties (helpers in meta/)
  player.rs   libmpv + GL render context; player/paint.rs: zoom-aware FBO, blit into the egui frame
  ffmpeg/     export arguments (args, cut, encode), runner with progress/cancel (run, graph), thumbnails
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
- **Quality:** High / Medium / Low / Compress map to per-encoder settings (CRF, CQ, `-q:v`, or fixed
  bitrate). Compress uses CRF 32 (x264) / 34 (x265) with `-preset slow`, AAC 96 kb/s, and tries software
  encoders first.
- **Resolution:** Original, 4K, 2K, 1080p, 720p, 480p, 360p, 144p set the short side of the cropped
  frame (2160/1440/1080/…), so portrait video scales the same; aspect kept, both sides even, `scale=W:H`.
  A preset equal to the frame size is a no-op and doesn't block stream copy. Larger presets upscale.
- **Size estimate** (≈ in the popover and next to each resolution): a stream copy is the source size
  times the kept share of its duration; a re-encode also keeps 0.8 / 0.5 / 0.3 / 0.2 of the source
  bitrate for High / Medium / Low / Compress, times (output ÷ source pixels)^0.75, capped at the
  source. Audio is ignored.
- **Encoders:** parsed once from `ffmpeg -encoders`. HEVC sources try HEVC encoders first, then H.264:
  VideoToolbox, NVENC, QSV, AMF, libx264/libx265, Media Foundation, libopenh264. A failing encoder falls
  through to the next. HEVC in MP4/MOV gets `-tag:v hvc1`.
- **Containers:** MP4, MOV, MKV (default: the source's). MKV→MP4/MOV converts text subtitles to
  `mov_text` and drops attachments; MP4→MKV converts to SRT.
- Interval sums (`select`, `aselect`, and the gap shifts in `setpts`) are written as balanced trees,
  `(a+b)+(c+d)`: ffmpeg's expression parser rejects a flat chain past 100 terms. Error messages keep
  ffmpeg's last 3 stderr lines, each cut to 160 characters; the status pill wraps within the window.
- **Limits:** no cap on kept segments. A filter graph over 24 000 characters goes in a temp file
  (`-/filter_complex`, ffmpeg 7+) so Windows' 32 KiB command line holds; tested with 3000 segments.
  Speed is validated as 0.25–4×.
- **Safety:** the save dialog defaults to `<name>-edited|compressed|stripped[-2x].<ext>` next to the source;
  the source itself is refused. Progress comes from `-progress pipe:1`; cancel kills ffmpeg and deletes the
  partial file, as does a failure. Errors show ffmpeg's last three stderr lines.

## UI

- Palette: black canvas with a faint green center and a dot grid; panels `#030805`, controls `#07130c`,
  hairlines `#12301f`, phosphor green `#4dff9b` for everything active, red `#ff5c78` for cuts and ignored
  areas. Every button is drawn by one painter (`button`/`Look`), so states look the same everywhere.
- Start: a few streams of digital rain (deterministic in time, ~30 fps, only on this screen) falling
  around a clickable drop target (the only open control besides ⌘O),
  a two-row key-cap shortcut legend, and the version and credit on the bottom line.
- Closing the window, quitting (⌘Q is pointed at the window's `performClose:`, so it asks too) or
  closing the video with edits (trim, crop, strip with its areas and padding, speed) that differ from
  the last load or successful export asks: Cancel, Quit/Close anyway, or Export (opens the popover).
- Opening a file (drop or ⌘O) while a video is open asks first: replace it (its edits are lost), open
  it in a new Chyk process (inside an AppImage, the `$APPIMAGE` file is relaunched), or cancel.
- macOS tags the window sRGB, so colors (UI and video) are converted to the display like a browser
  does, instead of landing oversaturated on wide-gamut panels.
- App icon: the green trim box (`design/icon/`, rendered by `design/icon/build.py` into
  `assets/chyk.png` and `assets/chyk.ico`).
- Header: mark and file (codec, size, duration), info (⌘I) and close (⌘W), Crop/Strip toggles, Export.
- Crop and Strip buttons open panels; opening one closes the other, and closing a panel (its button
  again, Escape, or opening the other) keeps its effect. A green dot marks an applied effect whose
  panel is closed. Each bar ends with a button that removes its effect (reset crop, turn Strip off).
- With the Crop panel open the whole frame shows with the selection box; once closed, the preview
  frames just the cropped part (video zoom works inside it).
- Nothing sits on the video. Header: mark, Crop/Strip, then a slot that shows the file (name, facts,
  info, close) or, while a panel is open, its bar (crop ratios or strip controls), then Export. The
  strip bar and the transport row each measure their full width when drawn; if it doesn't fit, the
  frame is discarded and redrawn compact (strip: icon-only buttons, short bars, no total; transport:
  no total time, no zoom-fit button, the zoom % fits on click). Video zoom sits in the transport row.
- A zoomed timeline shows a thin scrollbar in the margin under the trim bar: the thumb is the visible
  range, a tick marks the playhead; pressing beside the thumb centers the view there, dragging moves
  it by the pointer's movement; hover for zoom % and range.
- Transport row: every group and button is `DECK_H` (30 pt) tall on one center line. The speed
  selector has no chevron; its menu is as wide (wider only if its longest entry needs it), with
  22 pt left-aligned rows.
- Scrolling over the trim bar: vertical zooms around the pointer, horizontal pans a zoomed view; each
  event uses only its larger axis, so trackpad swipes don't do both.
- Scrolling over the video zooms it around the pointer (`exp(0.0015 · Δy)`, ~1.2× per wheel notch),
  like pinch and ⌘/Ctrl+wheel.
  Status messages sit at the bottom center.
- Popovers (export settings, file info) hang from their button over a dimmed window; Escape or a click
  outside closes them. ⌘S opens export settings, and pressing it again saves. On short windows the
  settings scroll between the fixed title and Save button. The resolution list opens over its box with
  the chosen row on it, earlier options above and later ones below; Escape closes the list first.
- Deck: play, ±5 s, timecode, speed, IN/OUT/LEN, timeline zoom, then the trim bar.

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

## Known limits

- Bitmap subtitles (PGS, VobSub) can't be converted to MP4/MOV, so such exports fail.
- Quitting during an export can leave a partial file.
- No rotate/flip yet; no smart-cut (frame-exact lossless trims).
