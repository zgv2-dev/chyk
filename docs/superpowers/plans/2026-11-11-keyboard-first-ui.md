# Keyboard-first Phosphor UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved keyboard-first Phosphor redesign: a complete key map with key caps and a ? sheet, undo/redo, status bar, toasts, ruler, restyled popover and prompts, trim context menu, recent files, save frame, and an ffmpeg log window.

**Architecture:** One declarative key table (`app/keys.rs`) drives dispatch, tooltips and the ? sheet; actions run through `App::run(Cmd)`. Undo snapshots the edits when the UI is at rest (`app/history.rs`). A process-wide ffmpeg log (`ffmpeg/log.rs`) records every spawn; a second egui viewport shows it. Visual changes reuse the existing painter (`button`/`Look`) and follow `design/concepts/`.

**Tech Stack:** Rust nightly (rust-toolchain.toml), egui/eframe 0.36 (glow), libmpv, ffmpeg CLI. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-11-11-keyboard-first-ui-design.md` (mockups: `design/concepts/`, review: `docs/superpowers/specs/2026-11-11-keyboard-first-ui-review.md`).

## Global Constraints

- No source file over 200 lines (code, scripts, tests); split by responsibility before crossing.
- Space Mono for UI text; Fira Code for Cyrillic as today. Palette and `button`/`Look` painter unchanged.
- ⌘ = `Modifiers::COMMAND` (Ctrl on Windows/Linux). Single-letter keys never fire while `ctx.egui_wants_keyboard_input()`.
- Every new string in both `assets/i18n/en.lang` and `uk.lang`, Ukrainian translated (tests `files_agree`, `used_keys_exist`).
- No new crates. Temp files from manual runs go in `<repo>/.scratchpad/`, never `/tmp`.
- Gates per task: `cargo check --workspace --all-targets`, focused `cargo nextest run <filter>`, `cargo clippy --workspace --all-targets -- -D warnings` (also `--target x86_64-unknown-linux-gnu` and `--target x86_64-pc-windows-msvc` when touching cfg'd code), `cargo fmt --all`; full `cargo nextest run --workspace` before each commit.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Don't push until Task 20.
- Existing shortcuts keep their meaning: Space, ←/→, I/O, C, Z, +/=/−/0, [/], `,`/`.`, ⌘O/S/I/W, Esc, ⌫ (zone edit).

## Review Focus

1. Typing in a time field, the padding stepper or the ? filter: no single-letter or arrow shortcut fires → test in Task 3.
2. ⇧I / ⇧O / ⇧← must not also run I / O / ← (egui matches Shift loosely) → test in Task 3.
3. Undo right after opening another file must not bring back the previous file's edits; undo during an export must not touch the running export → tests in Task 6.
4. Recent list with deleted or moved files, unicode and space-containing paths, an unwritable config folder → tests in Task 15.
5. Closing the log window while an export runs, reopening it, and quitting with it open: no hang, entries kept, playback unaffected → manual check in Task 18.

---

## Milestone A: keyboard

### Task 1: Commit the design work

**Files:** already on disk: `assets/chyk.{png,ico}`, `design/icon/*`, `design/concepts/*`, `docs/superpowers/specs/2026-11-11-*`, `scripts/i18n-editor/index.html`, `SPEC.md`, `.gitignore`.

- [ ] **Step 1:** `python3 -B design/icon/build.py && python3 -B design/concepts/build.py` both finish; `git status` shows only the files above.
- [ ] **Step 2:** Commit in two parts: `feat(icon): scissors cutting the trim box` (assets, design/icon, .gitignore, SPEC icon line) and `docs: keyboard-first UI review, design and mockups` (the rest).

### Task 1a: The new icon everywhere

The user asked for the scissors icon (`design/icon/snip.py`, already in `assets/chyk.{png,ico}`) to replace
the old trim-box mark everywhere it appears.

**Files:**
- Modify: `design/icon/snip.py` (add `mark()`: the two film halves and the scissors without the tile, dots
  or shadow, on a transparent background), `design/icon/build.py` (render `mark()` to `assets/mark.png`,
  120 × 96 px, viewBox cropped to the content so it fills a 5:4 box), `src/app/marks.rs` (`logo()` draws
  `assets/mark.png` via `eframe::icon_data::from_png_bytes` loaded once into a `TextureHandle`, 30 × 24 pt;
  same signature `pub(super) fn logo(ui: &mut Ui) -> Response`), `design/concepts/mock_base.py` (`LOGO` =
  `<img class="logo" src="../../assets/mark.png">` or the same SVG as `mark()`), `scripts/i18n-editor/index.html`
  (its inline logo), `site/assets/icon.png` (the new `assets/chyk.png` scaled to 256 px with `sips -z`).
- The window, Dock, taskbar, `.icns` and `.ico` already use `assets/chyk.*`; nothing to change there.

- [ ] **Step 1:** `python3 -B design/icon/build.py` writes `assets/mark.png` (120 × 96, transparent corners: check
  with `sips -g pixelWidth -g pixelHeight` and one corner pixel).
- [ ] **Step 2:** Implement `logo()`; no test (pure drawing). Run the app: the header and start screen show the
  scissors mark, crisp on a 2× display, equal space on both sides (as Task 9 measures).
- [ ] **Step 3:** Rebuild mockups (`python3 -B design/concepts/build.py`), check `phosphor-editor.png` and
  `phosphor-start.png` show the new mark; gates; commit `feat(icon): the scissors mark in the app, mockups and site`.

### Task 2: Key table and key caps

**Files:**
- Create: `src/app/keys.rs` (table + caps), `src/app/keys/tests.rs`
- Modify: `src/app/mod.rs` (add `mod keys; use keys::*;`), `src/app/controls.rs:120` (`keycap` stays the single-cap painter)

**Interfaces:**
- Produces:
  - `pub(super) enum Cmd { PlayPause, FrameBack, FrameNext, Back5, Forward5, Slower, Faster, SetIn, SetOut, GoIn, GoOut, ClearSelection, Crop, CropPreset(usize), RemoveCrop, Strip, AddZone, EditZones, DeleteZone, Rescan, StripOff, Undo, Redo, ZoomIn, ZoomOut, ZoomFit, TimelineIn, TimelineOut, TimelineToSelection, TimelineWhole, Open, Export, Info, CloseVideo, SaveFrame, Shortcuts, RevealExport, OpenRecent(usize), ToggleLog }` (`Clone, Copy, PartialEq, Eq, Debug`)
  - `pub(super) enum Group { Playback, Trim, Crop, Strip, Edit, View, File }`
  - `pub(super) struct Binding { pub(super) cmd: Cmd, pub(super) mods: Modifiers, pub(super) key: Key, pub(super) group: Group }`
  - `#[derive(Clone, Copy)] Binding`; `pub(super) fn bindings() -> Vec<Binding>`: exactly the spec's key table, plus ⌘L → `ToggleLog` (File), Ctrl+Y → `Redo` only when `!cfg!(target_os = "macos")`, `=` and ⇧+ → `ZoomIn`, ⇧`,` / ⇧`.` → speed (as today). `CropPreset(0..=5)` on `Num1..=Num6`, `OpenRecent(0..=4)` on ⌘`Num1..=Num5`.
  - `pub(super) fn label(cmd: Cmd) -> &'static str`: i18n key, `cmd.<snake_name>` (presets/recent share `cmd.crop_preset` / `cmd.open_recent`).
  - `pub(super) fn caps(mods: Modifiers, key: Key, mac: bool) -> Vec<String>`
  - `pub(super) fn keycaps(ui: &mut Ui, caps: &[String]) -> Response`: draws caps in a row with `controls::keycap`.
  - `pub(super) fn first_binding(cmd: Cmd) -> Option<Binding>`

- [ ] **Step 1: Write the failing tests** in `src/app/keys/tests.rs`:

```rust
#[test] fn no_two_bindings_share_a_chord() { /* (mods, key) pairs in bindings() are unique */ }
#[test] fn every_command_has_a_label_in_english() { /* label(b.cmd) is a key of assets/i18n/en.lang for every binding */ }
#[test] fn caps_follow_the_platform() {
    assert_eq!(caps(Modifiers::COMMAND | Modifiers::SHIFT, Key::R, true), ["⇧", "⌘", "R"]);
    assert_eq!(caps(Modifiers::COMMAND | Modifiers::SHIFT, Key::R, false), ["Ctrl", "Shift", "R"]);
    assert_eq!(caps(Modifiers::NONE, Key::ArrowLeft, true), ["←"]);
    assert_eq!(caps(Modifiers::NONE, Key::Space, false), ["Space"]);
}
#[test] fn existing_keys_keep_their_commands() { /* Space→PlayPause, I→SetIn, O→SetOut, C→Crop, Z→EditZones, ⌘S→Export, ⌘W→CloseVideo, [→TimelineOut, ]→TimelineIn */ }
```

- [ ] **Step 2:** `cargo nextest run keys::` fails (module missing).
- [ ] **Step 3:** Implement `keys.rs`. Mac cap order ⌃⌥⇧⌘ then key; elsewhere Ctrl, Alt, Shift, key. Key names via `Key::symbol_or_name()`; "Space" through `t!("key.space")` in `keycaps` only (tests stay language-free). Add the `cmd.*` keys to both `.lang` files now (English from `design/concepts/mock_overlays.py` `COMMANDS`; Ukrainian translated).
- [ ] **Step 4:** `cargo nextest run keys::` passes; full gates.
- [ ] **Step 5:** Commit `feat(keys): one table for every shortcut, with key caps`.

### Task 3: Dispatch through the table, new keys

**Files:**
- Create: `src/app/commands.rs` (`App::run`, `App::enabled`), `src/app/commands/tests.rs`
- Modify: `src/app/shortcuts.rs` (drop the hand-written key checks; keep drop handling, the modal early return and the Escape cascade), `src/app/mod.rs`

**Interfaces:**
- Consumes: `Cmd`, `bindings()`, from Task 2.
- Produces:
  - `pub(super) fn pressed(ctx: &egui::Context, typing: bool) -> Vec<Cmd>`: consumes matching key events; bindings with more modifiers are tried first; when `typing`, only bindings with ⌘ are considered.
  - `impl App { pub(super) fn enabled(&self, cmd: Cmd) -> bool; pub(super) fn run(&mut self, cmd: Cmd, ctx: &egui::Context) }`
  - Behavior of the new commands: `Back5/Forward5` → `skip(∓5.0)`; `GoIn/GoOut` → `player.seek(trim.0/1)`; `ClearSelection` → trim = (0, dur); `CropPreset(i)` only while `panel == Some(Panel::Crop)` → `set_crop_preset(i)`; `RemoveCrop` → `remove_crop`; `Strip` → `toggle_strip`; `AddZone` → open Strip panel if closed, then `add_zone`; `Rescan` → `start_scan`; `StripOff` → `strip_off`; `TimelineToSelection` → `fit_timeline_selection`; `TimelineWhole` → `fit_timeline`. `Undo/Redo`, `SaveFrame`, `Shortcuts`, `RevealExport`, `OpenRecent`, `ToggleLog` are no-ops returning early until their tasks land (each later task fills its arm).

- [ ] **Step 1: Write the failing tests** (drive a real `egui::Context` with synthetic `RawInput` key events):

```rust
#[test] fn shift_i_goes_to_start_and_does_not_set_in() { assert_eq!(press(Modifiers::SHIFT, Key::I, false), [Cmd::GoIn]); }
#[test] fn shift_arrow_skips_instead_of_stepping() { assert_eq!(press(Modifiers::SHIFT, Key::ArrowLeft, false), [Cmd::Back5]); }
#[test] fn typing_blocks_single_keys_but_not_command_keys() {
    assert!(press(Modifiers::NONE, Key::I, true).is_empty());
    assert!(press(Modifiers::NONE, Key::ArrowLeft, true).is_empty());
    assert_eq!(press(Modifiers::COMMAND, Key::S, true), [Cmd::Export]);
}
#[test] fn question_mark_opens_the_sheet() { /* Shift+Slash / Key::Questionmark → [Cmd::Shortcuts] */ }
```

- [ ] **Step 2:** `cargo nextest run commands::` fails.
- [ ] **Step 3:** Implement `pressed`, `enabled` (video loaded for editor commands; `OpenRecent` only without a video; `DeleteZone` only in zone edit), `run`. `shortcuts()` becomes: drops, modal, then `for cmd in pressed(ctx, ctx.egui_wants_keyboard_input()) { if self.enabled(cmd) { self.run(cmd, ctx) } }`, then the Escape cascade. While a popover is open and a widget has focus, Space/arrows aren't dispatched (they belong to the focused control).
- [ ] **Step 4:** Tests pass; run the app on `.scratchpad/samples/h264.mp4` (`bash scripts/make-samples.sh .scratchpad/samples`) and try every key in the spec table.
- [ ] **Step 5:** Commit `feat(keys): every action on a key (Strip S, presets 1–6, jumps, timeline)`.

### Task 4: Key caps in tooltips and on buttons

**Files:** Modify `src/app/buttons.rs` (`button`, `primary` get an optional `Option<Cmd>`; or add `fn with_keys(resp: Response, label: &str, cmd: Cmd) -> Response` used at call sites), `src/app/header.rs`, `src/app/bars.rs`, `src/app/strip_bar.rs`, `src/app/deck.rs`, `src/app/deck_controls.rs`, `src/app/update_button.rs`; `assets/i18n/*.lang` (tooltips lose their "(key)" suffixes: `tip.play`, `tip.pause`, `tip.zoom_*`, `tip.timeline_*`, `strip.edit_zones`, `tip.close_video`, `tip.export`, `tip.speed`).

**Interfaces:** Produces `pub(super) fn tip_keys(resp: Response, label: &str, cmd: Cmd) -> Response`: hover tooltip with the label and `keycaps(caps(first_binding(cmd)))`.

- [ ] **Step 1:** Test in `keys/tests.rs`: `tooltip_strings_carry_no_key_suffix`: no value of the listed keys in `en.lang` ends with `)` or contains `(Space)`, `(+)`, `(])`.
- [ ] **Step 2:** Fails. **Step 3:** Implement; Crop/Strip mode buttons draw caps C / S right of the label, Export draws ⌘S (hidden when the header is compact). Every icon-only control with a binding uses `tip_keys`.
- [ ] **Step 4:** Tests and gates pass; hover each control in the app.
- [ ] **Step 5:** Commit `feat(ui): tooltips and buttons show their keys`.

### Task 5: ? sheet, dialog keys, focus ring

**Files:**
- Create: `src/app/sheet.rs` (overlay), `src/app/focus.rs` (`focus_ring`)
- Modify: `src/app/modal.rs` (⏎ / ⌘⌫ / Esc), `src/app/pops.rs` (P / M while the export popover is open), `src/app/buttons.rs`, `src/app/controls.rs` (`segmented` handles ←/→ when focused; `switch`, `button` draw `focus_ring` when `resp.has_focus()`), `src/app/mod.rs` (`sheet: Option<String>` = open with its filter text), `src/app/commands.rs` (`Shortcuts` arm toggles it)

**Interfaces:** Produces `pub(super) enum Choice { Export, Discard, NewWindow, Replace }`, `pub(super) fn sheet_rows(filter: &str) -> Vec<(Group, Cmd, Vec<Binding>)>` (pure, case-insensitive match on the English or current-language label) and `pub(super) fn focus_ring(p: &egui::Painter, r: Rect, radius: f32)`.

- [ ] **Step 1: Tests** (`sheet` module):

```rust
#[test] fn sheet_lists_every_binding_once() { assert_eq!(sheet_rows("").iter().map(|r| r.1).collect::<HashSet<_>>().len(), /* distinct cmds in bindings() */); }
#[test] fn filter_narrows_rows() { assert!(sheet_rows("zoom").iter().all(|r| matches!(r.1, Cmd::ZoomIn | Cmd::ZoomOut | Cmd::ZoomFit | Cmd::TimelineIn | Cmd::TimelineOut))); }
```
  and in `modal` tests (pure helper `fn modal_key(confirm: &Confirm, enter: bool, cmd_backspace: bool) -> Option<Choice>`): Quit/CloseVideo: ⏎ → `Choice::Export`, ⌘⌫ → `Choice::Discard`; Open: ⏎ → `Choice::NewWindow`, ⌘⌫ → `Choice::Replace`.
- [ ] **Step 2:** Fail. **Step 3:** Implement. Sheet: overlay over a dim like the modal, three columns of groups, filter field focused on open, Esc or ? closes. Dialog buttons show their caps (Cancel esc, discard ⌘⌫, primary ⏎).
- [ ] **Step 4:** Pass; check Tab through the export popover shows the ring and ←/→ changes Format/Quality.
- [ ] **Step 5:** Commit `feat(keys): shortcut sheet, dialog keys, focus ring`.

## Milestone B: undo

### Task 6: Undo/redo

**Files:** Create `src/app/history.rs`, `src/app/history/tests.rs`; Modify `src/app/mod.rs` (field `history: History<Snapshot>`), `src/app/frame.rs` (settle at the end of `ui`), `src/app/file.rs` (`loaded` and `close` clear it), `src/app/session.rs` (`apply_session` uses `apply_snapshot` for the edit part), `src/app/commands.rs` (Undo/Redo arms).

**Interfaces:**
- `#[derive(Clone, PartialEq, Debug)] pub(super) struct Snapshot { trim: (f64, f64), crop: Option<Rect>, preset: usize, strip: bool, zones: Vec<Rect>, pad: f64, speed: f64 }`
- `pub(super) struct History<T> { past: Vec<T>, future: Vec<T>, rest: Option<T> }` with `const DEPTH: usize = 100;`
  - `fn settle(&mut self, now: &T)`: if `rest` differs from `now`, push `rest` to `past` (drop the oldest past `DEPTH`), clear `future`; then `rest = now`.
  - `fn undo(&mut self, now: &T) -> Option<T>`, `fn redo(&mut self, now: &T) -> Option<T>`, `fn reset(&mut self, now: T)`.
- `impl App { fn snapshot(&self) -> Snapshot; fn apply_snapshot(&mut self, s: Snapshot, ctx: &egui::Context) }`: sets fields, `set_speed`, `crop_applied`, `zones_changed` when zones differ, `ensure_scan` when Strip is on.
- At rest = `self.drag.is_none() && self.strip.drag.is_none() && !ctx.egui_wants_keyboard_input() && !ctx.input(|i| i.pointer.any_down())`.

- [ ] **Step 1: Tests:**

```rust
#[test] fn one_change_one_step() { let mut h = History::default(); h.reset(1); h.settle(&2); assert_eq!(h.undo(&2), Some(1)); assert_eq!(h.redo(&1), Some(2)); }
#[test] fn a_new_edit_clears_redo() { /* reset 1, settle 2, undo→1, settle 3: redo(&3) == None */ }
#[test] fn keeps_the_last_100() { /* 150 settles: undo succeeds 100 times then None */ }
#[test] fn reset_forgets_the_previous_file() { /* reset 1, settle 2, reset 9: undo(&9) == None */ }
```
- [ ] **Step 2:** Fail. **Step 3:** Implement. Undo/Redo stay enabled during an export: they change only the editor, the running export already holds its `Job` (say so in a comment).
- [ ] **Step 4:** Pass; in the app: drag a handle, ⌘Z restores it in one step; type a time, ⌘Z; add/move/delete zones; toggle Strip; speed.
- [ ] **Step 5:** Commit `feat(edit): undo and redo for trim, crop, strip and speed`.

## Milestone C: chrome and layout

### Task 7: Status bar

**Files:** Create `src/app/status_bar.rs`; Modify `src/app/frame.rs` (bottom panel below the deck, also on the start screen), `src/app/mod.rs` (`log_open: bool` used by Task 18; `export_started: Option<Instant>`), `src/app/export.rs` (set `export_started`).

**Interfaces:** `pub(super) fn reencode_reasons(a: &ReasonInput) -> Vec<&'static str>` (i18n keys) where `ReasonInput { crop: bool, resize: bool, strip: bool, retime: bool, precise: bool, quality_high: bool }`; `fn time_left(elapsed: Duration, frac: f32) -> Option<Duration>` (None below 2%).

- [ ] **Step 1: Tests:** `copy_has_no_reasons` (all false, quality_high → empty), `crop_and_speed_are_named` (→ `["reason.crop", "reason.speed"]` in that order: crop, resize, strip, speed, precise, quality), `time_left_extrapolates` (10 s at 0.5 → 10 s; 0.01 → None).
- [ ] **Step 2:** Fail. **Step 3:** Implement the bar (26 pt): log button (icon `TERMINAL_WINDOW`; it does nothing until Task 18, which also adds the busy dot), state ("Ready · Lossless copy" / "Ready · Re-encode · crop, speed" / `strip.scanning` / "Exporting {name} · {pct}% · {left} left"), right: caps `?` + "Shortcuts", version. New keys `status.ready`, `status.exporting`, `status.left`, `reason.*`.
- [ ] **Step 4:** Pass; check at 980×460.
- [ ] **Step 5:** Commit `feat(ui): status bar`.

### Task 8: Toasts and Show in folder

**Files:** Create `src/reveal.rs` (OS file manager), `src/app/toast.rs` (replaces `status.rs`); Modify `src/main.rs` (`mod reveal;`), `src/app/mod.rs` (fields `export_dst: Option<PathBuf>`, `last_export: Option<PathBuf>`), `src/app/export.rs` (set `export_dst`), `src/app/poll.rs` (on `Done(Ok)` move it to `last_export`), `src/app/commands.rs` (`RevealExport`).

**Interfaces:** `pub fn reveal_command(path: &Path, os: &str) -> (String, Vec<OsString>)` and `pub fn reveal(path: &Path)`; macOS `open -R <path>`, Windows `explorer /select,<path>`, Linux `xdg-open <parent>`. Toast: bottom-right above the status bar; the existing `status` / `status_since` fields and fade rules stay; a saved toast gets the file name as detail and a "Show in Finder/Explorer/folder" button with ⌘⇧R caps; ×; errors use `RED`.

- [ ] **Step 1: Tests** (`reveal`): `mac_selects_the_file`, `windows_selects_the_file` (`explorer`, `["/select,C:\\v\\a.mp4"]`), `linux_opens_the_folder`.
- [ ] **Step 2:** Fail. **Step 3:** Implement; keys `toast.reveal_mac`, `toast.reveal_win`, `toast.reveal_linux`.
- [ ] **Step 4:** Pass; export a clip, click Show in Finder and press ⌘⇧R.
- [ ] **Step 5:** Commit `feat(ui): toasts, show the export in the file manager`.

### Task 9: Header

**Files:** Modify `src/app/header.rs`, `src/app/update_button.rs`, `src/app/bars.rs`, `src/app/strip_bar.rs`.

- [ ] **Step 1:** Implement: equal space either side of the mark (measure: lights end → mark = mark → mode group); update button icon-only while `self.panel.is_some()` or the header is compact; caps on Crop/Strip/Export from Task 4; padding shown with the arrows icon and a stepper; the export progress pill adds the time left (`time_left` from Task 7); header height unchanged (52 pt).
- [ ] **Step 2:** Visual check against `design/concepts/renders/phosphor-{editor,crop,strip,small-strip}.png` at 1280×800 and 980×460, en and uk (`CHYK_LANG=uk`): nothing overflows, the video doesn't move when panels open.
- [ ] **Step 3:** Gates; commit `feat(ui): header spacing, update button collapses beside panel bars`.

### Task 10: Deck: labels, ruler, playhead time, small trim bar

**Files:** Create `src/app/ruler.rs`, `src/app/ruler/tests.rs`; Modify `src/app/deck.rs`, `src/app/deck_controls.rs` (IN/OUT/LEN text labels via `range.in`/`range.out`/`range.len`), `src/app/trim.rs` (height `TRIM_H` or `TRIM_H_SMALL = 34.0` when the window is under 520 pt tall), `src/app/trim_paint.rs` (playhead label), `src/app/style.rs`.

**Interfaces:** `pub(super) fn ruler_steps(span: f64, width: f32) -> (f64, f64)` returns (minor, major) seconds: the first pair of `const STEPS: &[(f64, f64)] = &[(0.02, 0.1), (0.05, 0.25), (0.1, 0.5), (0.2, 1.0), (0.5, 2.0), (1.0, 5.0), (2.0, 10.0), (5.0, 15.0), (5.0, 30.0), (10.0, 60.0), (30.0, 120.0), (60.0, 300.0), (120.0, 600.0), (300.0, 900.0), (300.0, 1800.0), (600.0, 3600.0)]` whose major is spaced ≥ 60 pt (the last pair if none); minor becomes major when spaced under 8 pt.

- [ ] **Step 1: Tests:** `forty_two_seconds_on_a_wide_bar` (42, 1248 → (1.0, 5.0)), `five_minutes` (315, 1248 → (5.0, 30.0)), `one_second` (1.0, 1248 → (0.02, 0.1)), `ten_hours_narrow` (36000, 300 → (600.0, 3600.0)).
- [ ] **Step 2:** Fail. **Step 3:** Implement; the ruler sits above the trim bar inside the deck, labels in mono 9.5, follows the timeline view.
- [ ] **Step 4:** Pass; compare with `phosphor-editor.png` and `phosphor-small-editor.png`.
- [ ] **Step 5:** Commit `feat(ui): time ruler, labeled selection fields, slimmer trim bar in small windows`.

### Task 11: Export popover

**Files:** Modify `src/app/pops.rs` (split: `src/app/pop_export.rs` if it passes 200 lines), `src/ffmpeg/encode.rs` (expose `pub fn first_encoder(codec: &str) -> Option<&'static str>` from `encoder_order`), lang files (`quality.<q>_hint`).

- [ ] **Step 1: Test** in `src/ffmpeg/tests/`: `first_encoder_prefers_hardware` with a fake encoder list string (reuse `candidates`).
- [ ] **Step 2:** Fail. **Step 3:** Implement rows (label left 78 pt, control right), the note under Quality ("{hint} · {codec}, {encoder}"), toggles with P/M caps, summary line, footer Cancel (esc) / Save as… (⌘S). Resolution dropdown and estimates unchanged.
- [ ] **Step 4:** Pass; compare with `phosphor-export.png`.
- [ ] **Step 5:** Commit `feat(ui): export popover rows, quality note, keyboard footer`.

### Task 12: Close prompt lists the changed edits

**Files:** Modify `src/app/modal.rs` (split `src/app/modal_chips.rs`), lang files.

**Interfaces:** `pub(super) enum Chip { Trim(f64, f64), Crop(usize), Strip(usize), Speed(f64) }`; `pub(super) fn changed(saved: &Edits, now: &Edits, preset: usize) -> Vec<Chip>` (order: trim, crop, strip, speed; Crop carries the current preset index, Strip the current area count). `saved`/`unsaved()` keep using `Edits`.

- [ ] **Step 1: Tests:** `only_speed_changed` → `[Speed(1.5)]`; `crop_and_trim` → `[Trim(..), Crop(5)]`; `nothing_changed` → empty.
- [ ] **Step 2:** Fail. **Step 3:** Implement; new key `unsaved.list`; chips use the existing icons (scissors, crop, magic wand, gauge).
- [ ] **Step 4:** Pass; compare with `phosphor-confirm.png`.
- [ ] **Step 5:** Commit `feat(ui): close prompt lists what changed`.

### Task 13: Trim context menu

**Files:** Create `src/app/trim_menu.rs`; Modify `src/app/trim.rs` (`resp.context_menu`), `src/app/commands.rs` (shared actions).

- [ ] **Step 1:** Implement with `egui::Response::context_menu`: the clicked time is `tv.t_at(pointer.x)` captured on `secondary_clicked`; rows Set start here (I), Set end here (O), Timeline to selection (\\), Clear selection (X), Save this frame… (⌘⇧E, at the clicked time). Rows show caps.
- [ ] **Step 2:** Manual check: right-click left of the selection, Set start here moves IN to that time, not the playhead.
- [ ] **Step 3:** Gates; commit `feat(ui): trim bar context menu`.

### Task 14: Start screen

**Files:** Modify `src/app/start.rs`, `src/app/legend.rs` (labeled hints), `src/app/backdrop.rs` (rain fades near the center).

- [ ] **Step 1:** Implement: no corner brackets, dashed border kept; Open video… button with ⌘O caps; legend `Space Play · I O Trim · C Crop · S Strip · ⌘S Export · ? Shortcuts`; rain alpha × smoothstep of distance from the content box; update button stays at the title bar's end. Recent list slot left empty until Task 15.
- [ ] **Step 2:** Compare with `phosphor-start.png`; gates; commit `feat(ui): start screen open button and labeled keys`.

## Milestone D: features

### Task 15: Recent files

**Files:** Create `src/recent.rs`, `src/recent/tests.rs`, `src/app/recent_list.rs`; Modify `src/update/paths.rs` (`pub fn cache_base() -> Option<PathBuf>`; `cache_dir()` = `cache_base()?.join("update")`), `src/app/file.rs` (`loaded` records), `src/app/view.rs` (first tile of a whole-file filmstrip → thumbnail), `src/app/commands.rs` (`OpenRecent`).

**Interfaces:**
- `pub struct Recent { pub path: PathBuf, pub opened: u64 /* unix secs */, pub codec: String, pub size: [u32; 2], pub dur: f64 }`
- `pub fn parse(text: &str) -> Vec<Recent>` / `pub fn format(list: &[Recent]) -> String`: one file per line, `opened\tdur\tWxH\tcodec\tpath` (path last, may contain anything but a newline).
- `pub fn push(list: &mut Vec<Recent>, item: Recent)`: dedupe by path, newest first, keep 10.
- `pub fn config_dir() -> Option<PathBuf>`: `~/Library/Application Support/Chyk`, `%APPDATA%\Chyk`, `$XDG_CONFIG_HOME/chyk` or `~/.config/chyk`. File `recent`.
- `pub fn load() -> Vec<Recent>`, `pub fn save(list: &[Recent]) -> std::io::Result<()>` (create the folder; errors are ignored by callers).
- Thumbnail: `cache_base()/recent/<hex of DefaultHasher(path)>.rgba`: 8-byte header (w, h as u32 LE) then RGBA. `pub fn thumb_path(path: &Path) -> Option<PathBuf>`.

- [ ] **Step 1: Tests:** `round_trips_unicode_and_spaces` (path `/Users/a/Відео/my clip.mov`), `push_dedupes_and_keeps_ten`, `parse_skips_broken_lines`, `save_into_missing_folder_creates_it` (temp dir under `std::env::temp_dir()` with a unique name, removed after), `unwritable_folder_returns_err` (a file where the folder should be).
- [ ] **Step 2:** Fail. **Step 3:** Implement; start screen shows up to 5 existing entries: thumbnail (or film icon), name, `codec · WxH · m:ss`, relative time (`recent.hours`, `recent.yesterday`, date), ⌘1–⌘5 caps; ↑↓ moves the highlight, ⏎ opens.
- [ ] **Step 4:** Pass; open three files, restart, the list shows them newest first; delete one file: it disappears.
- [ ] **Step 5:** Commit `feat(start): recent files`.

### Task 16: Save this frame

**Files:** Create `src/ffmpeg/frame.rs`, `src/ffmpeg/tests/frame.rs`; Modify `src/ffmpeg/mod.rs`, `src/app/commands.rs` (`SaveFrame`), `src/app/trim_menu.rs`.

**Interfaces:** `pub fn frame_args(src: &Path, t: f64, crop: Option<[u32; 4]>, dst: &Path) -> Vec<String>` → `-hide_banner -nostdin -y -v error -ss {t:.3} -i src -frames:v 1 [-vf crop=w:h:x:y] dst`; `pub fn save_frame(src, t, crop, dst) -> Result<(), String>` (blocking; run on a thread; logs as kind Frame once Task 17 lands). App: save dialog default `<stem>-<h-m-s.ms>.png` beside the source (`fmt_time` with `:`/`.` → `-`); refuse the source path like exports; toast on success with Show in folder.

- [ ] **Step 1: Tests:** `args_crop_then_png` (crop present → `-vf crop=640:360:2:4` before dst), `saves_a_png_of_the_crop` (fixture, crop 64×36 → `ffprobe` reports 64,36 and codec png).
- [ ] **Step 2:** Fail. **Step 3:** Implement. **Step 4:** Pass; ⌘⇧E in the app.
- [ ] **Step 5:** Commit `feat(export): save the current frame as PNG`.

### Task 17: ffmpeg log

**Files:** Create `src/ffmpeg/log.rs`, `src/ffmpeg/tests/log.rs`; Modify `src/ffmpeg/{thumbs,estimate,run,encode,graph,frame}.rs`, `src/strip/run.rs`.

**Interfaces:**
- `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Kind { Thumbnails, Scan, Estimate, Export, Frame, Probe }`
- `#[derive(Clone)] pub struct Run { pub args: String, pub state: RunState, pub output: VecDeque<String> }`, `pub enum RunState { Running, Exited { code: Option<i32>, took: Duration } }`
- `#[derive(Clone)] pub struct Entry { pub id: u64, pub at: SystemTime, pub kind: Kind, pub runs: Vec<Run>, pub progress: Option<String> }`
- `pub fn begin(kind: Kind) -> u64`: a new Thumbnails entry replaces the newest entry when that one is also Thumbnails and finished (a scrolling filmstrip keeps one entry).
- `pub fn spawn(entry: u64, cmd: &mut Command) -> std::io::Result<(Child, usize)>`: records `program args…` (args with spaces quoted) then spawns.
- `pub fn line(entry: u64, run: usize, text: &str)`, `pub fn progress(entry: u64, text: &str)`, `pub fn exited(entry: u64, run: usize, status: Option<ExitStatus>, took: Duration)`
- `pub fn entries() -> Vec<Entry>`, `pub fn generation() -> u64`, `pub fn busy() -> bool`
- Limits: `const ENTRIES: usize = 200; const LINES: usize = 200;` (oldest dropped).
- Each call site wraps its existing `spawn()`; stderr lines go to `line` (thumbs: stderr changes from `null` to `piped` and is read after stdout); export progress lines from `-progress pipe:1` go to `progress` (the latest block joined on one line).

- [ ] **Step 1: Tests:** `keeps_200_entries`, `keeps_200_lines_per_run`, `thumbnail_refreshes_share_one_entry`, `quotes_args_with_spaces` (`-i "/a b/c.mov"`), `export_run_is_logged` (fixture export → one Export entry, one run, `Exited { code: Some(0), .. }`).
- [ ] **Step 2:** Fail. **Step 3:** Implement (one `static LOG: Mutex<Log>`; never hold the lock across a spawn or a read).
- [ ] **Step 4:** Pass; gates for all three targets.
- [ ] **Step 5:** Commit `feat(ffmpeg): log every command Chyk runs`.

### Task 18: Log window

**Files:** Create `src/app/log_window.rs` (+ `src/app/log_entry.rs` if needed for size); Modify `src/app/frame.rs` (call it every `ui`), `src/app/status_bar.rs` (button toggles `log_open`), `src/app/commands.rs` (`ToggleLog`).

**Interfaces:** `impl App { pub(super) fn log_window(&mut self, ctx: &egui::Context) }` using `ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("chyk-ffmpeg-log"), ViewportBuilder::default().with_title("Chyk — ffmpeg log").with_inner_size([880.0, 500.0]).with_min_inner_size([520.0, 300.0]), …)`. Inside: header (title, read-only note, kind filter segmented: All/Export/Scan/Estimate/Thumbnails/Frame/Probe, Copy), scrolling body sticking to the bottom; entry = time, kind chip, result (✓ duration · exit n in red · live progress), `$ command` (a Thumbnails/Estimate batch shows the first run and "+N"), output lines (errors red). Text is selectable. Close request, Esc or ⌘W inside it → `log_open = false`. The main window closing ends the app, which closes it. Repaint only when `log::generation()` changed or input arrived.

- [ ] **Step 1:** Spike first: open the viewport empty while a video plays; confirm mpv keeps drawing (no black or frozen frames) on macOS; move/resize it; quit with it open. If mpv breaks, stop and report before continuing.
- [ ] **Step 2:** Implement; then the Review Focus 5 check: start an export, close the log, reopen (entries there, progress live), quit with it open (no hang).
- [ ] **Step 3:** Gates (three targets); commit `feat(ui): ffmpeg log window`.

### Task 19: Docs

**Files:** `SPEC.md` (UI section: header, status bar, keys table, toasts, log, recent, frame, undo; replace the "proposed revision" note), `features.md`, `design/concepts/` captions if behavior moved.

- [ ] **Step 1:** Update; `cargo nextest run --workspace` green; commit `docs: keyboard-first UI in SPEC and features`.

### Task 20: Release

- [ ] **Step 1:** Run the rust-release gates (`cargo fmt --all --check`, clippy ×3 targets, `cargo nextest run --workspace`, `cargo audit` exit 0).
- [ ] **Step 2:** Bump `Cargo.toml` to `1.3.0` (new features; confirm the number with the user), `cargo check` for the lockfile, commit `chore: release 1.3.0`, signed tag `v1.3.0` "Chyk 1.3.0", push main and the tag. Don't watch CI.
