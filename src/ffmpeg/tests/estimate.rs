//! Output size from encoded samples.

use super::fixture::{Fixture, probe, wait};
use super::*;

#[test]
fn sample_windows_spread_over_kept_time() {
    // Short enough: sample all of it.
    let keep = [(1.0, 2.0), (5.0, 6.5)];
    assert_eq!(sample_windows(&keep, 6, 1.0), keep);
    // One long interval: evenly spaced centers, one second each.
    let w = sample_windows(&[(0.0, 60.0)], 6, 1.0);
    assert_eq!(
        w,
        [5.0, 15.0, 25.0, 35.0, 45.0, 55.0].map(|c| (c - 0.5, c + 0.5))
    );
    // Windows stay inside their interval and in order.
    let keep = [(0.0, 3.0), (10.0, 10.4), (20.0, 30.0)];
    let w = sample_windows(&keep, 4, 1.0);
    assert_eq!(w.len(), 4);
    assert!(w.windows(2).all(|p| p[0].1 <= p[1].0), "{w:?}");
    for (a, b) in &w {
        assert!(keep.iter().any(|k| k.0 <= *a && *b <= k.1), "{w:?}");
    }
}

/// Needs `ffmpeg` and `ffprobe` on PATH. The sampled rate predicts the real export's size.
#[test]
fn sample_rate_predicts_the_export() {
    let f = Fixture::new("estimate");
    let job = Job {
        quality: 1,
        scale: Some([320, 180]),
        ..f.job("estimate.mkv")
    };
    let rate = sample_rate(&job, &AtomicBool::new(false)).expect("rate");
    let real = wait(export(job.clone(), Context::default())).unwrap() as f64;
    let guess = rate * job.output_duration();
    assert!((0.5..2.0).contains(&(guess / real)), "{guess} vs {real}");
    // A stop request gives up.
    assert_eq!(sample_rate(&job, &AtomicBool::new(true)), None);
}

/// Real-file check, off by default: `CHYK_REAL=video.mov [CHYK_REAL_SECS=60] [CHYK_REAL_Q=0]
/// cargo test real_file_estimate -- --ignored --nocapture`. Exports the first CHYK_REAL_SECS at
/// 1080p in each quality and prints the sampled guess next to the real size.
#[test]
#[ignore]
fn real_file_estimate() {
    let Some(src) = std::env::var_os("CHYK_REAL").map(PathBuf::from) else {
        return;
    };
    let secs = std::env::var("CHYK_REAL_SECS").map_or(60.0, |s| s.parse().unwrap());
    let size = probe(&src, "v:0", "stream=width,height");
    let [w, h]: [u32; 2] = size
        .split(',')
        .map(|v| v.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let dst = std::env::temp_dir().join("chyk-real-estimate.mp4");
    let only = std::env::var("CHYK_REAL_Q")
        .ok()
        .map(|q| q.parse::<usize>().unwrap());
    for quality in (0..QUALITIES.len()).filter(|q| only.is_none_or(|o| o == *q)) {
        let job = Job {
            src: src.clone(),
            dst: dst.clone(),
            start: 0.0,
            end: secs,
            codec: "h264".into(),
            scale: Some(scaled([w, h], 1080)),
            quality,
            ..job(None, "")
        };
        let t = std::time::Instant::now();
        let guess = sample_rate(&job, &AtomicBool::new(false)).unwrap() * job.output_duration();
        let sampled = t.elapsed().as_secs_f64();
        let real = wait(export(job, Context::default())).unwrap() as f64;
        println!(
            "quality {quality}: guess {:.1} MB ({sampled:.1}s), real {:.1} MB, ratio {:.2}",
            guess / 1e6,
            real / 1e6,
            guess / real
        );
    }
    let _ = std::fs::remove_file(dst);
}
