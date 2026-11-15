use super::*;

#[test]
fn round_trips() {
    let s = Settings {
        lang: LangPref::Uk,
        another: Another::NewWindow,
        check_updates: false,
        format: Some(2),
        quality: 3,
        size: 4,
        precise: true,
        mute: true,
        save_to: SaveTo::LastFolder,
        last_folder: Some(PathBuf::from("/home/me/Videos/out")),
        suffix: "-cut".into(),
        sound: false,
        skip: 10.0,
        stop_at_out: false,
        mute_preview: true,
        hwdec: false,
        pad: 1.25,
        sensitivity: Sensitivity::High,
    };
    assert_eq!(Settings::parse(&s.to_text()), s);
    assert_eq!(
        Settings::parse(&Settings::default().to_text()),
        Settings::default()
    );
}

#[test]
fn bad_values_keep_defaults() {
    let s = Settings::parse(
        "[general]\nlang = \"fr\"\nformat = 9\nquality = 99\nskip = nan\npad = 500\nsound = yes\nnonsense\nfoo = 1\n",
    );
    let d = Settings::default();
    assert_eq!(s.lang, d.lang);
    assert_eq!(s.format, None);
    assert_eq!(s.quality, d.quality);
    assert_eq!(s.skip, d.skip);
    assert_eq!(s.pad, PAD_RANGE.1);
    assert_eq!(s.sound, d.sound);
}

#[test]
fn empty_suffix_and_folder() {
    let s = Settings::parse("suffix = \"\"\nlast_folder = \"\"\n");
    assert_eq!(s.suffix, "");
    assert_eq!(s.last_folder, None);
}

#[test]
fn suffix_drops_unsafe_characters() {
    assert_eq!(clean_suffix("-a/b:c*?\"<>|\\d\n"), "-abcd");
}

#[test]
fn writes_and_replaces() {
    let dir = std::env::temp_dir().join(format!("chyk-settings-{}", std::process::id()));
    let file = dir.join("sub/settings.toml");
    write(&file, "a = 1\n").unwrap();
    write(&file, "a = 2\n").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "a = 2\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn folder_with_line_breaks_round_trips() {
    for folder in [r"C:\Users\me\new\clips", "/tmp/a\nmute = 0\\n\r"] {
        let s = Settings {
            last_folder: Some(PathBuf::from(folder)),
            ..Settings::default()
        };
        let text = s.to_text();
        assert_eq!(
            text.lines().count(),
            Settings::default().to_text().lines().count()
        );
        assert_eq!(Settings::parse(&text), s);
    }
}

#[test]
fn text_values_keep_their_spaces() {
    let s = Settings {
        suffix: " edited ".into(),
        last_folder: Some(PathBuf::from("/tmp/exports ")),
        ..Settings::default()
    };
    assert_eq!(Settings::parse(&s.to_text()), s);
}

#[test]
fn writes_toml() {
    let text = Settings::default().to_text();
    assert!(text.contains("lang = \"system\"\n"));
    assert!(text.contains("check_updates = true\n"));
    assert!(text.contains("skip = 5.0\n"));
    assert!(text.contains("suffix = \"-edited\"\n"));
}

#[test]
fn reads_one_key_and_comments() {
    let s = Settings::parse("# set by the installer\ncheck_updates = false # off\n");
    assert_eq!(
        s,
        Settings {
            check_updates: false,
            ..Settings::default()
        }
    );
    assert_eq!(
        Settings::parse("sensitivity = 'high'\n").sensitivity,
        Sensitivity::High
    );
}

#[test]
fn reads_old_settings_txt() {
    let s = Settings::parse(
        "lang = uk\ncheck_updates = 0\nsuffix =  cut \nlast_folder = C:\\\\Users\\\\me\\nclips\nskip = 10\n",
    );
    assert_eq!(s.lang, LangPref::Uk);
    assert!(!s.check_updates);
    assert_eq!(s.suffix, " cut ");
    assert_eq!(s.last_folder, Some(PathBuf::from("C:\\Users\\me\nclips")));
    assert_eq!(s.skip, 10.0);
}

#[test]
fn reads_toml_escapes_and_rejects_open_strings() {
    let s = Settings::parse(
        "last_folder = \"/exports/\\U0001F3AC \\u0438\"\nsuffix = \"-\\u0441\"\ncheck_updates = \"false\n",
    );
    assert_eq!(s.last_folder, Some(PathBuf::from("/exports/🎬 и")));
    assert_eq!(s.suffix, "-с");
    assert!(s.check_updates);
    assert!(Settings::parse("check_updates = \"fal\\se\"\n").check_updates);
    let q = Settings::parse("last_folder = \"/a\\\"b\\\\c\"\n");
    assert_eq!(q.last_folder, Some(PathBuf::from("/a\"b\\c")));
}
