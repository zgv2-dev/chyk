use super::*;

#[test]
fn source_from_ffmpeg_listing() {
    let s = parse_source(
        "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':\n  Duration: 00:12:02.27, start: 0.000000, bitrate: 776 kb/s\n  Stream #0:0[0x1](und): Video: hevc (Main) (hvc1 / 0x31637668), yuv420p(tv, bt709, progressive), 1920x1080 [SAR 1:1 DAR 16:9], 770 kb/s, 60 fps, 60 tbr, 15360 tbn (default)\n  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 48000 Hz, stereo, fltp, 64 kb/s (default)\n",
    );
    assert!(s.audio);
    assert_eq!((s.video_kbps, s.audio_kbps), (Some(770), Some(64)));
    // MKV streams often carry no bitrate: fall back to the file's.
    let s = parse_source(
        "  Duration: 00:00:10.00, start: 0.000000, bitrate: 2500 kb/s\n  Stream #0:0: Video: h264 (High), yuv420p, 1280x720, 30 fps\n",
    );
    assert!(!s.audio);
    assert_eq!((s.video_kbps, s.audio_kbps), (Some(2500), None));
}

#[test]
fn candidates_prefer_same_codec_then_h264() {
    assert_eq!(
        candidates("hevc", LIST),
        ["hevc_videotoolbox", "h264_videotoolbox", "libx264"]
    );
    assert_eq!(candidates("vp9", LIST), ["h264_videotoolbox", "libx264"]);
    assert!(candidates("h264", "").is_empty());
}

#[test]
fn compress_prefers_software_encoders() {
    assert_eq!(
        encoder_order("h264", LIST, false),
        ["h264_videotoolbox", "libx264"]
    );
    assert_eq!(
        encoder_order("h264", LIST, true),
        ["libx264", "h264_videotoolbox"]
    );
    assert_eq!(
        encoder_order("hevc", LIST, true),
        ["libx264", "hevc_videotoolbox", "h264_videotoolbox"]
    );
}

#[test]
fn long_sums_nest() {
    let t = |n: usize| (0..n).map(|i| format!("x{i}")).collect::<Vec<_>>();
    assert_eq!(sum(t(1)), "x0");
    assert_eq!(sum(t(2)), "x0+x1");
    assert_eq!(sum(t(5)), "((x0+x1)+(x2+x3))+x4");
    assert_eq!(
        close_gaps(&[(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)]),
        "(T-1.000-(gte(T,3.000)*1.000+gte(T,5.000)*1.000))/TB"
    );
}

/// Needs `ffmpeg` on PATH. Thousands of intervals must still parse: ffmpeg rejected the flat
/// chains past 100 terms.
#[test]
fn ffmpeg_parses_long_expressions() {
    let keep: Vec<(f64, f64)> = (0..3000)
        .map(|i| (i as f64 * 0.003, i as f64 * 0.003 + 0.001))
        .collect();
    let mut a: Vec<String> = ["-v", "error", "-f", "lavfi", "-i", "nullsrc=s=16x16:d=1"]
        .map(String::from)
        .into();
    a.push("-filter_complex".into());
    a.push(format!(
        "[0:v]select='{}',setpts='{}'[v]",
        select_expr(&keep),
        close_gaps(&keep)
    ));
    a.extend(["-map", "[v]", "-f", "null", "-"].map(String::from));
    let file = spill_graph(&mut a).expect("graph long enough to spill");
    assert!(file.0.exists() && a.contains(&graph_file_option().to_string()));
    let out = Command::new("ffmpeg").args(&a).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn major_version_parses_release_and_git_builds() {
    assert_eq!(
        major_version("ffmpeg version 6.1.1-3ubuntu5 Copyright"),
        Some(6)
    );
    assert_eq!(major_version("ffmpeg version n7.1 Copyright"), Some(7));
    assert_eq!(major_version("ffmpeg version 9.0.2 Copyright"), Some(9));
    assert_eq!(
        major_version("ffmpeg version N-118000-gabc Copyright"),
        None
    );
    assert_eq!(major_version(""), None);
}

#[test]
fn error_tail_cuts_long_lines() {
    let long = format!(
        "[select] Error while parsing expression '{}'",
        "x+".repeat(500)
    );
    let e = error_tail(&format!("{long}\nError initializing filters\n\n"));
    assert!(e.lines().all(|l| l.chars().count() <= 161), "{e}");
    assert!(e.ends_with("Error initializing filters"), "{e}");
}

#[test]
fn many_segments_fit_windows_command_line() {
    let mut j = job(None, "out.mp4");
    j.src = "s".repeat(260).into();
    j.dst = "x".repeat(260).into();
    j.start = 0.0;
    j.end = 99_999.0;
    // Long timestamps and short gaps: the widest expressions a strip export can produce.
    j.keep = Some(
        (0..3000)
            .map(|i| {
                let a = 10_000.0 + i as f64 * 3.123;
                (a, a + 1.567)
            })
            .collect(),
    );
    j.speed = 2.0;
    let mut a = args(&j, Some("h264_videotoolbox"), &ANY);
    let _graph = spill_graph(&mut a);
    let len: usize = a.iter().map(|a| a.len() + 3).sum();
    assert!(len < 32_000, "command line is {len} characters");
}

#[test]
fn atempo_chain_stays_in_range() {
    assert_eq!(atempo(1.5), "atempo=1.500000");
    assert_eq!(atempo(2.0), "atempo=2.000000");
    assert_eq!(atempo(4.0), "atempo=2.000000,atempo=2.000000");
    assert_eq!(atempo(0.25), "atempo=0.500000,atempo=0.500000");
}

#[test]
fn first_encoder_prefers_hardware() {
    assert_eq!(
        first_encoder_in("hevc", LIST, false),
        Some("hevc_videotoolbox")
    );
    assert_eq!(
        first_encoder_in("h264", LIST, false),
        Some("h264_videotoolbox")
    );
    assert_eq!(first_encoder_in("h264", LIST, true), Some("libx264"));
    assert_eq!(first_encoder_in("h264", "", false), None);
}
