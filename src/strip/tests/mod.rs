use super::*;

mod scans;

use eframe::egui::pos2;

/// A 320×180 source.
fn area(zones: Vec<Rect>, crop: Option<Rect>) -> Area {
    Area {
        aspect: 320.0 / 180.0,
        size: [320, 180],
        crop,
        zones,
        range: None,
    }
}

/// The final scan, after checking that the live `Partial` updates rebuild a prefix of it.
fn wait(run: Running) -> Scan {
    let mut partial: Vec<(f64, f64)> = Vec::new();
    let scan = loop {
        match run
            .rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .expect("scan timed out")
        {
            Msg::Done(r) => break r.unwrap(),
            Msg::Progress(_) => {}
            Msg::Partial {
                from, intervals, ..
            } => {
                partial.truncate(from);
                partial.extend(intervals);
            }
        }
    };
    // Every interval but the last sent one is final; the last may have grown since.
    let settled = partial.len().saturating_sub(1);
    assert_eq!(
        partial[..settled],
        scan.active[..settled.min(scan.active.len())]
    );
    scan
}

#[test]
fn pad_merges_and_clamps() {
    let active = [(1.0, 2.0), (4.0, 5.0)];
    assert_eq!(keep(&active, 0.0, 0.0, 10.0), [(1.0, 2.0), (4.0, 5.0)]);
    assert_eq!(keep(&active, 0.5, 0.0, 10.0), [(0.5, 2.5), (3.5, 5.5)]);
    // Two paddings meet: the gap disappears.
    assert_eq!(keep(&active, 1.0, 0.0, 10.0), [(0.0, 6.0)]);
    assert_eq!(keep(&active, 9.0, 0.0, 10.0), [(0.0, 10.0)]);
    assert_eq!(keep(&[(1.0, 2.0)], 0.0, 1.5, 2.5), [(1.5, 2.0)]);
    assert_eq!(keep(&[(1.0, 2.0)], -1.0, 0.0, 10.0), [(1.0, 2.0)]);
    assert!(keep(&[], 1.0, 0.0, 10.0).is_empty());
    assert!(keep(&[(1.0, 1.0)], 0.0, 0.0, 10.0).is_empty());
    // Adjacent intervals can appear at sample boundaries.
    assert_eq!(
        keep(&[(1.0, 2.0), (2.0, 3.0)], 0.0, 0.0, 10.0),
        [(1.0, 3.0)]
    );
}

#[test]
fn sample_sizes_keep_aspect() {
    assert_eq!(sample_size(1920.0 / 1080.0, [1920, 1080]), (320, 180));
    assert_eq!(sample_size(1080.0 / 1920.0, [1080, 1920]), (180, 320));
    assert_eq!(sample_size(4.0 / 3.0, [0, 0]), (276, 208));
    assert_eq!(sample_size(0.0, [0, 0]), (320, 180));
    assert_eq!(sample_size(f32::NAN, [0, 0]), (320, 180));
    // A thin 982×86 crop of a 1080p frame keeps nearly all of its pixels…
    assert_eq!(sample_size(982.0 / 86.0, [982, 86]), (810, 70));
    // …and a tiny one is never scaled up.
    assert_eq!(sample_size(1.0, [100, 100]), (100, 100));
}

#[test]
fn ignore_zones_mask_pixels() {
    let (w, h) = (320, 180);
    let mut a = vec![0u8; w * h];
    let mut b = a.clone();
    for y in 10..14 {
        for x in 10..14 {
            b[y * w + x] = 200;
        }
    }
    let all: Vec<usize> = (0..w * h).collect();
    assert_eq!(changed_pixels(&a, &b, &all), 16);

    // A 2-level shift is compression noise, not activity.
    a.fill(5);
    b.fill(7);
    assert_eq!(changed_pixels(&a, &b, &all), 0);

    let zone = Rect::from_min_max(
        pos2(8.0 / w as f32, 8.0 / h as f32),
        pos2(20.0 / w as f32, 20.0 / h as f32),
    );
    let pixels = compared_pixels(w, h, &[zone]);
    assert_eq!(changed_pixels(&a, &b, &pixels), 0);
    assert_eq!(compared_pixels(w, h, &[crate::crop::full()]).len(), 0);
}
