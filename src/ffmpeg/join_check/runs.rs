//! The ffmpeg runs of a seam trial.

use super::*;

/// ffmpeg arguments decoding `src` (from `at` for `len` seconds, with its audio or not) into
/// nothing, every error shown.
pub(super) fn decode(src: &Path, span: Option<(f64, f64)>, audio: bool) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-nostdin", "-v", "repeat+error"]
        .map(String::from)
        .into();
    if let Some((at, _)) = span {
        a.extend(["-ss".into(), format!("{at:.3}")]);
    }
    a.extend(["-i".into(), src.to_string_lossy().into_owned()]);
    if let Some((_, len)) = span {
        a.extend(["-t".into(), format!("{len:.3}")]);
    }
    a.extend(["-map", "0:v:0"].map(String::from));
    if audio {
        a.extend(["-map".into(), "0:a?".into()]);
    }
    a.extend(["-f", "null", "-"].map(String::from));
    a
}

/// Runs ffmpeg as part of `entry` and counts its error lines, plus one if it failed. `None` if
/// it couldn't start, ran out of disk space, or `stop` was set first.
pub(super) fn errors(entry: u64, args: Vec<String>, stop: &AtomicBool) -> Option<usize> {
    if stop.load(Ordering::Relaxed) {
        return None;
    }
    let out = log::output(entry, command("ffmpeg").args(args)).ok()?;
    let err = String::from_utf8_lossy(&out.stderr);
    // A full disk says nothing about the clips (ffmpeg prints strerror, in English).
    if err.contains("No space left on device") {
        return None;
    }
    // The null muxer's timestamp complaints about the cuts aren't decode errors.
    let lines = err
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("[null @"))
        .count();
    Some(lines + usize::from(!out.status.success()))
}
