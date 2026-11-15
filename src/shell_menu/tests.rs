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
        key_path(""),
        r"Software\Classes\SystemFileAssociations\video\shell\Chyk"
    );
    assert_eq!(
        key_path("command"),
        r"Software\Classes\SystemFileAssociations\video\shell\Chyk\command"
    );
}

#[test]
fn command_matches_this_exe() {
    let exe = Path::new(r"C:\Tools\Chyk.exe");
    assert!(matches(OsStr::new(r#""C:\Tools\Chyk.exe" "%1""#), exe));
    assert!(matches(OsStr::new(r#""c:\tools\CHYK.EXE" "%1""#), exe));
    assert!(!matches(OsStr::new(r#""D:\Old\Chyk.exe" "%1""#), exe));
}
