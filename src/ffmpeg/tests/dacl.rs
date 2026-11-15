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

fn icacls(p: &Path, args: &[&str]) {
    let ok = Command::new("icacls").arg(p).args(args).status().unwrap();
    assert!(ok.success(), "icacls {} {args:?}", p.display());
}

/// A file moved in from a stricter folder keeps its old entries; the export must not pick up
/// the broader ones its new folder hands to new files.
#[test]
fn replacing_a_moved_in_file_adds_no_folder_access() {
    let f = Fixture::new("dacl_moved");
    let job = f.job("moved.mp4");
    let strict = job.dst.with_file_name("strict");
    std::fs::create_dir(&strict).unwrap();
    let user = std::env::var("USERNAME").unwrap();
    icacls(
        &strict,
        &["/inheritance:r", "/grant:r", &format!("{user}:(OI)(CI)F")],
    );
    // Everyone may read new files here; set first, so it never reaches the moved-in file.
    icacls(job.dst.parent().unwrap(), &["/grant", "*S-1-1-0:(OI)(R)"]);
    let made = strict.join("moved.mp4");
    std::fs::write(&made, b"old").unwrap();
    std::fs::rename(&made, &job.dst).unwrap();
    // A protected copy clears the inherited flag; the entries themselves must match.
    let plain = |p: &Path| aces(p).replace("(I)", "");
    let want = plain(&job.dst);
    wait(export(job.clone(), Context::default())).unwrap();
    assert_eq!(plain(&job.dst), want);
}
