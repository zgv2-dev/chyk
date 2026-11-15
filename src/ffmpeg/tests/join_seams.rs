//! The seam trial before a lossless join, end to end. Needs `ffmpeg` on PATH.

use super::fixture::{Fixture, wait};
use super::join::{joined, part};
use super::*;
use crate::ffmpeg::join_check::seam_join;

#[test]
fn stops_a_lossless_join_whose_seam_wont_decode() {
    let f = Fixture::new("join-bad-seam");
    // The same clip with garbage early on, where the seam trial decodes it.
    let bad = f.dir.join("bad.mkv");
    let mut bytes = std::fs::read(&f.src).unwrap();
    for at in (40_000..200_000).step_by(5_000) {
        bytes[at..at + 500].fill(0xab);
    }
    std::fs::write(&bad, bytes).unwrap();
    let part = |src: &Path| Part {
        src: src.to_path_buf(),
        start: 0.0,
        end: 4.0,
        fit: Fit::Fit,
        keep: None,
        audio: true,
    };
    let job = Job {
        join: Some(Join {
            parts: vec![part(&f.src), part(&bad)],
            frame: [640, 360],
            fps: None,
        }),
        ..f.job("bad.mp4")
    };
    let dst = job.dst.clone();
    assert_eq!(
        wait(export(job, Context::default())),
        Err("lossless join failed".into())
    );
    assert!(!dst.exists());
}

#[test]
fn an_open_gop_clip_joins_losslessly_whole() {
    // Its seam trial starts the first clip's tail on an open-GOP I frame, which errors without
    // any join; that alone mustn't stop the copy.
    let f = Fixture::new("join-open-gop");
    let src = f.dir.join("open.mkv");
    let ok = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .args([
            "testsrc2=size=640x360:rate=25:duration=6",
            "-c:v",
            "libx264",
            "-bf",
            "3",
        ])
        .args(["-x264-params", "open_gop=1:keyint=50"])
        .arg(&src)
        .status()
        .unwrap();
    assert!(ok.success());
    let part = Part {
        src,
        start: 0.0,
        end: 6.0,
        fit: Fit::Fit,
        keep: None,
        audio: false,
    };
    let job = Job {
        join: Some(Join {
            parts: vec![part.clone(), part],
            frame: [640, 360],
            fps: None,
        }),
        end: 6.0,
        ..f.job("open.mkv")
    };
    wait(export(job, Context::default())).unwrap();
}

#[test]
fn seam_trial_keeps_only_the_seams() {
    let job = joined(vec![
        part("a.mp4", 0.0, 10.0, Fit::Fit, true),
        part("b.mp4", 1.0, 30.0, Fit::Fit, true),
        part("c.mp4", 0.0, 3.0, Fit::Fit, true),
        part("d.mp4", 5.0, 20.0, Fit::Fit, true),
    ]);
    let s = seam_join(job.join.as_ref().unwrap());
    let spans: Vec<_> = s
        .parts
        .iter()
        .map(|p| (p.src.clone(), p.start, p.end))
        .collect();
    let at = |f: &str, a, b| (std::path::PathBuf::from(f), a, b);
    assert_eq!(
        spans,
        [
            at("a.mp4", 8.0, 10.0),
            at("b.mp4", 1.0, 3.0),
            at("b.mp4", 28.0, 30.0),
            at("c.mp4", 0.0, 3.0),
            at("d.mp4", 5.0, 7.0),
        ]
    );
}

#[test]
fn seam_key_tells_a_tail_from_a_trimmed_start() {
    // The same 2 s window, once the tail of a longer clip and once the whole trimmed clip.
    let key = |start| {
        let job = joined(vec![
            part("a.mp4", start, 6.0, Fit::Fit, true),
            part("b.mp4", 0.0, 6.0, Fit::Fit, true),
        ]);
        crate::ffmpeg::seam_key(&job).unwrap()
    };
    assert_ne!(key(0.0), key(4.0));
}
