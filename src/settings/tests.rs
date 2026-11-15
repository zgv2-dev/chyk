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
        "lang = fr\nformat = 9\nquality = 99\nskip = nan\npad = 500\nsound = yes\nnonsense\nfoo = 1\n",
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
    let s = Settings::parse("suffix =\nlast_folder =\n");
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
    let file = dir.join("sub/settings.txt");
    write(&file, "a = 1\n").unwrap();
    write(&file, "a = 2\n").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "a = 2\n");
    let _ = std::fs::remove_dir_all(dir);
}
