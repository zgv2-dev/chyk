# Glint features

- **Open anything** — Drop a video on the window, click the drop target, press ⌘/Ctrl+O, or pass a path (Open With / file associations). Close the video (× next to its name, or ⌘/Ctrl+W) to return to the start screen. Plays whatever libmpv can decode, with hardware decoding.
- **Playback & speed** — Play/pause (Space, or click the video), frame step (←/→), and ±5 s skips. One speed setting, 0.25×–4× (`,` / `.`), applies to both the preview and the export, with pitch-corrected audio.
- **Trim** — Quick Look–style trim box on the filmstrip: drag the handles to resize, drag its top or bottom bar to move it, and hold a handle still to zoom in for frame-exact edges. You can also press I/O or type IN/OUT/LEN with millisecond precision; playback stops at the out point.
- **Timeline zoom** — Wheel over the filmstrip or `[` / `]` zooms down to a quarter second, horizontal scroll pans, and there are fit-all and fit-selection buttons. Off-screen trim edges stay grabbable at the view edge.
- **Crop** — Drag 8 handles or move the rect; presets Free, Original, 16:9, 9:16, 1:1, 4:3 (`C` toggles). Switching to Free keeps the current box and only drops the ratio lock; output is rotation-aware and rounded to even pixels.
- **Video zoom** — Pinch, ⌘/Ctrl+wheel, or `+` / `-` / `0` zooms up to 16× around the pointer, and drag pans. While cropping, zoom never goes past the crop box. The frame is rendered at source resolution, so crop edges land on real pixels.
- **Strip inactive frames** — Scans for on-screen change and cuts the still stretches, with seconds of padding kept around each change. Ignore zones (clocks, cursors) mask areas from the scan; the cut parts are shaded on the timeline.
- **File info** — ⓘ next to the file name (⌘/Ctrl+I) shows container, size, bitrate, codecs, frame rate, color and tracks, with a copy button.
- **Export** — ⌘/Ctrl+S or Export opens the settings; Save as… (or ⌘/Ctrl+S again) writes the file. MP4, MOV or MKV. A plain trim stream-copies losslessly in seconds; crop, strip, retime or lower quality re-encode with the best available hardware encoder, then fall back to software.
- **Quality & Compress** — High/Medium/Low, or Compress for the smallest file (slow x264/x265, AAC 96k), and it reports how much smaller the file got. Options: precise frame-exact trim, remove audio.
- **English & Ukrainian** — The interface follows the OS language: Ukrainian if the system is set to Ukrainian, English otherwise. Translations live in `assets/i18n/` and can be edited in the browser with a live preview (`python3 scripts/i18n-editor.py`).
- **Window controls in the header** — Native traffic lights on macOS; on Windows and Linux Glint draws minimize/maximize/close on the side your desktop uses, and you can drag or double-click the title area.
- **Safe saving** — Progress with cancel (partial files are deleted), ffmpeg's error tail on failure, and it never overwrites the source video.
- **Cross-platform builds** — macOS .app/DMG, a single-file Windows .exe with libmpv and ffmpeg embedded, and Linux as a portable AppImage or a .deb, all built by CI.
