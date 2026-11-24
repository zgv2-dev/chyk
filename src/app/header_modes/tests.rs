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

#[test]
fn squeeze_gives_way_in_order_and_holds() {
    let savings = [80.0, 130.0];
    // Room to spare: nothing given.
    assert_eq!(squeeze_level(-50.0, &savings, 0), 0);
    // Overflows: the time left goes first, then the labels, and no further.
    assert_eq!(squeeze_level(60.0, &savings, 0), 1);
    assert_eq!(squeeze_level(150.0, &savings, 0), 2);
    assert_eq!(squeeze_level(500.0, &savings, 0), 2);
    // Squeezed, it comes back only with room to spare beyond what it gave.
    assert_eq!(squeeze_level(70.0, &savings, 1), 1);
    assert_eq!(squeeze_level(-5.0, &savings, 1), 1);
    assert_eq!(squeeze_level(-10.0, &savings, 1), 0);
    assert_eq!(squeeze_level(200.0, &savings, 2), 2);
    assert_eq!(squeeze_level(75.0, &savings, 2), 2);
    assert_eq!(squeeze_level(70.0, &savings, 2), 1);
    // No export running: its level gives nothing, so the labels go.
    assert_eq!(squeeze_level(10.0, &[0.0, 130.0], 0), 2);
}
