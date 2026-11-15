use super::*;

fn args(a: &[&str]) -> Vec<OsString> {
    a.iter().map(OsString::from).collect()
}

#[test]
fn setup_shows_only_for_a_fresh_plain_launch() {
    assert_eq!(decide(&[], false, false, false), Some(Mode::Install));
    assert_eq!(decide(&[], true, false, false), None, "already installed");
    assert_eq!(
        decide(&[], false, true, false),
        None,
        "chose to run without installing"
    );
    assert_eq!(decide(&[], false, false, true), None, "debug build");
    assert_eq!(
        decide(&args(&["clip.mp4"]), false, false, false),
        None,
        "opening a file"
    );
}

#[test]
fn flags_win_over_state() {
    assert_eq!(
        decide(&args(&["--uninstall"]), true, true, true),
        Some(Mode::Uninstall)
    );
    assert_eq!(
        decide(&args(&["--setup"]), true, true, true),
        Some(Mode::Install)
    );
}

#[test]
fn picked_folder_gets_a_chyk_folder() {
    assert_eq!(folder_for(Path::new("D:/Apps")), Path::new("D:/Apps/Chyk"));
    assert_eq!(
        folder_for(Path::new("D:/Apps/chyk")),
        Path::new("D:/Apps/chyk")
    );
}

#[test]
fn uninstall_entry_runs_this_exe() {
    let exe = Path::new(r"C:\P\Chyk\Chyk.exe");
    let v = uninstall_values(exe, 180_000);
    let get = |n| v.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone());
    assert_eq!(
        get("UninstallString"),
        Some(Value::Str(r#""C:\P\Chyk\Chyk.exe" --uninstall"#.into()))
    );
    assert_eq!(get("DisplayVersion"), Some(Value::Str(VERSION.into())));
    assert_eq!(get("EstimatedSize"), Some(Value::Dword(180_000)));
}

#[test]
fn assoc_covers_every_video_type_and_opens_the_file() {
    let v = assoc_values(Path::new(r"C:\P\Chyk.exe"));
    let open = v
        .iter()
        .find(|(k, _, _)| k.ends_with(r"shell\open\command"))
        .unwrap();
    assert_eq!(open.2, OsString::from(r#""C:\P\Chyk.exe" "%1""#));
    for ext in crate::app::videos::VIDEO_EXTS {
        let key = format!(r"Software\Classes\.{ext}\OpenWithProgids");
        assert!(v.iter().any(|(k, n, _)| *k == key && n == PROG_ID), "{ext}");
    }
    // Uninstall deletes these keys whole; they must never be shared ones.
    for key in OWNED {
        assert!(key.contains("Chyk"), "{key}");
    }
}

#[test]
fn flag_replaces_its_line_and_keeps_the_rest() {
    let t = with_flag(
        "lang = uk\ncheck_updates = 1\nsound = 0\n",
        "check_updates",
        false,
    );
    assert_eq!(t, "lang = uk\nsound = 0\ncheck_updates = 0\n");
    assert_eq!(with_flag("", "check_updates", false), "check_updates = 0\n");
}

#[test]
fn cleanup_waits_then_deletes_quoted_paths() {
    let s = cleanup_script(
        &[PathBuf::from(r"C:\A B\Chyk.exe")],
        &[PathBuf::from(r"C:\A B")],
        &[PathBuf::from(r"C:\L\Chyk")],
    );
    assert_eq!(
        s,
        r#"ping -n 3 127.0.0.1 >nul & del /f /q "C:\A B\Chyk.exe" & rmdir "C:\A B" & rmdir /s /q "C:\L\Chyk""#
    );
}
