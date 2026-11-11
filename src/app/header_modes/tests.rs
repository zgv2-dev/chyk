use super::*;

#[test]
fn caps_need_room_and_hysteresis() {
    // Fits with spare room: on, whether or not they were shown.
    assert!(caps_fit(200.0, 100.0, false));
    assert!(caps_fit(200.0, 100.0, true));
    // Barely fits: stays on if shown, stays off if not (no flicker).
    assert!(caps_fit(110.0, 100.0, true));
    assert!(!caps_fit(110.0, 100.0, false));
    // Exactly zero slack with them shown is an overflow clamped by egui: off.
    assert!(!caps_fit(100.0, 100.0, true));
    // Does not fit: off either way.
    assert!(!caps_fit(99.0, 100.0, true));
    assert!(!caps_fit(99.0, 100.0, false));
}
