use super::*;
use std::collections::VecDeque;

fn run(code: Option<Option<i32>>, ms: u64, out: &[&str]) -> Run {
    Run {
        args: "ffmpeg -i a.mp4".into(),
        state: code.map_or(RunState::Running, |code| RunState::Exited {
            code,
            took: Duration::from_millis(ms),
        }),
        output: out.iter().map(|s| s.to_string()).collect::<VecDeque<_>>(),
    }
}

fn entry(kind: Kind, runs: Vec<Run>) -> Entry {
    Entry {
        id: 0,
        at: SystemTime::now(),
        kind,
        runs,
        progress: None,
    }
}

#[test]
fn status_follows_the_runs() {
    let e = |kind, runs| status(&entry(kind, runs));
    assert_eq!(e(Kind::Export, vec![]), Status::Waiting);
    let running = vec![run(Some(Some(0)), 10, &[]), run(None, 0, &[])];
    assert_eq!(e(Kind::Thumbnails, running), Status::Running);
    let done = vec![run(Some(Some(0)), 30, &[]), run(Some(Some(0)), 50, &[])];
    assert_eq!(
        e(Kind::Thumbnails, done),
        Status::Done(2, Duration::from_millis(40))
    );
    let fell_back = vec![run(Some(Some(1)), 5, &[]), run(Some(Some(0)), 9, &[])];
    assert_eq!(e(Kind::Estimate, fell_back), Status::Failed(Some(1)));
    assert_eq!(
        e(Kind::Export, vec![run(Some(None), 0, &[])]),
        Status::Failed(None),
        "killed or never started"
    );
}

#[test]
fn a_probe_exiting_1_is_not_a_failure() {
    let e = entry(Kind::Probe, vec![run(Some(Some(1)), 20, &["Stream #0:0"])]);
    assert_eq!(status(&e), Status::Probed(Duration::from_millis(20)));
    assert!(!is_error("Stream #0:0", failed(e.kind, &e.runs[0])));
    let e = entry(Kind::Probe, vec![run(Some(None), 0, &[])]);
    assert_eq!(status(&e), Status::Failed(None), "it couldn't run at all");
}

#[test]
fn error_lines() {
    assert!(is_error("[h264 @ 0x1] Error while decoding", false));
    assert!(
        is_error("Falling back to libx265", true),
        "a failed run's lines"
    );
    assert!(!is_error("Stream #0:0: Video: h264", false));
}

#[test]
fn wall_clock_is_hh_mm_ss() {
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_027);
    let s = wall_clock(at);
    assert_eq!(s.len(), 8, "{s}");
    let parts: Vec<u32> = s.split(':').map(|p| p.parse().unwrap()).collect();
    assert!(parts[0] < 24 && parts[1] < 60);
    // Time zones move hours and minutes, never seconds.
    assert_eq!(parts[2], 47);
}

#[test]
fn copied_text_shows_the_batch_and_output() {
    let mut e = entry(
        Kind::Thumbnails,
        vec![
            run(Some(Some(0)), 40, &["warn"]),
            run(Some(Some(0)), 40, &[]),
        ],
    );
    e.progress = Some("progress=end".into());
    let text = entry_text(&e);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].ends_with("2 × 0.04 s"), "{text}");
    assert_eq!(lines[1], "$ ffmpeg -i a.mp4  +1");
    assert_eq!(&lines[2..], ["  warn", "  progress=end"]);
}
