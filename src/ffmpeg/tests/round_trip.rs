//! End-to-end exports. Need `ffmpeg` and `ffprobe` on PATH.

use super::fixture::{Fixture, probe, wait};
use super::*;

#[test]
fn copy_and_crop() {
    let f = Fixture::new("copy");
    for (crop, name, size, want) in [
        // Stream copy starts at a keyframe before IN, so allow up to one GOP (1 s) extra.
        (None, "copy.mkv", "640,360", 1.9..3.4),
        (Some([320, 180, 10, 20]), "crop.mp4", "320,180", 1.85..2.15),
    ] {
        let job = Job {
            start: 1.0,
            end: 3.0,
            crop,
            ..f.job(name)
        };
        let dst = job.dst.clone();
        wait(export(job, Context::default()))
            .map(|_| ())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(probe(&dst, "v:0", "stream=width,height"), size, "{name}");
        let dur: f64 = probe(&dst, "v:0", "format=duration").parse().unwrap();
        assert!(want.contains(&dur), "{name}: duration {dur}");
    }
}

/// Compress the whole clip, so the output is comparable to the source.
#[test]
fn compress_shrinks() {
    let f = Fixture::new("compress");
    let src_len = std::fs::metadata(&f.src).unwrap().len();
    let job = Job {
        quality: COMPRESS,
        ..f.job("compress.mp4")
    };
    let dst = job.dst.clone();
    let out_len = wait(export(job, Context::default())).unwrap();
    assert!(out_len < src_len, "compress: {out_len} >= {src_len}");
    assert_eq!(out_len, std::fs::metadata(&dst).unwrap().len());
}

/// Strip within a trim that starts mid-GOP (1.5 s), keeping 1.5..2 s and 2.5..3 s. The filter
/// times are relative to the `-ss` point, so this checks that they line up.
#[test]
fn strip_keeps_audio() {
    let f = Fixture::new("strip");
    let job = Job {
        start: 1.5,
        end: 3.0,
        keep: Some(vec![(1.5, 2.0), (2.5, 3.0)]),
        ..f.job("strip.mp4")
    };
    let dst = job.dst.clone();
    wait(export(job, Context::default())).unwrap();
    let dur: f64 = probe(&dst, "v:0", "format=duration").parse().unwrap();
    assert!((0.9..=1.1).contains(&dur), "strip duration {dur}");
    assert_eq!(probe(&dst, "a", "stream=codec_name"), "aac");
    let adur: f64 = probe(&dst, "a:0", "stream=duration").parse().unwrap();
    assert!((0.9..=1.1).contains(&adur), "strip audio duration {adur}");
}

/// Variable frame rate, like a screen recording: no frames at all between 1.5 s and 2.5 s.
/// Keeping 1..3 s must keep that hold as time, or the video ends long before the audio.
#[test]
fn strip_keeps_vfr_holds() {
    let f = Fixture::new("vfr");
    let vfr = f.dir.join("vfr.mp4");
    let ok = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=25:duration=4",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=4",
            "-vf",
            "select='not(between(t,1.5,2.5))'",
            "-fps_mode",
            "passthrough",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&vfr)
        .status()
        .unwrap();
    assert!(ok.success());
    let job = Job {
        src: vfr,
        keep: Some(vec![(0.5, 1.0), (1.2, 3.0)]),
        ..f.job("strip-vfr.mp4")
    };
    let dst = job.dst.clone();
    wait(export(job, Context::default())).unwrap();
    let vdur: f64 = probe(&dst, "v:0", "stream=duration").parse().unwrap();
    assert!(
        (2.2..=2.4).contains(&vdur),
        "vfr strip video duration {vdur}"
    );
}

/// Retiming 4 s: at 2× it lands near 2 s, at 0.5× near 8 s, audio kept either way.
#[test]
fn speed_retimes() {
    let f = Fixture::new("speed");
    for (speed, name, want) in [(2.0, "fast.mp4", 1.8..=2.2), (0.5, "slow.mp4", 7.8..=8.2)] {
        let job = Job {
            speed,
            ..f.job(name)
        };
        let dst = job.dst.clone();
        wait(export(job, Context::default())).unwrap();
        let dur: f64 = probe(&dst, "v:0", "format=duration").parse().unwrap();
        assert!(want.contains(&dur), "{name}: duration {dur}");
        assert_eq!(probe(&dst, "a", "stream=codec_name"), "aac", "{name}");
    }
}
