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

#[test]
fn labels_keep_one_decimal_count_per_step() {
    assert_eq!(tick_label(1.0, 0.5), "0:01.0");
    assert_eq!(tick_label(0.5, 0.5), "0:00.5");
    assert_eq!(tick_label(1.25, 0.25), "0:01.25");
    assert_eq!(tick_label(65.0, 5.0), "1:05");
    assert_eq!(tick_label(3600.0, 3600.0), "1:00:00");
}
