# Keyboard-first Phosphor UI: design

Status: design, awaiting approval. Builder check done (2026-11-11); its questions are settled below
and marked in "Calls made without the user". Background and the review of 1.2.6: `2026-11-11-keyboard-first-ui-review.md`.
Visual reference: `design/concepts/` (`python3 design/concepts/build.py`, viewer `design/concepts/index.html`).
Where this text and the mockups disagree, this text wins. The mockups approximate sizes and use made-up data.

## Intent

The user wants Chyk to feel premium, the way Linear does: calm, precise, and fully usable from the
keyboard. They want to keep the Phosphor colors and overall look, not get a new brand. The review found
the current UI is consistent but: keys are missing or hidden, there's no shortcut reference in the
editor, labels and numbers look the same, the trim bar has no time scale, and some flows (close prompt,
save) give too little or too generic information.

## Goals

1. Every action a user repeats has a key (the table below is the complete list), and every key is
   discoverable: tooltips show it, a ? sheet lists them all.
2. A calmer, more legible hierarchy without changing the palette or the typeface.
3. New: status bar, ffmpeg log window, trim context menu, saved toast, specific close prompt, recent
   files, save this frame, undo/redo.
4. Works at the minimum window size (980 × 460 pt) on macOS, Windows and Linux.

## Non-goals

- No command palette (rendered and declined: too much for an app this size).
- No menu bar, settings window, light theme, or remappable shortcuts.
- No change to export behavior, encoders, the scan, or the app icon and header mark.
- No file menu on the file name (rendered with a caret and declined).

## Anti-goals (would make this a failure even if it works)

- Opening or closing Crop/Strip changes the stage's size or position (the header stays one height).
  The crop preview's own framing is unchanged: the whole frame while cropping, the cropped part after.
- A key fires while the user types in a field (time fields, padding, the sheet's filter).
- An existing shortcut stops working or changes meaning.
- The log or history slows playback, scrubbing or exports, or grows without bound.
- A second window disturbs mpv's rendering in the main window.

## Constraints

- egui/eframe 0.36, glow backend; mpv renders through the main window's GL context.
- Space Mono for all UI text (decided over a UI sans); Fira Code for Cyrillic as today.
- Palette and hand-drawn buttons stay (`button`/`Look`); new controls use the same painter.
- ⌘ on macOS means Ctrl on Windows/Linux everywhere below (egui `Modifiers::COMMAND`).
- Every new string goes into both `assets/i18n/en.lang` and `uk.lang` (Ukrainian written, not left empty).
- Source files stay under 200 lines; new logic gets tests (history, recent list, key map, log buffer).
- `SPEC.md` and `features.md` describe the result; the mockups stay the visual reference.

## Decided design

### Layout

- **Header** keeps its current height and structure: traffic lights (or drawn window buttons), mark,
  Crop | Strip, then one slot that shows the file (name, codec · size · length, info, close) or, while a
  panel is open, that panel's bar; then the update button and Export. The video never moves when a panel
  opens or closes. Equal spacing on both sides of the mark.
- **Crop/Strip buttons** show their key caps (C, S). A dot marks an effect that's applied while its
  panel is closed (as today).
- **Panel bars** carry no key caps (keys are in their tooltips). Crop bar: presets, Remove crop. Strip
  bar: Ignore area, Edit areas with the area count, padding icon and stepper, kept share, Rescan, Turn
  off. The existing measure-and-compact logic stays and applies to the whole header.
- **Update button**: as today (available → downloading with progress and cancel → Update now, label over
  the version tag), restyled; icon-only on small windows and while a panel bar is in the header.
- **Export button** shows ⌘S. While exporting, a progress pill with percent, time left and cancel.
- **Status bar** (new, ~26 pt, bottom edge): far left the ffmpeg log button (a dot while any ffmpeg runs);
  then the state: "Ready · Lossless copy" or "Ready · Re-encode · <reason>", "Scanning 42%",
  "Exporting <name> · 64% · 0:12 left"; at the right "? Shortcuts" and the version. On the start screen:
  log button, version and credit, "? Shortcuts".
- **Deck**: play, ±5 s, timecode (total hidden when compact), speed pill, video zoom group, then
  IN / OUT / LEN with text labels instead of icons, then the timeline zoom group. A **time ruler** above
  the trim bar: minor and labeled major ticks that adapt to the visible range when the timeline is
  zoomed. The playhead carries its timecode at the top.
- **Small windows** (both dimensions near the minimum): no file facts and no key caps in the header,
  icon-only update and bar buttons, no total time, trim bar 34 pt instead of 52 pt, the ruler keeps its
  spacing so its labels clear the transport row.

### Keyboard

New keys marked *. Single keys never fire while a text field has focus (existing rule).

| Group | Keys |
|---|---|
| Playback | Space play/pause · ← / → frame · ⇧← / ⇧→ ±5 s* · `,` `.` speed |
| Trim | I / O set start/end at the playhead · ⇧I / ⇧O go to start/end* · X clear the selection (whole file)* |
| Crop | C open/close the panel · 1–6 presets while the panel is open* · ⇧C remove crop* |
| Strip | S open/close the panel (turning Strip on)* · A add an ignored area* · Z edit areas · ⌫ delete the selected area · R scan/rescan* · ⇧S turn Strip off* |
| Edit | ⌘Z undo* · ⇧⌘Z (and Ctrl+Y on Windows/Linux) redo* |
| View | + − 0 video zoom · [ ] timeline zoom · \ timeline to the selection* · ⇧\ whole timeline* |
| File | ⌘O open · ⌘S export settings, again saves · ⌘I info · ⌘W close video · ⌘⇧E save the frame at the playhead* · ? shortcut sheet* · ⌘⇧R show the last export in the file manager* · ⌘1–⌘5 open a recent file (start screen)* |
| Export popover | P precise trim* · M remove audio* · Esc cancel · ⌘S save · Tab/⇧Tab move focus, ←/→ change a focused segmented control* |
| Dialogs | ⏎ the safe primary action · Esc cancel · ⌘⌫ the action that discards work* |

Padding, the update button and the dialogs' other buttons have no keys of their own (Tab reaches them).

- **Key caps** are one shared painter used in header buttons, tooltips, the popover, dialogs, the sheet,
  the context menu, the start legend and the status bar. Platform glyphs: ⌘ ⇧ ⌥ on macOS, "Ctrl",
  "Shift", "Alt" elsewhere (egui's `format_shortcut` style).
- **Tooltips** show a label plus key caps for every control that has a key; no control is icon-only
  without a tooltip.
- **Focus**: Tab order through the popover and dialogs, with a visible green focus ring.
- **Shortcut sheet (?)**: overlay in the main window, groups in three columns, a filter field that
  narrows rows as you type, Esc or ? closes. It lists every key in the table above.

### Overlays and flows

- **Export popover**: label/control rows (Format, Quality, Resolution), a one-line note under Quality
  saying what it means and which encoder will run, toggles with their keys and hints, a summary line
  (output length, ≈ size, Lossless copy or Re-encode · codec), Cancel (Esc) and Save as… (⌘S).
- **Close prompt** (closing or quitting with unexported edits): lists only the edits that differ, as
  chips: trim range, crop preset or "Free", Strip (with area count), speed. Cancel (Esc), Close/Quit anyway
  (⌘⌫), Export (⏎, opens the popover). The open-another-file prompt: Open in new window (⏎, its primary
  button today), Replace current (⌘⌫, it discards the open video's edits), Cancel (Esc).
- **Trim context menu** (right-click on the trim bar): Set start here, Set end here (at the clicked time,
  not the playhead), Timeline to selection, Clear selection, Save this frame… (the clicked time; ⌘⇧E
  saves the frame at the playhead). Each row shows its key where one exists.
- **Toasts** replace the bottom-center status pill: bottom-right above the status bar, icon, title,
  detail, optional action, ×. Saved: size and "N% smaller", "Show in Finder" (⌘⇧R; "Show in Explorer" on
  Windows, "Show in folder" on Linux). Errors use the red icon and the existing messages.
- **Start screen**: drop target without corner brackets (dashed border stays), Open video… (⌘O), Recent
  list, labeled key hints, the rain fading out behind the content, the update button at the title bar's
  end.

### New features

- **Undo/redo**: covers trim (in/out), crop (rect, preset, remove), Strip on/off, ignored areas
  (add, move, resize, delete, clear), padding and speed. Not playback, zoom, open panels or export
  settings. One step per gesture: a drag or a typed value is one step when it ends. 100 steps; cleared
  when another file opens.
- **Recent files**: the last 10 opened files, stored as one path per line in the OS config folder
  (`~/Library/Application Support/Chyk/recent`, `%APPDATA%\Chyk\recent`, `~/.config/chyk/recent`).
  The start screen shows up to 5 that still exist: thumbnail, name, codec · size · length, when; ↑↓ ⏎ and
  ⌘1–⌘5. Thumbnail and facts are cached when the file is opened (the first filmstrip tile, saved in
  Chyk's cache folder); without a cache entry the row shows a plain film icon.
- **Save this frame…**: one PNG of the chosen frame as previewed (crop applied, no scaling), via ffmpeg;
  save dialog default `<name>-<h-m-s.ms>.png` next to the source; the source can't be overwritten.
- **Show in the file manager**: macOS `open -R`, Windows `explorer /select,`, Linux opens the folder.
- **ffmpeg log window**: a separate OS window (egui viewport) titled "Chyk — ffmpeg log", opened and
  closed by the status bar button. Closing the main window closes it; closing it leaves the main window
  open and turns the button off. Read-only, text selectable. One session-wide list, newest last: time,
  kind (thumbnails, scan, estimate, export, frame), the full command, the result (exit code and duration,
  or live progress for a running export) and what ffmpeg printed (stderr; for exports the latest progress
  line, not every one). Filter by kind; Copy copies the visible entries as text. Every ffmpeg Chyk
  starts is logged, including the thumbnails whose stderr is discarded today.
  An entry is one job, not one process: a filmstrip refresh (its 12 tiles), a scan, a size estimate (its
  samples and encoder fallbacks), an export (fallbacks and the oversize redo), a frame save. The entry
  keeps every command it ran with its result; a batch of identical-but-for-time commands shows the first
  in full and a count. Encoder and source probes (`-encoders`, `-version`, `ffmpeg -i`) are kind
  "probe". Bounded: the last 200 entries, each entry's output capped (about 200 lines).

## Left to the builder

- The order of work and how to split it into releases.
- Exact sizes, spacing and animation, within the mockups' look.
- The log window's shortcut (it must appear in the ? sheet), its default size and minimum.
- Internal structure: where the key map, history, log sink and toast queue live.
- How the open-another-file prompt looks beyond its keys.
- Whether the ruler and playhead label scale differently on very long videos.

## Calls made without the user

Settled after the builder check, for the user to confirm while reading:

1. Open-another-file prompt: ⏎ opens in a new window (the safe choice, today's primary button);
   Replace, which discards edits, moves to ⌘⌫.
2. The no-movement anti-goal covers the layout only; crop preview framing stays as it is.
3. A log entry is one job (filmstrip refresh, scan, estimate, export, frame save), not one process;
   probes are their own kind.
4. The key table is the complete list. Added ⌘⇧E (save the frame at the playhead) and ⇧\ (whole
   timeline); padding, the update button and secondary dialog buttons get none.
5. There is no update prompt; updating stays one click, as today.
