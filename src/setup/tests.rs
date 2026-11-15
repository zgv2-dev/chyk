use super::*;

fn args(a: &[&str]) -> Vec<OsString> {
    a.iter().map(OsString::from).collect()
}

#[test]
fn setup_shows_only_for_a_plain_launch_of_the_installer() {
    assert_eq!(decide(&[], true), Some(Mode::Install));
    assert_eq!(
        decide(&[], false),
        None,
        "installed copy, portable or debug build"
    );
    assert_eq!(decide(&args(&["clip.mp4"]), true), None, "opening a file");
}

#[test]
fn flags_win_over_state() {
    assert_eq!(
        decide(&args(&["--uninstall"]), false),
        Some(Mode::Uninstall)
    );
    assert_eq!(decide(&args(&["--setup"]), false), Some(Mode::Install));
}

#[test]
fn elevated_restarts_carry_their_choices() {
    assert_eq!(
        decide(
            &args(&["--setup", "--all-users", r"C:\Program Files\Chyk"]),
            false
        ),
        Some(Mode::InstallAllUsers(PathBuf::from(
            r"C:\Program Files\Chyk"
        )))
    );
    assert_eq!(
        decide(
            &args(&["--uninstall", "--confirm", "--keep-settings"]),
            false
        ),
        Some(Mode::UninstallNow {
            keep_settings: true
        })
    );
    assert_eq!(
        decide(&args(&["--uninstall", "--confirm"]), false),
        Some(Mode::UninstallNow {
            keep_settings: false
        })
    );
}

#[test]
fn command_line_quotes_each_argument_whole() {
    let line = command_line(&args(&[
        "--setup",
        r"C:\Program Files\Chyk\",
        r#"say "hi""#,
    ]));
    assert_eq!(
        line,
        OsString::from(r#""--setup" "C:\Program Files\Chyk\\" "say \"hi\"""#)
    );
}

#[test]
fn writable_checks_the_closest_existing_folder_without_creating_it() {
    let tmp = std::env::temp_dir().join(format!("chyk-writable-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let missing = tmp.join("a").join("Chyk");
    assert!(writable(&missing));
    assert!(!missing.exists(), "nothing is created");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root writes anywhere; the check only means something for a normal user.
        let root = std::fs::write(tmp.join("probe"), b"").is_ok();
        assert_eq!(writable(&missing), root);
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::remove_dir_all(&tmp).unwrap();
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
fn cleanup_retries_and_keeps_paths_out_of_the_command() {
    let (script, env) = cleanup_script(
        &[
            PathBuf::from(r"C:\A %OS%\Chyk.exe"),
            PathBuf::from(r"C:\A %OS%\Chyk.old.exe"),
        ],
        &[PathBuf::from(r"C:\A %OS%")],
        &[PathBuf::from(r"C:\L\Chyk")],
    );
    assert_eq!(
        script,
        r#"for /l %i in (1,1,60) do @(ping -n 2 127.0.0.1 >nul & if defined CHYK_P0 (del /f /q "%CHYK_P0%" 2>nul & if not exist "%CHYK_P0%" set CHYK_P0=) & if defined CHYK_P1 (del /f /q "%CHYK_P1%" 2>nul & if not exist "%CHYK_P1%" set CHYK_P1=) & if defined CHYK_P2 (rmdir "%CHYK_P2%" 2>nul & if not exist "%CHYK_P2%" set CHYK_P2=) & if defined CHYK_P3 (rmdir /s /q "%CHYK_P3%" 2>nul & if not exist "%CHYK_P3%" set CHYK_P3=) & if not defined CHYK_P0 if not defined CHYK_P1 if not defined CHYK_P2 if not defined CHYK_P3 exit)"#
    );
    assert_eq!(env[0], ("CHYK_P0".into(), r"C:\A %OS%\Chyk.exe".into()));
    assert_eq!(env.len(), 4);
    assert!(!script.contains("OS%\\"), "paths stay out of the script");
}
