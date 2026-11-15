//! Replacing a file keeps its DACL on Windows. Needs `ffmpeg` and `icacls`.

use super::fixture::{Fixture, wait};
use super::*;
use std::process::Command;

/// The file's ACEs as `icacls` lists them, without the path it prints first.
fn aces(p: &Path) -> String {
    let out = Command::new("icacls").arg(p).output().unwrap();
    assert!(
        out.status.success(),
        "icacls {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next().unwrap_or_default();
    let mut lines = vec![
        first
            .trim_start_matches(&*p.to_string_lossy())
            .trim()
            .to_owned(),
    ];
    lines.extend(
        text.lines()
            .skip(1)
            .map(str::trim)
            .take_while(|l| !l.is_empty())
            .map(str::to_owned),
    );
    lines.join("\n")
}

#[test]
fn replacing_a_file_keeps_its_protected_dacl() {
    let f = Fixture::new("dacl_dst");
    let job = f.job("dacl.mp4");
    std::fs::write(&job.dst, b"old").unwrap();
    let user = std::env::var("USERNAME").unwrap();
    // Drop inherited entries and leave only the current user, so a folder default would show.
    let ok = Command::new("icacls")
        .arg(&job.dst)
        .args(["/inheritance:r", "/grant:r", &format!("{user}:F")])
        .status()
        .unwrap();
    assert!(ok.success());
    let want = aces(&job.dst);
    assert!(!want.contains("(I)"), "{want}");
    wait(export(job.clone(), Context::default())).unwrap();
    assert_eq!(aces(&job.dst), want);
}
