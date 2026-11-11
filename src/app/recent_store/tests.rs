use super::*;

#[test]
fn dates() {
    assert_eq!(date(0), "1970-01-01");
    assert_eq!(date(1_709_942_400), "2024-03-09");
    assert_eq!(date(1_709_942_400 + 86_399), "2024-03-09");
}

#[test]
fn clock_rounds() {
    assert_eq!(clock(65.4), "1:05");
}

#[test]
fn height_zero_without_rows() {
    assert_eq!(height(0), 0.0);
    assert!(height(1) > 0.0);
}
