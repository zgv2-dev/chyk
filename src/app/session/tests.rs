use super::*;

#[test]
fn round_trip() {
    let r = |a, b, c, d| Rect::from_min_max(pos2(a, b), pos2(c, d));
    let s = Session {
        path: "/Users/me/Movies/a b = c.mp4".into(),
        trim: (1.25, 9.5),
        crop: Some(r(0.1, 0.2, 0.6, 0.9)),
        preset: 3,
        speed: 1.5,
        strip: true,
        pad: 0.25,
        zones: vec![r(0.0, 0.0, 0.1, 0.1), r(0.5, 0.5, 0.7, 0.8)],
        format: 2,
        quality: 1,
        size: 4,
        precise: true,
        mute: true,
        pos: 4.0,
        exported: true,
    };
    assert_eq!(Session::parse(&s.to_text()), Some(s));
    assert_eq!(Session::parse("trim = 1 2\n"), None);
}
