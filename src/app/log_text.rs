//! What a log entry says, as data and plain text: its result, error lines, time and kind.

use super::*;
use ffmpeg::log::{Entry, Kind, Run, RunState};
use std::time::SystemTime;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq)]
pub(super) enum Status {
    /// No command started yet.
    Waiting,
    Running,
    /// Every run exited 0: how many, and the average time each took.
    Done(usize, Duration),
    /// The first run that failed, with its exit code (`None`: killed, or it never started).
    Failed(Option<i32>),
    /// A probe's time. `ffmpeg -i` always exits 1, so its code says nothing.
    Probed(Duration),
}

pub(super) fn failed(kind: Kind, run: &Run) -> bool {
    match run.state {
        RunState::Running => false,
        RunState::Exited { code: None, .. } => true,
        RunState::Exited { code: Some(c), .. } => c != 0 && kind != Kind::Probe,
    }
}

pub(super) fn status(e: &Entry) -> Status {
    if e.runs.iter().any(|r| matches!(r.state, RunState::Running)) {
        return Status::Running;
    }
    if let Some(r) = e.runs.iter().find(|r| failed(e.kind, r)) {
        let RunState::Exited { code, .. } = r.state else {
            unreachable!("a failed run has exited")
        };
        return Status::Failed(code);
    }
    let took: Duration = e
        .runs
        .iter()
        .map(|r| match r.state {
            RunState::Exited { took, .. } => took,
            RunState::Running => Duration::ZERO,
        })
        .sum();
    match (e.kind, e.runs.len()) {
        (_, 0) => Status::Waiting,
        (Kind::Probe, _) => Status::Probed(took),
        (_, n) => Status::Done(n, took / n as u32),
    }
}

/// Red output: every line of a failed run, and any line that reports an error.
pub(super) fn is_error(line: &str, failed: bool) -> bool {
    failed
        || line
            .as_bytes()
            .windows(5)
            .any(|w| w.eq_ignore_ascii_case(b"error"))
}

/// `at` as local `HH:MM:SS`, from the C library: std has no time zones.
pub(super) fn wall_clock(at: SystemTime) -> String {
    // `struct tm` starts with seconds, minutes, hours; 16 ints hold all of it on every platform.
    #[repr(C, align(8))]
    struct Tm([i32; 16]);
    unsafe extern "C" {
        #[cfg(unix)]
        fn localtime_r(t: *const i64, tm: *mut Tm) -> *mut Tm;
        #[cfg(windows)]
        fn _localtime64_s(tm: *mut Tm, t: *const i64) -> i32;
    }
    let secs = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let mut tm = Tm([0; 16]);
    // SAFETY: both read one 64-bit time and write one `struct tm`, which fits in `tm`.
    #[cfg(unix)]
    let ok = unsafe { !localtime_r(&secs, &mut tm).is_null() };
    #[cfg(windows)]
    let ok = unsafe { _localtime64_s(&mut tm, &secs) == 0 };
    let [s, m, h, ..] = tm.0;
    if ok {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        String::new()
    }
}

pub(super) fn kind_label(kind: Option<Kind>) -> &'static str {
    match kind {
        None => t!("log.all"),
        Some(Kind::Export) => t!("log.export"),
        Some(Kind::Scan) => t!("log.scan"),
        Some(Kind::Estimate) => t!("log.estimate"),
        Some(Kind::Thumbnails) => t!("log.thumbnails"),
        Some(Kind::Frame) => t!("log.frame"),
        Some(Kind::Probe) => t!("log.probe"),
    }
}

fn secs(d: Duration) -> String {
    format!("{:.2} {}", d.as_secs_f64(), t!("unit.s"))
}

pub(super) fn status_text(s: &Status) -> (String, Color32) {
    match s {
        Status::Waiting => (String::new(), MUTED),
        Status::Running => (t!("log.running").into(), GREEN),
        Status::Done(1, d) | Status::Probed(d) => (secs(*d), MUTED),
        Status::Done(n, d) => (format!("{n} × {}", secs(*d)), MUTED),
        Status::Failed(Some(code)) => (t!("log.exit", code = code), RED),
        Status::Failed(None) => (t!("log.stopped").into(), RED),
    }
}

/// A running export's result: share done (from the app; the log doesn't know the length) and
/// ffmpeg's speed from its latest progress block, like "64% · 1.96×".
pub(super) fn progress_text(progress: Option<&str>, done: Option<f32>) -> Option<String> {
    let speed = progress
        .and_then(|p| p.split_once("speed="))
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .and_then(|s| s.strip_suffix('x'))
        .map(|s| format!("{s}×"));
    let done = done.map(|d| format!("{:.0}%", d * 100.0));
    let parts: Vec<String> = done.into_iter().chain(speed).collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A batch of near-identical commands shows the first and how many more ran.
pub(super) fn more(e: &Entry) -> String {
    match e.runs.len() {
        0 | 1 => String::new(),
        n => format!("  +{}", n - 1),
    }
}

/// The entry as plain text, for Copy.
pub(super) fn entry_text(e: &Entry) -> String {
    let kind = kind_label(Some(e.kind)).to_lowercase();
    let mut s = format!(
        "{} {kind} {}\n",
        wall_clock(e.at),
        status_text(&status(e)).0
    );
    if let Some(r) = e.runs.first() {
        s += &format!("$ {}{}\n", r.args, more(e));
    }
    for l in e.runs.iter().flat_map(|r| &r.output).chain(&e.progress) {
        s += &format!("  {l}\n");
    }
    s
}
