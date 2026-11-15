//! Before a lossless join: join a few seconds around each seam and decode the result, so clips
//! that won't copy cleanly (a failed join, or frames the decoder can't read, like a gray seam
//! from different encoder settings) are caught in seconds rather than after the whole export.

use super::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};

/// Seconds kept on each side of a seam.
const SIDE: f64 = 2.0;

/// `j` cut down to each clip's head and tail next to a seam. A short clip stays whole.
pub(super) fn seam_join(j: &Join) -> Join {
    let n = j.parts.len();
    let mut parts = Vec::new();
    for (i, p) in j.parts.iter().enumerate() {
        let a = if i > 0 {
            p.start
        } else {
            (p.end - SIDE).max(p.start)
        };
        let b = if i + 1 < n {
            p.end
        } else {
            (p.start + SIDE).min(p.end)
        };
        let cut = |start: f64, end: f64| Part {
            start,
            end,
            ..p.clone()
        };
        if i == 0 || i + 1 == n || b - a <= 2.0 * SIDE {
            parts.push(cut(a, b));
        } else {
            parts.push(cut(a, a + SIDE));
            parts.push(cut(b - SIDE, b));
        }
    }
    Join { parts, ..j.clone() }
}

/// Results by [`seam_key`], and the keys being tried now.
type Seams = (HashMap<String, bool>, HashSet<String>);
fn seams() -> MutexGuard<'static, Seams> {
    static S: OnceLock<Mutex<Seams>> = OnceLock::new();
    let s = S.get_or_init(Default::default);
    s.lock().unwrap_or_else(|e| e.into_inner())
}

/// What a seam trial of `job` depends on: the seams' clips and times, container, audio and codec.
/// `None` for anything but a join of several clips.
pub fn seam_key(job: &Job) -> Option<String> {
    let j = job.join.as_ref().filter(|j| j.parts.len() > 1)?;
    let ext = job.dst.extension().unwrap_or_default().to_string_lossy();
    Some(format!(
        "{}{ext} {} {}",
        concat_list(&seam_join(j)),
        job.mute,
        job.codec
    ))
}

/// A finished trial's result for `key`: whether its seams join and decode cleanly.
pub fn seams_ok(key: &str) -> Option<bool> {
    seams().0.get(key).copied()
}

/// Tries `job`'s seams on a thread, unless `key` is known or being tried; wakes `ctx` when done.
pub fn try_seams(job: Job, key: String, ctx: Context) {
    {
        let mut s = seams();
        if s.0.contains_key(&key) || !s.1.insert(key.clone()) {
            return;
        }
    }
    std::thread::spawn(move || {
        let ok = trial(&job, log::begin(log::Kind::Probe)).is_ok();
        let mut s = seams();
        s.0.insert(key.clone(), ok);
        s.1.remove(&key);
        drop(s);
        crate::wake(&ctx);
    });
}

/// [`trial`] for an export, reusing an earlier result for the same seams.
pub(super) fn check_seams(job: &Job, entry: u64) -> bool {
    let key = seam_key(job);
    if let Some(ok) = key.as_deref().and_then(seams_ok) {
        return ok;
    }
    let ok = trial(job, entry).is_ok();
    if let Some(k) = key {
        seams().0.insert(k, ok);
    }
    ok
}

/// Joins `job`'s seams losslessly into a temp file and decodes it. `Err` holds ffmpeg's errors.
fn trial(job: &Job, entry: u64) -> Result<(), String> {
    let Some(j) = job.join.as_ref().filter(|j| j.parts.len() > 1) else {
        return Ok(());
    };
    // Named after the export's own list, so exports running at once don't share a trial.
    let name = list_path(job)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .replace("chyk-join", "chyk-seams");
    let ext = job.dst.extension().unwrap_or_default();
    let trial = Job {
        dst: std::env::temp_dir().join(name).with_extension(ext),
        join: Some(seam_join(j)),
        ..job.clone()
    };
    // A fresh list, never through a link left at that name (as for the export's own list).
    let list = trial.dst.with_extension("txt");
    let _ = std::fs::remove_file(&list);
    let mut f = std::fs::File::create_new(&list).map_err(|e| e.to_string())?;
    let _list = TempFile(list.clone());
    std::io::Write::write_all(&mut f, concat_list(trial.join.as_ref().unwrap()).as_bytes())
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&trial.dst);
    let _out = TempFile(trial.dst.clone());
    let mut a = concat_copy_args(&trial);
    // Read from this trial's list, not the export's.
    if let Some(i) = a.iter().position(|s| s == "-i") {
        a[i + 1] = list.to_string_lossy().into_owned();
    }
    let ran = |args: Vec<String>| {
        let out = log::output(entry, command("ffmpeg").args(args)).map_err(|e| e.to_string())?;
        // The null muxer's timestamp complaints about the cut aren't decode errors.
        let err: Vec<&str> = std::str::from_utf8(&out.stderr)
            .unwrap_or("?")
            .lines()
            .filter(|l| !l.starts_with("[null @") && !l.trim_start().starts_with("Last message"))
            .filter(|l| !l.trim().is_empty())
            .collect();
        let err = err.join("\n");
        match out.status.success() && err.is_empty() {
            true => Ok(()),
            false => Err(error_tail(&err)),
        }
    };
    ran(a)?;
    let decode = ["-hide_banner", "-nostdin", "-v", "error", "-i"]
        .map(String::from)
        .into_iter()
        .chain([trial.dst.to_string_lossy().into_owned()])
        .chain(["-f", "null", "-"].map(String::from));
    ran(decode.collect())
}
