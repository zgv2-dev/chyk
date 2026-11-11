use super::*;

fn key(mods: Modifiers, key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: mods,
    }
}

/// Events through a real egui context, one pass each: the commands of the last pass, and whether
/// a key event is still in its input afterwards. `text`: a text field has the keyboard.
fn feed(events: &[egui::Event], typing: bool, text: bool) -> (Vec<Cmd>, bool) {
    let ctx = egui::Context::default();
    let (mut out, mut left) = (Vec::new(), false);
    for e in events {
        let input = egui::RawInput {
            events: vec![e.clone()],
            ..Default::default()
        };
        let mut full = ctx.run_ui(input, |ui| {
            out = pressed(ui.ctx(), typing, text);
            left = ui.input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { .. }))
            });
        });
        full.textures_delta.clear();
    }
    (out, left)
}

fn press(mods: Modifiers, k: Key, typing: bool) -> Vec<Cmd> {
    feed(&[key(mods, k)], typing, false).0
}

const N: Modifiers = Modifiers::NONE;
const S: Modifiers = Modifiers::SHIFT;

#[test]
fn shift_i_goes_to_start_and_does_not_set_in() {
    assert_eq!(press(S, Key::I, false), [Cmd::GoIn]);
    assert_eq!(press(S, Key::O, false), [Cmd::GoOut]);
    assert_eq!(press(N, Key::I, false), [Cmd::SetIn]);
}

#[test]
fn shifted_keys_never_fire_their_plain_command() {
    assert_eq!(press(S, Key::ArrowLeft, false), [Cmd::Back5]);
    assert_eq!(press(S, Key::ArrowRight, false), [Cmd::Forward5]);
    assert_eq!(press(S, Key::C, false), [Cmd::RemoveCrop]);
    assert_eq!(press(S, Key::S, false), [Cmd::StripOff]);
    assert_eq!(press(S, Key::Backslash, false), [Cmd::TimelineWhole]);
    assert_eq!(press(N, Key::Backslash, false), [Cmd::TimelineToSelection]);
}

#[test]
fn shift_pipe_is_the_whole_timeline() {
    // egui-winit sends the logical key: Shift+\ is `|` on US-style layouts.
    assert_eq!(press(S, Key::Pipe, false), [Cmd::TimelineWhole]);
}

#[test]
fn presets_need_a_bare_digit() {
    assert_eq!(press(N, Key::Num1, false), [Cmd::CropPreset(0)]);
    assert!(press(S, Key::Num1, false).is_empty());
    assert_eq!(
        press(Modifiers::COMMAND, Key::Num1, false),
        [Cmd::OpenRecent(0)]
    );
}

#[test]
fn typing_blocks_single_keys_but_not_command_keys() {
    assert!(press(N, Key::I, true).is_empty());
    assert!(press(N, Key::ArrowLeft, true).is_empty());
    assert!(press(N, Key::Space, true).is_empty());
    assert_eq!(press(Modifiers::COMMAND, Key::S, true), [Cmd::Export]);
}

#[test]
fn undo_in_a_text_field_is_the_fields_own() {
    let mut chords = vec![
        (Modifiers::COMMAND, Key::Z),
        (Modifiers::COMMAND | S, Key::Z),
    ];
    if !cfg!(target_os = "macos") {
        chords.push((Modifiers::CTRL, Key::Y));
    }
    for (m, k) in chords {
        assert_eq!(feed(&[key(m, k)], true, true), (vec![], true));
    }
    // Other command keys still work there.
    let save = feed(&[key(Modifiers::COMMAND, Key::S)], true, true);
    assert_eq!(save, (vec![Cmd::Export], false));
}

#[test]
fn repeats_fire_only_step_commands() {
    // A second press with no release between: egui marks it a repeat.
    let repeat = |m, k| feed(&[key(m, k), key(m, k)], false, false).0;
    assert!(repeat(N, Key::A).is_empty());
    assert!(repeat(Modifiers::COMMAND, Key::L).is_empty());
    assert!(repeat(N, Key::Questionmark).is_empty());
    assert_eq!(repeat(N, Key::ArrowRight), [Cmd::FrameNext]);
    assert_eq!(repeat(Modifiers::COMMAND, Key::Z), [Cmd::Undo]);
}

#[test]
fn question_mark_opens_the_sheet_with_or_without_shift() {
    assert_eq!(press(S, Key::Slash, false), [Cmd::Shortcuts]);
    assert_eq!(press(S, Key::Questionmark, false), [Cmd::Shortcuts]);
    assert_eq!(press(N, Key::Questionmark, false), [Cmd::Shortcuts]);
}

#[test]
fn plus_and_comma_keep_working_shifted() {
    assert_eq!(press(S, Key::Plus, false), [Cmd::ZoomIn]);
    assert_eq!(press(S, Key::Comma, false), [Cmd::Slower]);
}

#[test]
fn command_chords_pick_the_most_specific() {
    assert_eq!(press(Modifiers::COMMAND | S, Key::Z, false), [Cmd::Redo]);
    assert_eq!(press(Modifiers::COMMAND, Key::Z, false), [Cmd::Undo]);
    assert_eq!(
        press(Modifiers::COMMAND | S, Key::R, false),
        [Cmd::RevealExport]
    );
}
