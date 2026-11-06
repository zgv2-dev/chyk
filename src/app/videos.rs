//! Which files Chyk opens: videos, told apart by extension (the picker and drag and drop).

/// File extensions Chyk opens: common video containers mpv plays. Matched case-insensitively.
pub(super) const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "mov", "qt", "mkv", "webm", "avi", "wmv", "asf", "flv", "f4v", "mpg", "mpeg",
    "mpe", "m2v", "ts", "mts", "m2ts", "vob", "ogv", "3gp", "3g2", "mxf", "dv", "divx", "y4m",
    "rm", "rmvb", "hevc", "h264", "h265",
];

/// Whether `path` looks like a video, by its extension.
pub(super) fn is_video(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTS.iter().any(|v| e.eq_ignore_ascii_case(v)))
}

#[cfg(test)]
#[test]
fn videos_by_extension() {
    use std::path::Path;
    for p in ["a.mp4", "B.MOV", "c.d.mkv", "/x/y.webm", "clip.M2TS"] {
        assert!(is_video(Path::new(p)), "{p}");
    }
    for p in [
        "notes.txt",
        "photo.jpg",
        "song.mp3",
        "archive.mp4.zip",
        "noext",
        ".mp4",
    ] {
        assert!(!is_video(Path::new(p)), "{p}");
    }
}
