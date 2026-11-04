# Glint technical spec

Glint opens a video, trims, crops, strips inactive stretches or retimes it, and saves the result with
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
  main.rs     eframe setup, window options, file from argv
  app.rs      state, shortcuts, theme and widgets, header, floating bars, popovers, deck, trim bar
  meta.rs     file info for the info popover, read from mpv properties
  player.rs   libmpv + GL render context, zoom-aware FBO, blit into the egui frame
  ffmpeg.rs   export arguments, encoder fallback, runner with progress/cancel, thumbnails
  strip.rs    activity scan, ignore-zone mask, padding math
  crop.rs     crop rect math (normalized), even-pixel conversion
  zoom.rs     video and timeline zoom/pan math
  chrome.rs   in-header window controls and edge resizing (Windows/Linux)
  i18n.rs     language detection, t!/tn! lookups
  bundle.rs   Windows only: unpacks embedded libmpv + ffmpeg
assets/       icon, fonts, Linux .desktop, i18n/*.lang
scripts/      package-macos.sh, package-windows.ps1, package-linux.sh, make-samples.sh, i18n-editor.py
```

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
  regenerated in one ffmpeg run, debounced 140 ms. Holding a handle still for 0.45 s zooms the timeline 10×
  around it; releasing restores the previous view. Timeline zoom stops at a 0.25 s window.
- **Crop:** presets Free, Original, 16:9, 9:16, 1:1, 4:3. Aspect presets fit a new centered rect; Free keeps
  the current rect and only drops the lock. Export converts to even `[w, h, x, y]` pixels (yuv420).
- **Video zoom:** 1–16×, around the pointer or the center. While cropping, zoom and pan are clamped so the
  whole crop rect stays in view.
- **Speed:** one value, 0.25–4×, for preview (mpv `speed`) and export. Resets to 1× on open.
- **Close:** ⌘/Ctrl+W or the header × returns to the start screen; a running export keeps going.

## Strip inactive frames

- Scan: ffmpeg decodes the first video stream at 10 fps, scaled to 320 px on the long side, grayscale.
  A pixel changed if its value moved by more than 10; a frame is active if at least 6 unmasked pixels changed.
  Ignore zones mask pixels out of the comparison. Each change keeps the sample before it.
- Keep list: active intervals expanded by the padding (seconds, 0 allowed), merged, clamped to the trim.
- The scan covers the whole file once; trimming only changes which part of it is used. Results are kept in
  memory per file and set of ignored areas for the session, so turning Strip off and on, switching modes
  or reopening the file reuses them, and a running scan continues in the background. Only Rescan (or a
  change to the ignored areas, followed by Scan) scans again.

## Export

- **Stream copy** when nothing needs re-encoding (no crop, strip, retime or precise trim, quality High):
  `-ss IN -to OUT -i SRC -map 0 -dn -c copy -avoid_negative_ts make_zero`. The start may snap to the
  previous keyframe.
- **Re-encode** otherwise: first video stream, all audio (copied unless filtered), subtitles when timing
  is preserved. Filters, in order: crop, strip `select` plus a gap-closing `setpts`, retime `setpts=PTS/s`.
  - Strip timestamps shift each kept interval back by the time cut before it. Frames are not renumbered,
    so variable-frame-rate video (screen recordings) stays in sync with `aselect` + `asetpts=N/SR/TB` audio.
  - Retime audio uses chained `atempo` (each 0.5–2.0), pitch-preserving.
  - Strip and retime drop subtitles and chapters, since their timing no longer matches.
- **Quality:** High / Medium / Low / Compress map to per-encoder settings (CRF, CQ, `-q:v`, or fixed
  bitrate). Compress uses CRF 32 (x264) / 34 (x265) with `-preset slow`, AAC 96 kb/s, and tries software
  encoders first.
- **Encoders:** parsed once from `ffmpeg -encoders`. HEVC sources try HEVC encoders first, then H.264:
  VideoToolbox, NVENC, QSV, AMF, libx264/libx265, Media Foundation, libopenh264. A failing encoder falls
  through to the next. HEVC in MP4/MOV gets `-tag:v hvc1`.
- **Containers:** MP4, MOV, MKV (default: the source's). MKV→MP4/MOV converts text subtitles to
  `mov_text` and drops attachments; MP4→MKV converts to SRT.
- **Limits:** at most 300 kept segments per strip export (the filters must fit Windows' 32 KiB command
  line; tested). Speed is validated as 0.25–4×.
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
- macOS tags the window sRGB, so colors (UI and video) are converted to the display like a browser
  does, instead of landing oversaturated on wide-gamut panels.
- App icon: the green trim box (`design/icon/`, rendered by `design/icon/build.py` into
  `assets/glint.png` and `assets/glint.ico`).
- Header: mark and file (codec, size, duration), info (⌘I) and close (⌘W), Crop/Strip toggles, Export.
- Crop and Strip are exclusive modes: turning one on turns the other off.
- Floating bars on the frame: crop ratios or strip controls at the top, video zoom at the bottom right.
  Status messages sit at the bottom center.
- Popovers (export settings, file info) hang from their button over a dimmed window; Escape or a click
  outside closes them. ⌘S opens export settings, and pressing it again saves. On short windows the
  settings scroll between the fixed title and Save button.
- Deck: play, ±5 s, timecode, speed, IN/OUT/LEN, timeline zoom, then the trim bar.

## Window chrome

- macOS: full-size content view with a transparent titlebar. The native traffic lights are moved to the
  vertical center of the 52 pt header each frame (titlebar container resized, buttons repositioned via
  objc2), since AppKit resets them on every relayout.
- Windows/Linux: undecorated window. Glint draws minimize/maximize/close on the side the desktop uses
  (GNOME `button-layout`, KDE `kwinrc`, default right) and handles 5 px edge resizing.
- The whole header drags the window (controls on it keep their clicks); double-click maximizes.

## Languages

- Text lives in `assets/i18n/en.lang` and `uk.lang` (`key = value`, `{name}` placeholders, plural keys
  `.one/.few/.many/.other`) and is compiled in. Ukrainian when the OS locale is `uk*`, else English;
  `GLINT_LANG=uk|en` overrides. Missing or empty keys fall back to English.
- `python3 scripts/i18n-editor.py` edits both files in the browser with a live mockup preview
  (127.0.0.1 only, same-origin saves, writes only the two `.lang` files).
- Fonts: Space Mono for English, Fira Code for Ukrainian (Space Mono has no Cyrillic) and for Cyrillic
  file names in the English UI. Phosphor icons come right after the text fonts in the fallback chain,
  ahead of egui's emoji font, which reuses some of Phosphor's code points.

## Packaging

| OS | Script | Output | libmpv | ffmpeg |
|----|--------|--------|--------|--------|
| macOS | `scripts/package-macos.sh [version]` | `dist/Glint.app` + drag-to-Applications DMG (background from `design/dmg/build.py`, laid out by Finder via AppleScript) | Homebrew dylibs via `dylibbundler`, ad-hoc signed | Homebrew, next to the binary |
| Windows | `scripts/package-windows.ps1 [-Version v]` | single `dist/Glint-<v>-windows-x64.exe` | shinchiro DLL embedded by `build.rs` | BtbN static, embedded |
| Linux | `scripts/package-linux.sh [version]` | `dist/Glint-<v>-linux-<arch>.AppImage` (portable) and `.deb` (Debian/Ubuntu) | AppImage: system `libmpv` bundled by linuxdeploy; .deb: depends on `libmpv2` | AppImage: BtbN static in `usr/bin`; .deb: depends on `ffmpeg` |

- Windows unpacks the embedded files to `%LOCALAPPDATA%\Glint\<build-id>` on first run.
- ffmpeg is found next to the Glint binary, else on PATH.
- CI (`.github/workflows/ci.yml`) tests and packages on all three OSes for every push, and publishes a
  GitHub release for `v*` tags.
- Licensing: the ffmpeg builds are GPL (for libx264); ffmpeg runs as a separate process and its source must
  be offered alongside. libmpv is LGPL and linked dynamically.

## Testing

`cargo test` runs the pure math tests (crop, zoom, padding, plural rules), argument-builder tests, i18n file
checks, and ffmpeg round trips that need `ffmpeg`/`ffprobe` on PATH: copy and crop exports, compress size,
strip timing (including variable frame rate), retime durations, scan accuracy, and ignore zones.

## Known limits

- Bitmap subtitles (PGS, VobSub) can't be converted to MP4/MOV, so such exports fail.
- Quitting during an export can leave a partial file.
- No rotate/flip yet; no smart-cut (frame-exact lossless trims).
