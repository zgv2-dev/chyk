//! Joining clips: arguments, stream checks, and end-to-end exports (need `ffmpeg`/`ffprobe`).

use super::*;

fn part(src: &str, start: f64, end: f64, fit: Fit, audio: bool) -> Part {
    Part {
        src: src.into(),
        start,
        end,
        fit,
        keep: None,
        audio,
    }
}

fn joined(parts: Vec<Part>) -> Job {
    Job {
        join: Some(Join {
            parts,
            frame: [1920, 1080],
            fps: Some(30.0),
        }),
        ..job(None, "out.mp4")
    }
}

#[test]
fn fit_modes_scale_into_the_frame() {
    let n = |f| normalize(f, [1920, 1080], None);
    assert!(n(Fit::Fit).starts_with("scale=1920:1080:force_original_aspect_ratio=decrease"));
    assert!(n(Fit::Fit).contains("pad=1920:1080:(ow-iw)/2:(oh-ih)/2"));
    assert!(
        n(Fit::Fill)
            .contains("force_original_aspect_ratio=increase:force_divisible_by=2,crop=1920:1080")
    );
    assert!(n(Fit::Stretch).starts_with("scale=1920:1080,setsar=1,format=yuv420p"));
    assert!(normalize(Fit::Stretch, [8, 8], Some(60.0)).contains(",fps=60.000,"));
}

#[test]
fn join_args_pad_silent_clips_and_crop_after_concat() {
    let mut j = joined(vec![
        part("a.mp4", 1.0, 3.0, Fit::Fit, true),
        part("b.mov", 0.0, 2.5, Fit::Fill, false),
    ]);
    j.crop = Some([1280, 720, 10, 20]);
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(
        a.contains("-ss 1.000 -to 3.000 -i a.mp4 -ss 0.000 -to 2.500 -i b.mov"),
        "{a}"
    );
    assert!(
        a.contains(
            "[0:v:0]setpts=PTS-STARTPTS,scale=1920:1080:force_original_aspect_ratio=decrease"
        ),
        "{a}"
    );
    assert!(
        a.contains(
            "[0:a:0]asetpts=PTS-STARTPTS,aresample=48000,aformat=channel_layouts=stereo[a0];"
        ),
        "{a}"
    );
    assert!(
        a.contains("anullsrc=r=48000:cl=stereo,atrim=duration=2.500[a1];"),
        "{a}"
    );
    assert!(
        a.contains("[v0][a0][v1][a1]concat=n=2:v=1:a=1[vj][aj];[vj]crop=1280:720:10:20[v]"),
        "{a}"
    );
    assert!(a.contains("-map [v] -map [aj] -c:v libx264"), "{a}");
    assert!(a.contains("-map_chapters -1"), "{a}");
}

#[test]
fn join_args_strip_and_mute() {
    let mut p = part("a.mp4", 0.0, 10.0, Fit::Fit, true);
    p.keep = Some(vec![(1.0, 2.0), (3.0, 4.0)]);
    let mut j = joined(vec![p, part("b.mp4", 0.0, 1.0, Fit::Fit, true)]);
    assert!((j.kept_duration() - 3.0).abs() < 1e-9);
    j.mute = true;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(a.contains("-ss 1.000 -to 4.000 -i a.mp4"), "{a}");
    assert!(
        a.contains("select='between(t,0.000,1.000)+between(t,2.000,3.000)'"),
        "{a}"
    );
    assert!(a.contains("concat=n=2:v=1:a=0[vj] -map [vj]"), "{a}");
    assert!(!a.contains("anullsrc") && a.ends_with("-an out.mp4"), "{a}");
}

#[test]
fn lossless_join_lists_the_clips() {
    let j = joined(vec![
        part("/x/it's.mp4", 1.5, 3.0, Fit::Fit, true),
        part("b.mp4", 0.0, 2.0, Fit::Fit, true),
    ]);
    let list = concat_list(j.join.as_ref().unwrap());
    assert!(
        list.starts_with(
            "ffconcat version 1.0\nfile '/x/it'\\''s.mp4'\ninpoint 1.500\noutpoint 3.000\n"
        ),
        "{list}"
    );
    let a = args(&j, None, &ANY).join(" ");
    assert!(a.contains("-f concat -safe 0 -i "), "{a}");
    assert!(a.contains("-map 0:v:0 -map 0:a? -c copy"), "{a}");
}

#[test]
fn streams_from_the_listing() {
    let l = "  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(tv, bt709, progressive), 1920x1080 [SAR 1:1 DAR 16:9], 3125 kb/s, 29.97 fps, 29.97 tbr, 30k tbn (default)\n  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 48000 Hz, stereo, fltp, 128 kb/s (default)";
    let s = parse_streams(l);
    assert_eq!(s.size, Some([1920, 1080]));
    assert_eq!(s.fps, Some(29.97));
    assert_eq!(
        s.video.as_deref(),
        Some(
            "h264 (High) (avc1 / 0x31637661)|yuv420p(tv, bt709, progressive)|1920x1080 [SAR 1:1 DAR 16:9]|29.97|30k|rot 0"
        )
    );
    assert_eq!(
        s.audio.as_deref(),
        Some("aac (LC) (mp4a / 0x6134706D)|48000 Hz|stereo|fltp")
    );
    // Other color, or one more audio track, can't be copied into the same file.
    let pc = parse_streams(&l.replace("tv, bt709", "pc, bt709"));
    assert_ne!(pc.video, s.video);
    let extra = format!("{l}\n  Stream #0:2: Audio: aac (LC), 48000 Hz, mono, fltp, 64 kb/s");
    assert_ne!(parse_streams(&extra).audio, s.audio);
}

#[test]
fn samples_spread_over_the_clips() {
    let j = joined(vec![
        part("a.mp4", 0.0, 20.0, Fit::Fit, true),
        part("b.mp4", 5.0, 25.0, Fit::Fit, true),
    ]);
    let s = sample_join(j.join.as_ref().unwrap(), 4, 2.5);
    assert_eq!(s.parts.len(), 2);
    for p in &s.parts {
        let k = p.keep.as_ref().unwrap();
        assert_eq!(k.len(), 2, "{k:?}");
        assert!(k.iter().all(|&(a, b)| a >= p.start && b <= p.end), "{k:?}");
    }
}
