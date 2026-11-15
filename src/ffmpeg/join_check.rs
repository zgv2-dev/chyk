//! Before a lossless join: join a few seconds around each seam and decode the result, so clips
//! that won't copy cleanly (a failed join, or frames the decoder can't read, like a gray seam
//! from different encoder settings) are caught in seconds rather than after the whole export.

use super::*;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

mod runs;
use runs::{decode, errors};

/// Seconds kept on each side of a seam.
const SIDE: f64 = 2.0;
/// How long a trial that couldn't run (no disk space, say) waits before it is tried again.
const RETRY: Duration = Duration::from_secs(30);

/// `j`'s clips cut down to their heads and tails next to a seam, in order, each with whether it
/// starts inside its clip (a tail; the export reaches it from the frames before). A short clip
/// stays whole.
fn seam_pieces(j: &Join) -> Vec<(Part, bool)> {
    let n = j.parts.len();
    let mut out = Vec::new();
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
        let cut = |start: f64, end: f64| {
            let part = Part {
                start,
                end,
                ..p.clone()
            };
            (part, start > p.start)
        };
        if i == 0 || i + 1 == n || b - a <= 2.0 * SIDE {
            out.push(cut(a, b));
        } else {
            out.push(cut(a, a + SIDE));
            out.push(cut(b - SIDE, b));
        }
    }
    out
}

/// `j` cut down to its seams, as [`seam_pieces`].
pub(super) fn seam_join(j: &Join) -> Join {
    let parts = seam_pieces(j).into_iter().map(|(p, _)| p).collect();
    Join { parts, ..j.clone() }
}

/// Results by [`seam_key`], the keys being tried now, and when trials that couldn't run ended.
type Seams = (
    HashMap<String, bool>,
    HashSet<String>,
    HashMap<String, Instant>,
);
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
    // Which pieces are tails changes how the trial counts errors, not the list.
    let tails: String = seam_pieces(j)
        .iter()
        .map(|(_, t)| ['h', 't'][*t as usize])
        .collect();
    Some(format!(
        "{}{ext} {} {} {tails}",
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
        let failed = s.2.get(&key).is_some_and(|t| t.elapsed() < RETRY);
        if failed || s.0.contains_key(&key) || !s.1.insert(key.clone()) {
            return;
        }
    }
    std::thread::spawn(move || {
        let ok = trial(&job, log::begin(log::Kind::Probe), &AtomicBool::new(false));
        let mut s = seams();
        if let Some(ok) = ok {
            s.0.insert(key.clone(), ok);
        } else {
            s.2.insert(key.clone(), Instant::now());
        }
        s.1.remove(&key);
        drop(s);
        crate::wake(&ctx);
    });
}

/// [`trial`] for an export, reusing an earlier result. One that can't run lets the copy try.
pub(super) fn check_seams(job: &Job, entry: u64, stop: &AtomicBool) -> bool {
    let key = seam_key(job);
    if let Some(ok) = key.as_deref().and_then(seams_ok) {
        return ok;
    }
    let Some(ok) = trial(job, entry, stop) else {
        return true;
    };
    if let Some(k) = key {
        seams().0.insert(k, ok);
    }
    ok
}

/// Joins `job`'s seams losslessly into a temp file and decodes it: whether both went cleanly.
/// `None` when the trial couldn't run or `stop` was set, which says nothing about the clips.
fn trial(job: &Job, entry: u64, stop: &AtomicBool) -> Option<bool> {
    let Some(j) = job.join.as_ref().filter(|j| j.parts.len() > 1) else {
        return Some(true);
    };
    // Numbered, so trials running at once (the background's and exports') never share files.
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let name = list_path(job)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .replace("chyk-join", &format!("chyk-seams-{n}"));
    let ext = job.dst.extension().unwrap_or_default();
    let trial = Job {
        dst: std::env::temp_dir().join(name).with_extension(ext),
        join: Some(seam_join(j)),
        ..job.clone()
    };
    // A fresh list, never through a link left at that name (as for the export's own list).
    let list = trial.dst.with_extension("txt");
    let _ = std::fs::remove_file(&list);
    let mut f = std::fs::File::create_new(&list).ok()?;
    let _list = TempFile(list.clone());
    std::io::Write::write_all(&mut f, concat_list(trial.join.as_ref().unwrap()).as_bytes()).ok()?;
    let _ = std::fs::remove_file(&trial.dst);
    let _out = TempFile(trial.dst.clone());
    let mut a = concat_copy_args(&trial);
    // Read from this trial's list, not the export's.
    if let Some(i) = a.iter().position(|s| s == "-i") {
        a[i + 1] = list.to_string_lossy().into_owned();
    }
    if errors(entry, a, stop)? > 0 {
        return Some(false);
    }
    let decoded = errors(entry, decode(&trial.dst, None), stop)?;
    if decoded == 0 {
        return Some(true);
    }
    // A tail can start on a frame that needs earlier ones (an open GOP), which errors without any
    // join: only errors beyond those of the tails decoded on their own count.
    let mut alone = 0;
    for (p, _) in seam_pieces(j).into_iter().filter(|(_, tail)| *tail) {
        alone += errors(
            entry,
            decode(&p.src, Some((p.start, p.end - p.start))),
            stop,
        )?;
    }
    Some(decoded <= alone)
}
