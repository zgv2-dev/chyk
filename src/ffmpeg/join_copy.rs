//! Lossless join: the concat demuxer reads the clips from a list file and copies their streams.

use super::*;

/// `-tag:v hvc1` for HEVC outside MKV, so QuickTime/Apple players open it.
pub(super) fn tag_hevc(a: &mut Vec<String>, hevc: bool, dst: &Path) {
    let mkv = dst
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mkv"));
    if hevc && !mkv {
        a.extend(["-tag:v".into(), "hvc1".into()]);
    }
}

/// The concat demuxer's list for a lossless join.
pub(super) fn concat_list(j: &Join) -> String {
    let mut s = String::from("ffconcat version 1.0\n");
    for p in &j.parts {
        let path = p.src.to_string_lossy().replace('\'', "'\\''");
        s.push_str(&format!(
            "file '{path}'\ninpoint {:.3}\noutpoint {:.3}\n",
            p.start, p.end
        ));
    }
    s
}

/// Where a lossless join of `job` keeps its list while ffmpeg runs.
pub(super) fn list_path(job: &Job) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    job.dst.hash(&mut h);
    std::env::temp_dir().join(format!(
        "chyk-join-{}-{:x}.txt",
        std::process::id(),
        h.finish()
    ))
}

/// Lossless join through the concat demuxer: every clip has the same streams.
pub(super) fn concat_copy_args(job: &Job) -> Vec<String> {
    let mut a = head();
    a.extend(
        ["-f", "concat", "-safe", "0", "-i"]
            .map(String::from)
            .into_iter()
            .chain([list_path(job).to_string_lossy().into_owned()])
            .chain(
                [
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a?",
                    "-c",
                    "copy",
                    "-avoid_negative_ts",
                    "make_zero",
                ]
                .map(String::from),
            ),
    );
    if job.mute {
        a.push("-an".into());
    }
    tag_hevc(&mut a, job.codec == "hevc", &job.dst);
    a.push(job.dst.to_string_lossy().into_owned());
    a
}
