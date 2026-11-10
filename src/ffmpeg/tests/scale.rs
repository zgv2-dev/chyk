//! Export resolution presets.

use super::fixture::{Fixture, probe, wait};
use super::*;

#[test]
fn scaled_sets_the_short_side_and_keeps_sides_even() {
    assert_eq!(scaled([1920, 1080], 720), [1280, 720]);
    assert_eq!(scaled([1280, 720], 2160), [3840, 2160]);
    // Portrait: the short side is the width.
    assert_eq!(scaled([1080, 1920], 720), [720, 1280]);
    assert_eq!(scaled([640, 480], 144), [192, 144]);
    // An odd crop still comes out even.
    assert_eq!(scaled([1918, 1080], 720), [1278, 720]);
    assert_eq!(scaled([0, 0], 720), [2, 2]);
}

#[test]
fn sizes_start_with_original_then_shrink() {
    assert_eq!(SIZES[0].1, None);
    let shorts: Vec<u32> = SIZES.iter().filter_map(|s| s.1).collect();
    assert!(shorts.windows(2).all(|w| w[0] > w[1]), "{shorts:?}");
}

#[test]
fn scale_follows_the_crop() {
    let mut j = job(Some([640, 360, 2, 4]), "out.mp4");
    j.scale = Some([1280, 720]);
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(a.contains("-vf crop=640:360:2:4,scale=1280:720 "), "{a}");
    j.keep = Some(vec![(1.5, 2.0)]);
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(
        a.contains("[0:v:0]crop=640:360:2:4,scale=1280:720,select="),
        "{a}"
    );
    j.crop = None;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(a.contains("[0:v:0]scale=1280:720,select="), "{a}");
}

/// Needs `ffmpeg` and `ffprobe` on PATH. A scaled export re-encodes even without a crop.
#[test]
fn scaled_export_has_the_new_size() {
    let f = Fixture::new("scale");
    let job = Job {
        end: 1.0,
        scale: Some([256, 144]),
        ..f.job("scale.mkv")
    };
    let dst = job.dst.clone();
    wait(export(job, Context::default())).unwrap();
    assert_eq!(probe(&dst, "v:0", "stream=width,height"), "256,144");
}
