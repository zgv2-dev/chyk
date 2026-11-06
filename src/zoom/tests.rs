use super::*;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn video_zoom_keeps_the_anchor() {
    let view = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 50.0));
    let p = pos2(25.0, 25.0);
    let (z, c) = video_zoom(1.0, pos2(0.5, 0.5), 2.0, Some((p, view)));
    assert_eq!(z, 2.0);
    let src = video_src(z, c);
    let a = screen_to_source(src, view, p);
    assert!(close(a.x, 0.25) && close(a.y, 0.5), "{a:?}");
    let back = source_to_screen(src, view, a);
    assert!(close(back.x, p.x) && close(back.y, p.y), "{back:?}");
}

#[test]
fn video_zoom_clamps() {
    assert_eq!(video_zoom(1.0, pos2(0.5, 0.5), 0.1, None).0, 1.0);
    assert_eq!(video_zoom(1.0, pos2(0.5, 0.5), 100.0, None).0, MAX_VIDEO);
}

#[test]
fn video_pan_uses_source_space() {
    let c = video_pan(pos2(0.5, 0.5), vec2(10.0, 0.0), vec2(100.0, 50.0), 2.0);
    assert!(close(c.x, 0.45) && close(c.y, 0.5), "{c:?}");
    // Hitting the edge clamps instead of drifting.
    let c = video_pan(pos2(0.3, 0.5), vec2(100.0, 0.0), vec2(100.0, 50.0), 2.0);
    assert!(close(c.x, 0.25), "{c:?}");
}

#[test]
fn contain_keeps_the_crop_in_view() {
    let crop = Rect::from_min_max(pos2(0.6, 0.1), pos2(0.9, 0.35));
    // 8x would show a 1/8 slice; the crop is 0.3 wide, so zoom stops at 1/0.3.
    let (z, c) = contain(8.0, pos2(0.1, 0.9), crop);
    assert!(close(z, 1.0 / 0.3), "{z}");
    let src = video_src(z, c);
    assert!(src.expand(1e-4).contains_rect(crop), "{src:?}");
    // A zoom that already shows the crop stays put.
    let (z, c) = contain(2.0, pos2(0.7, 0.25), crop);
    assert_eq!(z, 2.0);
    assert!(video_src(z, c).expand(1e-4).contains_rect(crop));
    // A full-frame crop allows no zoom at all.
    assert_eq!(
        contain(
            4.0,
            pos2(0.3, 0.3),
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
        )
        .0,
        1.0
    );
}

#[test]
fn timeline_view_clamps() {
    assert_eq!(timeline_view(10.0, 1.0, 0.0), (0.0, 10.0));
    assert_eq!(timeline_view(10.0, 2.0, 5.0), (2.5, 7.5));
    assert_eq!(timeline_view(10.0, 2.0, 0.0), (0.0, 5.0));
    assert_eq!(timeline_view(0.0, 3.0, 0.0), (0.0, 0.0));
}

#[test]
fn timeline_zoom_keeps_the_anchor() {
    let (z, c) = timeline_zoom(10.0, 1.0, 2.0, 2.5, 0.25);
    assert_eq!(z, 2.0);
    let (a, b) = timeline_view(10.0, z, c);
    assert!(close((a + (b - a) * 0.25) as f32, 2.5), "{a}..{b}");
}

#[test]
fn timeline_fit_selection_zooms_to_it() {
    let (z, c) = timeline_fit_selection(100.0, 20.0, 30.0);
    assert_eq!(z, 10.0);
    assert_eq!(timeline_view(100.0, z, c), (20.0, 30.0));
}
