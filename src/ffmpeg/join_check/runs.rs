//! The ffmpeg runs of a seam trial.

use super::*;

/// ffmpeg arguments decoding `src` (from `at` for `len` seconds) into nothing, every error shown.
pub(super) fn decode(src: &Path, span: Option<(f64, f64)>) -> Vec<String> {
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
    a.extend(["-map", "0:v:0", "-map", "0:a?", "-f", "null", "-"].map(String::from));
    a
}

/// Runs ffmpeg as part of `entry` and counts its error lines, plus one if it failed. `None` if
/// it couldn't start, or `stop` was set first.
pub(super) fn errors(entry: u64, args: Vec<String>, stop: &AtomicBool) -> Option<usize> {
    if stop.load(Ordering::Relaxed) {
        return None;
    }
    let out = log::output(entry, command("ffmpeg").args(args)).ok()?;
    // The null muxer's timestamp complaints about the cuts aren't decode errors.
    let lines = String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("[null @"))
        .count();
    Some(lines + usize::from(!out.status.success()))
}
