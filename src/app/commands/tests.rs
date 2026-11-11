use super::*;

/// One key event through a real egui context; the commands it triggers.
fn press(mods: Modifiers, key: Key, typing: bool) -> Vec<Cmd> {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        }],
        ..Default::default()
    };
    let mut out = Vec::new();
    let mut full = ctx.run_ui(input, |ui| out = pressed(ui.ctx(), typing));
    full.textures_delta.clear();
    out
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
    assert_eq!(press(Modifiers::COMMAND, Key::S, true), [Cmd::Export]);
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
