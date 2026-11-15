use super::*;

/// Needs `ffmpeg` on PATH. A 4×4 dot blinking in a 1280×720 frame is below the noise floor
/// of a whole-frame scan, but a crop around it is sampled at full detail and catches it.
#[test]
fn crop_scans_at_its_own_resolution() {
    let dir = std::env::temp_dir().join(format!("chyk-crop-{}", std::process::id()));
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
        pre: String::new(),
        range: None,
        thresholds: Default::default(),
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
    let dir = std::env::temp_dir().join(format!("chyk-strip-{}", std::process::id()));
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

    // Scanning only a trimmed range finds the same motion, at the same absolute times.
    let at = |range| Area {
        range,
        ..area(vec![], None)
    };
    let piece = |r| wait(scan(src.clone(), 6.0, at(Some(r)), Context::default()));
    let trimmed = piece((1.5, 4.0));
    assert!((trimmed.range.1 - 4.0).abs() < 1e-9, "{:?}", trimmed.range);
    // An end between two samples is still covered, even by a piece shorter than one step:
    // otherwise the gap left is rescanned forever.
    let off_grid = piece((1.5, 2.55));
    assert!(
        (off_grid.range.1 - 2.55).abs() < 1e-9,
        "{:?}",
        off_grid.range
    );
    let sliver = piece((2.5, 2.55));
    assert!((sliver.range.1 - 2.55).abs() < 1e-9, "{:?}", sliver.range);
    let after = piece((3.5, 6.0));
    assert!(after.active.is_empty(), "{:?}", after.active);
    // Stopped mid-motion, then the rest scanned and merged: same as one scan of the span.
    let left = piece((1.5, 2.5));
    assert!(
        left.active.iter().all(|&(_, b)| b <= 2.61),
        "{:?}",
        left.active
    );
    let joined = left.clone().merge(piece((left.range.1, 4.0)));
    assert_eq!(
        joined.active.len(),
        trimmed.active.len(),
        "{:?}",
        joined.active
    );
    assert!((joined.range.0 - 1.5).abs() < 1e-9 && (joined.range.1 - 4.0).abs() < 1e-9);

    let scan = wait(scan(src, 6.0, area(vec![], None), Context::default()));
    assert!(!scan.active.is_empty());
    for (a, b) in scan.active.iter().zip(&trimmed.active) {
        assert!(
            (a.0 - b.0).abs() < 0.15 && (a.1 - b.1).abs() < 0.15,
            "{a:?} vs {b:?}"
        );
    }
    assert_eq!(scan.active.len(), trimmed.active.len());
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
    let dir = std::env::temp_dir().join(format!("chyk-zone-{}", std::process::id()));
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

/// Needs `ffmpeg` on PATH. Screen recordings write a frame only when the picture changes, so the
/// last one can come well before the end. A piece of the trim after it decodes to no frames at
/// all: that is "nothing changed", not a failure (a failure gets rescanned over and over).
#[test]
fn piece_after_last_frame_is_still() {
    let dir = std::env::temp_dir().join(format!("chyk-tail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("sparse.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:r=1:d=3",
            "-c:v",
            "mpeg4",
        ])
        .arg(&src)
        .status()
        .unwrap();
    assert!(ok.success());
    let tail = Area {
        range: Some((2.5, 3.0)),
        ..area(vec![], None)
    };
    let scan = wait(scan(src, 3.0, tail, Context::default()));
    assert!(scan.active.is_empty(), "{:?}", scan.active);
    assert_eq!(scan.range, (2.5, 3.0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Needs `ffmpeg` on PATH. A joined clip is scanned as the export places it in the output frame,
/// so ignored areas drawn on that frame cover the right part of the clip in Fit and in Place.
#[test]
fn joined_clip_scans_in_the_output_frame() {
    let dir = std::env::temp_dir().join(format!("chyk-joinscan-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("box.mp4");
    // A 40×40 box blinking in the right half of a 320×180 clip.
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
            "color=c=white:s=40x40:r=10:d=4",
            "-filter_complex",
            "[0:v][1:v]overlay=240:70:enable='lt(mod(t,2),1)'[v]",
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
    let frame = [1280, 720];
    let at = |fit, zones| {
        let (pre, size) = ffmpeg::scan_fit(fit, frame, 0.5);
        Area {
            aspect: 16.0 / 9.0,
            size,
            crop: None,
            zones,
            pre,
            range: None,
            thresholds: Default::default(),
        }
    };
    let found = |fit, zones: Vec<Rect>| {
        !wait(scan(src.clone(), 4.0, at(fit, zones), Context::default()))
            .active
            .is_empty()
    };
    let rect = |x0, y0, x1, y1| Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
    // Fit scales the clip to the whole frame: the box lands at 0.75–0.875 across.
    assert!(found(ffmpeg::Fit::Fit, vec![rect(0.0, 0.0, 0.5, 1.0)]));
    assert!(!found(ffmpeg::Fit::Fit, vec![rect(0.7, 0.3, 0.9, 0.7)]));
    // Placed at 800, 400 at its own size: the box sits at 1040–1080 × 470–510.
    let place = ffmpeg::Fit::Place([800, 400]);
    assert!(found(place, vec![rect(0.62, 0.55, 0.75, 0.81)]));
    assert!(!found(place, vec![rect(0.80, 0.64, 0.86, 0.72)]));
    let _ = std::fs::remove_dir_all(&dir);
}
