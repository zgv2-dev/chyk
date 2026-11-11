use super::*;

#[test]
fn unexported_edits_enter_exports_and_cmd_backspace_discards() {
    for c in [Confirm::Quit, Confirm::CloseVideo] {
        assert_eq!(modal_key(&c, true, false), Some(Choice::Export));
        assert_eq!(modal_key(&c, false, true), Some(Choice::Discard));
        assert_eq!(modal_key(&c, false, false), None);
    }
}

#[test]
fn another_file_enter_opens_a_window_and_cmd_backspace_replaces() {
    let c = Confirm::Open(PathBuf::from("a.mp4"));
    assert_eq!(modal_key(&c, true, false), Some(Choice::NewWindow));
    assert_eq!(modal_key(&c, false, true), Some(Choice::Replace));
}
