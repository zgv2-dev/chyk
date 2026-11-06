use super::*;

#[test]
fn formats() {
    assert_eq!(ratio(1920, 1080), "16:9");
    assert_eq!(ratio(1080, 1920), "9:16");
    assert_eq!(ratio(1998, 1080), "1.85:1");
    assert_eq!(trim_float(29.97003, 3), "29.97");
    assert_eq!(trim_float(48.0, 1), "48");
    assert_eq!(bitrate(47_300_000.0), "47.3 Mb/s");
    assert_eq!(bitrate(256_000.0), "256 kb/s");
}
