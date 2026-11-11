use super::*;

fn base() -> Edits {
    ((1.0, 9.0), None, None, 1.0)
}

#[test]
fn only_speed_changed() {
    let mut now = base();
    now.3 = 1.5;
    assert_eq!(changed(&base(), &now, 0), [Chip::Speed(1.5)]);
}

#[test]
fn crop_and_trim() {
    let mut now = base();
    now.0 = (2.0, 8.0);
    now.1 = Some(crop::full());
    assert_eq!(
        changed(&base(), &now, 5),
        [Chip::Trim(2.0, 8.0), Chip::Crop(5)]
    );
}

#[test]
fn nothing_changed() {
    assert!(changed(&base(), &base(), 0).is_empty());
}
