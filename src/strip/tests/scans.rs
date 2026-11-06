use super::*;

/// Needs `ffmpeg` on PATH. A 4×4 dot blinking in a 1280×720 frame is below the noise floor
/// of a whole-frame scan, but a crop around it is sampled at full detail and catches it.
#[test]
fn crop_scans_at_its_own_resolution() {
    let dir = std::env::temp_dir().join(format!("glint-crop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("dot.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=1280x720:r=10:d=4",
            "-f",
            "lavfi",
            "-i",
            "color=c=white:s=4x4:r=10:d=4",
            "-filter_complex",
            "[0:v][1:v]overlay=1001:601:enable='lt(mod(t,2),1)'[v]",
            "-map",
            "[v]",
            "-c:v",
            "libx264",
            "-crf",
            "10",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&src)
        .status()
        .unwrap();
    assert!(ok.success());
    let at = |crop| Area {
        aspect: 16.0 / 9.0,
        size: [1280, 720],
        crop,
        zones: vec![],
    };
    let whole = wait(scan(src.clone(), 4.0, at(None), Context::default()));
    assert!(whole.active.is_empty(), "{:?}", whole.active);
    let corner = Rect::from_min_max(pos2(0.75, 0.75), pos2(1.0, 1.0));
    let cropped = wait(scan(src, 4.0, at(Some(corner)), Context::default()));
    assert!(!cropped.active.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Needs `ffmpeg` on PATH.
#[test]
fn scan_finds_motion_only() {
    let dir = std::env::temp_dir().join(format!("glint-strip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("motion.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:r=25:d=2",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=1",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:r=25:d=3",
            "-filter_complex",
            "[0:v][1:v][2:v]concat=n=3:v=1:a=0[v]",
            "-map",
            "[v]",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&src)
        .status()
        .unwrap();
    assert!(ok.success());

    let scan = wait(scan(src, 6.0, area(vec![], None), Context::default()));
    assert!(!scan.active.is_empty());
    // testsrc2 runs 2 s..3 s; sampling adds at most one 100 ms frame at each edge.
    for &(a, b) in &scan.active {
        assert!((1.8..3.3).contains(&a), "active starts at {a}");
        assert!((1.8..3.3).contains(&b), "active ends at {b}");
    }
    let total: f64 = scan.active.iter().map(|(a, b)| b - a).sum();
    assert!((1.0..2.2).contains(&total), "active duration {total}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Needs `ffmpeg` on PATH.
#[test]
fn ignore_zone_hides_a_clock() {
    let dir = std::env::temp_dir().join(format!("glint-zone-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("clock.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:r=10:d=4",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=40x40:r=10:d=4",
            "-filter_complex",
            "[0:v][1:v]overlay=10:10[v]",
            "-map",
            "[v]",
            "-c:v",
            "libx264",
            "-g",
            "10",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&src)
        .status()
        .unwrap();
    assert!(ok.success());

    // The only moving thing is the overlay in the top-left corner.
    let all = wait(scan(
        src.clone(),
        4.0,
        area(vec![], None),
        Context::default(),
    ));
    assert!(!all.active.is_empty());

    let clock = Rect::from_min_max(pos2(0.0, 0.0), pos2(0.2, 0.3));
    let masked = wait(scan(src, 4.0, area(vec![clock], None), Context::default()));
    assert!(masked.active.is_empty(), "{:?}", masked.active);
    let _ = std::fs::remove_dir_all(&dir);
}
