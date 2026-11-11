# Chyk UI review: toward a keyboard-first, Linear-grade Phosphor

Status: proposal. Mockups: `design/concepts/` (`python3 design/concepts/build.py`, viewer
`design/concepts/index.html`). Nothing here is built yet.

Goal (from the user): keep the colors and the overall Phosphor look, but make it feel premium the way
Linear does, with keyboard support for everything.

## What the app does well today

- One palette, one button painter: states look the same everywhere.
- Nothing sits on the video; the header and deck stay out of the way.
- Common actions already have keys: Space, ←/→, I/O, C, Z, +/−/0, [/], ,/., ⌘O/S/I/W, Esc.

## Findings (from the running 1.2.6 build)

Keyboard
1. Strip has no key (Crop has C). Rescan, Ignore area, padding, Precise trim, Remove audio,
   formats and qualities have none either.
2. No shortcut reference inside the editor. The only list is the start
   screen's key-cap legend, and it pairs keys with icons alone (← → + a film icon), so it doesn't say
   what the keys do.
3. Tooltips mention keys inconsistently ("Play (Space)", "Zoom in (+)", but none for ±5 s, the
   selection or crop presets).
4. The close prompt only takes Esc. Enter doesn't export and nothing closes anyway.
5. The export popover, the resolution list and the crop presets can't be driven from the keyboard
   (no focus ring, no arrow keys, no number keys).
6. No undo. A slipped trim handle or a cleared crop can't be taken back.

Layout and hierarchy
7. The header slot changes meaning: file info normally, the crop or strip bar while a panel is open.
   The bar has to shrink to a compact mode when it doesn't fit, and the file name disappears.
8. Monospace everywhere (labels, buttons, values) makes everything the same weight; numbers, which
   need monospace, don't stand out from labels.
9. Transport icons have no labels (±5 s clocks, four timeline zoom icons). The selection fields use
   icons for IN/OUT/LEN.
10. The trim bar has no time ruler, so a handle's position can only be read from the fields.
11. The close prompt lists every kind of edit ("trim, crop, strip and speed") whatever changed.
12. After a save there's a status pill but no way to reach the file.

## Proposal (as rendered)

Decided with the user (2026-11-11): no command palette (too much for an app this size; tooltips, the
? sheet and the context menu cover discovery); the panel bars stay in the header so opening Crop or
Strip never changes the video's height; no file menu caret; no corner brackets on the drop target.
Liked: status bar, trim context menu, saved toast, close prompt that lists the edits.

Chrome
- **Header** (fixed 48 pt): lights, mark, Crop | Strip (key chips C, S; a dot marks an applied
  effect), then a slot: the file (name, codec · size · length, info, close) or, while a panel is
  open, its bar. Right: the update button, then Export with ⌘S.
- **Panel bars** (in the header): Crop: presets, Remove crop. Strip: Ignore area, Edit areas (with
  the count), padding stepper, kept share, Rescan, Turn off. Keys live in the tooltips and the
  ? sheet so the bars fit; Shift + the mode key removes that effect.
- **Update button**: Update available (label over the version tag) → downloading (progress, cancel)
  → Update now. Icon only on small windows and while a panel bar takes the header. On the start
  screen it sits at the title bar's end.
- **Status bar** (26 pt): the ffmpeg log button at the far left, state and what an export will do
  ("Re-encode · 4:3 crop"), export progress while saving, "? Shortcuts", version.
- **ffmpeg log**: a separate OS window (its own title bar, movable to another screen), read-only and
  terminal-like. Closing the main window closes it; closing it leaves the main window open. It lists
  every ffmpeg command Chyk ran
  (thumbnails, scan, size estimate, export) with time, kind, result (exit code, duration, speed) and
  what it printed; a running export streams its progress. Filter by kind, Copy. The button shows a
  dot while ffmpeg runs.
- **Small windows** (980 × 460 minimum): no file facts, no key chips in the header, icon-only
  update and bar buttons, no total time, a shorter ruler and trim bar.

Editor
- Crop overlay: thirds grid, corner and edge handles, a size badge.
- Strip: ignored areas labeled on the video; cut stretches hatched red on the trim bar.
- Deck: text labels IN / OUT / LEN; speed and zoom as pills; a time ruler above the trim bar; the
  playhead carries its timecode.
- Tooltips: label plus key caps, never a bare icon.

Overlays
- **Shortcut sheet (?)**: all keys in groups, filterable.
- **Export popover**: label/control rows, quality explained in one line, resolution with its size,
  toggles with P / M, a summary (length, size, re-encode or copy), Cancel Esc and Save ⌘S.
- **Close prompt**: lists only the edits that changed, as chips; Esc cancels, ⌘⌫ closes anyway,
  ⏎ exports.
- **Context menu** on the trim bar (right-click): set start/end here, timeline to selection, clear
  selection, save this frame.
- **Saved toast**: size and how much smaller, "Show in Finder ⌘⇧R".
- **Start**: drop target with an Open button (⌘O), a Recent list (⌘1–3, ↑↓ ⏎), labeled key hints;
  the rain fades out behind the content.

## Proposed key map (new keys marked *)

| Group | Keys |
|---|---|
| Playback | Space play · ←/→ frame · ⇧←/⇧→ ±5 s* · , . speed |
| Trim | I / O set · ⇧I / ⇧O go to start/end* · X clear selection* |
| Crop & Strip | C crop · 1–6 presets (crop open)* · ⇧C remove crop* · S strip* · A ignore area* · Z edit areas · R rescan* · ⇧S strip off* |
| Edit | ⌘Z / ⇧⌘Z undo / redo* |
| View | + − 0 video zoom · [ ] timeline zoom · \ timeline to selection* |
| File | ⌘O open · ⌘S export, again saves · ⌘I info · ⌘W close · ? shortcuts* · ⌘⇧R show in Finder* · ⌘1–3 recent* |
| Dialogs | ⏎ default · Esc cancel · ⌘⌫ destructive* · Tab / ⇧Tab and arrows move focus* |

## New features this implies

Shortcut sheet, undo/redo, recent files, context menu, save-frame, show in Finder, a status bar, the
ffmpeg log. Recent files and save-frame are new behavior, not just UI; drop them if unwanted.

## Open questions

1. Which of the new features to keep (recent files, save frame, undo).

Decided: Space Mono stays for everything (a UI sans was rendered and declined). The ffmpeg log is a
separate OS window.

New strings in the mockups that aren't in `assets/i18n/*.lang` yet show in English in both languages.
