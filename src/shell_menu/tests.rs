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

#[test]
fn manifest_has_a_verb_per_extension_and_the_dll() {
    let m = package::manifest("1.3.2", &["mp4", "mkv"]);
    assert!(m.contains(r#"Version="1.3.2.0""#));
    assert!(m.contains(r#"<desktop5:ItemType Type=".mp4">"#));
    assert!(m.contains(r#"<desktop5:ItemType Type=".mkv">"#));
    assert_eq!(
        m.matches(package::CLSID).count(),
        3,
        "two verbs and the COM class"
    );
    assert!(m.contains(r#"Path="chyk_shell.dll""#));
    assert!(m.contains(r#"Executable="chyk-shell.exe""#));
}

#[test]
fn register_script_quotes_paths() {
    let s = package::script(Some(Path::new(r"C:\Users\O'Brien\Chyk")));
    assert!(s.contains(r"-Register 'C:\Users\O''Brien\Chyk\AppxManifest.xml'"));
    assert!(s.contains("-ExternalLocation 'C:\\Users\\O''Brien\\Chyk'"));
    assert!(s.starts_with("$ErrorActionPreference = 'Stop'; Get-AppxPackage -Name Chyk.ShellMenu"));
    assert_eq!(
        package::script(None),
        "Get-AppxPackage -Name Chyk.ShellMenu | Remove-AppxPackage"
    );
}
