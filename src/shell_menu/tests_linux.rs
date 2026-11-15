use super::linux_files::*;
use super::thunar::*;
use std::path::Path;

fn entry() -> Entry<'static> {
    Entry {
        exe: Path::new("/home/me/Apps/Chyk $1.AppImage"),
        label: "Edit with Chyk",
        icon: Path::new("/home/me/.local/share/chyk/chyk.png"),
    }
}

#[test]
fn exec_arg_quotes_for_desktop_files() {
    assert_eq!(exec_arg(Path::new("/opt/chyk")), r#""/opt/chyk""#);
    assert_eq!(
        exec_arg(Path::new(r#"/a "b" `c` 100% \d"#)),
        r#""/a \"b\" \`c\` 100%% \\\\d""#
    );
}

#[test]
fn dolphin_menu_is_top_level_for_one_video() {
    let d = dolphin(&entry());
    assert!(d.contains("Type=Service\nMimeType=video/*;"));
    assert!(d.contains("X-KDE-Priority=TopLevel"));
    assert!(d.contains(r#"Exec="/home/me/Apps/Chyk \$1.AppImage" %f"#));
    assert!(d.contains("Name=Edit with Chyk"));
}

#[test]
fn nemo_and_fm_actions() {
    let n = nemo(&entry(), &["mp4", "mkv"]);
    assert!(n.contains("Extensions=mp4;mkv;\n") && n.contains("Selection=s"));
    let f = fm_action(&entry());
    assert!(f.contains("Type=Action") && f.contains("SelectionCount==1"));
    assert!(f.contains("MimeTypes=video/*;"));
}

#[test]
fn nautilus_entries() {
    let py = nautilus_py(&entry(), &["mp4", "mkv"]);
    assert!(py.contains(r#"EXE = "/home/me/Apps/Chyk $1.AppImage""#));
    assert!(py.contains(r#"EXTS = {"mp4", "mkv"}"#));
    let e = Entry {
        exe: Path::new("/it's/chyk"),
        ..entry()
    };
    let sh = nautilus_script(&e);
    assert!(sh.contains(SCRIPT_MARK));
    assert!(sh.ends_with("exec '/it'\\''s/chyk' \"$1\"\n"));
}

#[test]
fn thunar_action_is_added_once_and_removed() {
    let doc =
        "<?xml version=\"1.0\"?>\n<actions>\n<action><name>Term</name></action>\n</actions>\n";
    let once = uca_insert(doc, &entry(), &["mp4"]);
    let twice = uca_insert(&once, &entry(), &["mp4"]);
    assert_eq!(once, twice);
    assert!(once.contains("<patterns>*.mp4;*.MP4</patterns>"));
    assert!(once.contains("<name>Term</name>"));
    assert_eq!(uca_remove(&once), doc);
    assert!(uca_insert("", &entry(), &["mp4"]).contains("<actions>\n<action>"));
}

#[test]
fn open_with_types_match_the_desktop_file() {
    let shipped = include_str!("../../assets/chyk.desktop");
    let line = format!("MimeType={};", MIME_TYPES.join(";"));
    assert!(
        shipped.lines().any(|l| l == line),
        "assets/chyk.desktop: {line}"
    );
    assert!(open_with(&entry()).contains(&line));
}
