//! The ffmpeg log: bounded, one entry per job.

use super::fixture::{Fixture, wait};
use super::*;
// `log` here names this test module, so the real one goes by its full path.
use crate::ffmpeg::log::{Kind, Log, RunState, describe, entries};
use std::time::Duration;

#[test]
fn keeps_200_entries() {
    let mut log = Log::new();
    let first = log.begin(Kind::Scan);
    for _ in 0..204 {
        log.begin(Kind::Scan);
    }
    let all = log.entries();
    assert_eq!(all.len(), 200);
    assert_eq!(all[0].id, first + 5, "oldest dropped first");
    assert_eq!(all[199].id, first + 204, "newest last");
}

#[test]
fn keeps_200_lines_per_run() {
    let mut log = Log::new();
    let id = log.begin(Kind::Export);
    let run = log.start(id, "ffmpeg".into());
    let before = log.generation();
    for i in 0..205 {
        log.line(id, run, &format!("line {i}"));
    }
    assert!(log.generation() > before, "a new line is a change");
    let out = &log.entries()[0].runs[run].output;
    assert_eq!(out.len(), 200);
    assert_eq!(out.front().map(String::as_str), Some("line 5"));
    assert_eq!(out.back().map(String::as_str), Some("line 204"));
}

#[test]
fn thumbnail_refreshes_share_one_entry() {
    let mut log = Log::new();
    let a = log.begin(Kind::Thumbnails);
    let ra = log.start(a, "ffmpeg".into());
    assert!(log.busy());
    // A refresh while the last one still runs gets its own entry.
    let b = log.begin(Kind::Thumbnails);
    let rb = log.start(b, "ffmpeg".into());
    assert_eq!(log.entries().len(), 2);
    log.exited(a, ra, Some(0), Duration::from_millis(5));
    log.exited(b, rb, Some(0), Duration::from_millis(5));
    assert!(!log.busy());
    // Once it is done, the next refresh takes its place.
    let c = log.begin(Kind::Thumbnails);
    let ids: Vec<u64> = log.entries().iter().map(|e| e.id).collect();
    assert_eq!(ids, [a, c]);
    assert_ne!(b, c);
    // Any other job in between keeps both.
    log.begin(Kind::Probe);
    log.begin(Kind::Thumbnails);
    assert_eq!(log.entries().len(), 4);
}

#[test]
fn thumbnails_entry_without_runs_is_not_replaced() {
    let mut log = Log::new();
    let a = log.begin(Kind::Thumbnails);
    // Its first tile is still on the way: a refresh now must not drop it.
    let b = log.begin(Kind::Thumbnails);
    let ids: Vec<u64> = log.entries().iter().map(|e| e.id).collect();
    assert_eq!(ids, [a, b]);
}

#[test]
fn long_lines_are_cut() {
    let mut log = Log::new();
    let id = log.begin(Kind::Export);
    let run = log.start(id, "ffmpeg".into());
    log.line(id, run, &"é".repeat(1500));
    log.line(id, run, &"a".repeat(1000));
    let out = &log.entries()[0].runs[run].output;
    assert_eq!(out[0].chars().count(), 1001);
    assert!(out[0].ends_with("é…"), "cut on a char boundary, marked");
    assert_eq!(out[1].len(), 1000, "a line at the limit stays whole");
}

#[test]
fn unknown_entry_is_no_change() {
    let mut log = Log::new();
    let before = log.generation();
    assert_eq!(log.start(99, "ffmpeg".into()), 0);
    log.line(99, 0, "lost");
    log.progress(99, "lost");
    log.exited(99, 0, Some(0), Duration::ZERO);
    assert_eq!(log.generation(), before);
}

#[test]
fn quotes_args_with_spaces() {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-i", "/a b/c.mov"]);
    assert_eq!(describe(&cmd), r#"ffmpeg -i "/a b/c.mov""#);
}

#[test]
fn export_run_is_logged() {
    let f = Fixture::new("log");
    let job = f.job("logged.mp4");
    let dst = job.dst.to_string_lossy().into_owned();
    wait(export(job, Context::default())).unwrap();
    let entry = entries()
        .into_iter()
        .find(|e| e.runs.iter().any(|r| r.args.contains(&dst)))
        .expect("the export's entry");
    assert_eq!(entry.kind, Kind::Export);
    assert_eq!(entry.runs.len(), 1);
    assert!(matches!(
        entry.runs[0].state,
        RunState::Exited { code: Some(0), .. }
    ));
    assert!(entry.progress.is_some_and(|p| p.ends_with("progress=end")));
}
