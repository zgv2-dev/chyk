use super::*;

#[test]
fn forty_two_seconds_on_a_wide_bar() {
    assert_eq!(ruler_steps(42.0, 1248.0), (1.0, 5.0));
}

#[test]
fn five_minutes() {
    assert_eq!(ruler_steps(315.0, 1248.0), (5.0, 30.0));
}

#[test]
fn one_second() {
    assert_eq!(ruler_steps(1.0, 1248.0), (0.02, 0.1));
}

#[test]
fn ten_hours_narrow() {
    assert_eq!(ruler_steps(36000.0, 300.0), (3600.0, 3600.0));
}
