use super::*;
use std::path::Path;

#[test]
fn entry_values() {
    let exe = Path::new(r"C:\Tools\Chyk.exe");
    let v = values(exe, "Edit with Chyk");
    let text: Vec<_> = v
        .iter()
        .map(|(sub, name, data)| (*sub, *name, data.to_str().unwrap()))
        .collect();
    assert_eq!(
        text,
        [
            ("", "", "Edit with Chyk"),
            ("", "Icon", r#""C:\Tools\Chyk.exe",0"#),
            ("", "MultiSelectModel", "Single"),
            ("command", "", r#""C:\Tools\Chyk.exe" "%1""#),
        ]
    );
}

#[test]
fn key_paths() {
    assert_eq!(
        key_path("mp4", ""),
        r"Software\Classes\SystemFileAssociations\.mp4\shell\Chyk"
    );
    assert_eq!(
        key_path("mkv", "command"),
        r"Software\Classes\SystemFileAssociations\.mkv\shell\Chyk\command"
    );
}

#[test]
fn every_video_type_gets_the_entry() {
    for e in ["mp4", "mkv", "mov", "webm", "avi"] {
        assert!(exts().contains(&e), "{e}");
    }
}

#[test]
fn command_matches_this_exe() {
    let exe = Path::new(r"C:\Tools\Chyk.exe");
    assert!(matches(OsStr::new(r#""C:\Tools\Chyk.exe" "%1""#), exe));
    assert!(matches(OsStr::new(r#""c:\tools\CHYK.EXE" "%1""#), exe));
    assert!(!matches(OsStr::new(r#""D:\Old\Chyk.exe" "%1""#), exe));
}
