//! End-to-end joins. Need `ffmpeg` and `ffprobe` on PATH.

use super::fixture::{Fixture, probe, wait};
use super::*;

/// A 2 s square 30 fps clip without audio.
fn square(dir: &Path) -> PathBuf {
    let src = dir.join("square.mp4");
    let ok = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=480x480:rate=30:duration=2",
        ])
        .args(["-c:v", "libx264"])
        .arg(&src)
        .status()
        .unwrap();
    assert!(ok.success());
    src
}

#[test]
fn joins_different_clips() {
    let f = Fixture::new("join");
    let sq = square(&f.dir);
    for (fit, name) in [
        (Fit::Fit, "fit.mp4"),
        (Fit::Fill, "fill.mkv"),
        (Fit::Stretch, "stretch.mov"),
        (Fit::Place([42, -20]), "place.mp4"),
    ] {
        let job = Job {
            join: Some(Join {
                parts: vec![
                    Part {
                        src: f.src.clone(),
                        start: 1.0,
                        end: 3.0,
                        fit,
                        keep: None,
                        audio: false,
                    },
                    Part {
                        src: sq.clone(),
                        start: 0.0,
                        end: 2.0,
                        fit,
                        keep: None,
                        audio: false,
                    },
                ],
                frame: [640, 360],
                fps: Some(30.0),
            }),
            ..f.job(name)
        };
        let dst = job.dst.clone();
        wait(export(job, Context::default())).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            probe(&dst, "v:0", "stream=width,height"),
            "640,360",
            "{name}"
        );
        let dur: f64 = probe(&dst, "v:0", "format=duration").parse().unwrap();
        assert!((3.8..4.3).contains(&dur), "{name}: duration {dur}");
        // The fixture's audio continues through the silent clip.
        assert!(
            !probe(&dst, "a:0", "stream=codec_name").is_empty(),
            "{name}: no audio stream"
        );
        // MKV reports no stream duration.
        if let Ok(adur) = probe(&dst, "a:0", "stream=duration").parse::<f64>() {
            assert!((3.8..4.3).contains(&adur), "{name}: audio {adur}");
        }
    }
}

#[test]
fn joins_matching_clips_losslessly() {
    let f = Fixture::new("join-copy");
    let part = |start, end| Part {
        src: f.src.clone(),
        start,
        end,
        fit: Fit::Fit,
        keep: None,
        audio: true,
    };
    let job = Job {
        join: Some(Join {
            parts: vec![part(0.0, 2.0), part(2.0, 4.0)],
            frame: [640, 360],
            fps: None,
        }),
        quality: 0,
        ..f.job("copy.mkv")
    };
    let dst = job.dst.clone();
    wait(export(job, Context::default())).unwrap();
    let dur: f64 = probe(&dst, "v:0", "format=duration").parse().unwrap();
    assert!((3.8..4.4).contains(&dur), "duration {dur}");
    // Stream copy keeps the source's encode settings; a re-encode would be Lavc's x264 defaults.
    assert_eq!(probe(&dst, "v:0", "stream=codec_name"), "h264");
    // ffmpeg writes the hidden part file, renamed to `dst` once it finishes.
    let out = crate::ffmpeg::part::path(&dst)
        .to_string_lossy()
        .into_owned();
    let entry = crate::ffmpeg::log::entries()
        .into_iter()
        .find(|e| e.runs.iter().any(|r| r.args.contains(&out)))
        .expect("the export's entry");
    // The seam trial (a copy, then a decode), then the copy itself, with no re-encode after it.
    let args: Vec<&str> = entry.runs.iter().map(|r| r.args.as_str()).collect();
    assert_eq!(args.len(), 3, "{args:?}");
    assert!(
        args[0].contains("chyk-seams") && args[1].contains("-f null"),
        "{args:?}"
    );
    assert!(
        args[2].contains("-f concat") && args[2].contains(&out),
        "{args:?}"
    );
}
