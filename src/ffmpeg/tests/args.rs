use super::*;

#[test]
fn reencodes_are_capped_at_the_source_bitrate() {
    let mut j = job(Some([640, 360, 2, 4]), "out.mp4");
    let src = Source {
        audio: true,
        video_kbps: Some(770),
        audio_kbps: Some(64),
        strict: false,
    };
    let a = args(&j, Some("hevc_videotoolbox"), &src).join(" ");
    assert!(a.contains("-q:v 65 -maxrate 770k -bufsize 1540k"), "{a}");
    j.keep = Some(vec![(1.5, 2.0)]);
    let a = args(&j, Some("libx264"), &src).join(" ");
    assert!(
        a.contains("-crf 20 -preset medium -maxrate 770k -bufsize 1540k"),
        "{a}"
    );
    // Re-encoded audio never above the source's.
    assert!(a.contains("-c:a aac -b:a 64k"), "{a}");
    let strict = Source {
        strict: true,
        ..src
    };
    let a = args(&j, Some("h264_amf"), &strict).join(" ");
    assert!(
        a.contains("-rc vbr_peak -b:v 770k -maxrate 770k -bufsize 1540k"),
        "{a}"
    );
}

#[test]
fn copy_args() {
    let a = args(&job(None, "out.mkv"), None, &ANY).join(" ");
    assert!(
        a.contains("-ss 1.500 -to 4.000 -i in.mkv -map 0 -dn -c copy"),
        "{a}"
    );
    assert!(a.ends_with(" out.mkv"));
    let mut j = job(None, "out.mp4");
    j.mute = true;
    let a = args(&j, None, &ANY).join(" ");
    assert!(
        a.ends_with("-an -map -0:t -c:s mov_text -tag:v hvc1 out.mp4"),
        "{a}"
    );
}

#[test]
fn crop_args() {
    let a = args(
        &job(Some([640, 360, 2, 4]), "out.mp4"),
        Some("hevc_videotoolbox"),
        &ANY,
    )
    .join(" ");
    assert!(
        a.contains("-c:v hevc_videotoolbox -q:v 65 -vf crop=640:360:2:4 -map -0:t"),
        "{a}"
    );
    assert!(a.ends_with("-tag:v hvc1 out.mp4"), "{a}");
    let mut j = job(None, "out.mkv");
    j.quality = 2;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(
        a.contains("-c:v libx264 -crf 28 -preset medium out.mkv"),
        "{a}"
    );
    let a = args(
        &job(Some([640, 360, 2, 4]), "out.mkv"),
        Some("hevc_videotoolbox"),
        &ANY,
    )
    .join(" ");
    assert!(!a.contains("hvc1"));
}

#[test]
fn compress_args() {
    let mut j = job(None, "out.mp4");
    j.quality = COMPRESS;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(
        a.contains("-c:v libx264 -crf 32 -preset slow -c:a aac -b:a 96k"),
        "{a}"
    );
    let a = args(&j, Some("libx265"), &ANY).join(" ");
    assert!(a.contains("-c:v libx265 -crf 34 -preset slow"), "{a}");
    let a = args(&j, Some("h264_videotoolbox"), &ANY).join(" ");
    assert!(a.contains("-q:v 28"), "{a}");

    j.mute = true;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(!a.contains("-c:a aac"), "{a}");
    assert!(a.ends_with("-an -map -0:t -c:s mov_text out.mp4"), "{a}");
}

#[test]
fn kept_duration_sums_intervals() {
    let mut j = job(None, "out.mp4");
    assert_eq!(j.kept_duration(), 2.5);
    j.keep = Some(vec![(1.5, 2.0), (3.0, 4.0)]);
    assert_eq!(j.kept_duration(), 1.5);
}

#[test]
fn speed_args_retime_video_and_audio() {
    let mut j = job(None, "out.mp4");
    j.speed = 2.0;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(a.contains("-vf setpts=PTS/2.000000"), "{a}");
    assert!(a.contains("-af atempo=2.000000"), "{a}");
    assert!(a.contains("-c:a aac -b:a 128k"), "{a}");
    assert!(!a.contains("-map 0:s?"), "{a}");
    assert!(a.contains("-map_chapters -1"), "{a}");

    j.speed = 0.25;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(a.contains("-af atempo=0.500000,atempo=0.500000"), "{a}");

    j.speed = 2.0;
    j.mute = true;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(!a.contains("-af"), "{a}");
    assert!(a.ends_with("-an out.mp4"), "{a}");
}

#[test]
fn speed_changes_output_duration() {
    let mut j = job(None, "out.mp4");
    assert_eq!(j.output_duration(), 2.5);
    j.speed = 2.0;
    assert_eq!(j.output_duration(), 1.25);
    j.speed = 0.5;
    assert_eq!(j.output_duration(), 5.0);
}
