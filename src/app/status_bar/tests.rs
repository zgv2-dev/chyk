use super::*;

const NONE: ReasonInput = ReasonInput {
    crop: false,
    resize: false,
    strip: false,
    retime: false,
    precise: false,
    quality_high: true,
};

#[test]
fn copy_has_no_reasons() {
    assert!(reencode_reasons(&NONE).is_empty());
}

#[test]
fn crop_and_speed_are_named() {
    let a = ReasonInput {
        crop: true,
        retime: true,
        ..NONE
    };
    assert_eq!(reencode_reasons(&a), ["reason.crop", "reason.speed"]);
}

#[test]
fn time_left_extrapolates() {
    let ten = Duration::from_secs(10);
    assert_eq!(time_left(ten, 0.5), Some(ten));
    assert_eq!(time_left(ten, 0.01), None);
}
