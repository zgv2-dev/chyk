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

/// Stands for the list file in [`concat_copy_args`] until [`write_list`] fills it in.
const LIST: &str = "\u{1}concat-list\u{1}";

/// Writes `j`'s list to a temp file for a job on `src`, and points `a` at it.
pub(super) fn write_list(a: &mut [String], src: &Path, j: &Join) -> std::io::Result<TempFile> {
    let (file, mut f) = scratch::create(src, "join", "txt")?;
    std::io::Write::write_all(&mut f, concat_list(j).as_bytes())?;
    for s in a.iter_mut().filter(|s| *s == LIST) {
        *s = file.0.to_string_lossy().into_owned();
    }
    Ok(file)
}

/// Lossless join through the concat demuxer: every clip has the same streams.
pub(super) fn concat_copy_args(job: &Job) -> Vec<String> {
    let mut a = head();
    a.extend(
        ["-f", "concat", "-safe", "0", "-i"]
            .map(String::from)
            .into_iter()
            .chain([LIST.to_string()])
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
