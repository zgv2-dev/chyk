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
        r#""/a \\"b\\" \\`c\\` 100%% \\\\d""#
    );
}

#[test]
fn dolphin_menu_is_top_level_for_one_video() {
    let d = dolphin(&entry());
    assert!(d.contains("Type=Service\nMimeType=video/*;"));
    assert!(d.contains("X-KDE-Priority=TopLevel"));
    assert!(d.contains(r#"Exec="/home/me/Apps/Chyk \\$1.AppImage" %f"#));
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
    let once = uca_insert(doc, &entry(), &["mp4"]).unwrap();
    let twice = uca_insert(&once, &entry(), &["mp4"]).unwrap();
    assert_eq!(once, twice);
    assert!(once.contains("<patterns>*.mp4;*.MP4</patterns>"));
    assert!(once.contains("<name>Term</name>"));
    assert_eq!(uca_remove(&once), doc);
    assert!(
        uca_insert("", &entry(), &["mp4"])
            .unwrap()
            .contains("<actions>\n<action>")
    );
    // A closing tag with a space still works; a file we can't read as actions is left alone.
    let spaced = doc.replace("</actions>", "</actions >");
    assert!(
        uca_insert(&spaced, &entry(), &["mp4"])
            .unwrap()
            .ends_with("</actions >\n")
    );
    assert_eq!(uca_insert("<junk/>", &entry(), &["mp4"]), None);
    // Chyk's action cut off before its end tag: nothing after it is removed.
    let cut = "<actions>\n<action><unique-id>chyk-edit</unique-id><name>Mine";
    assert_eq!(uca_remove(cut), cut);
    assert_eq!(uca_insert(cut, &entry(), &["mp4"]), None);
    let open = "<actions>\n<action><unique-id>chyk-edit</unique-id>\n<action><name>Mine</name></action>\n</actions>\n";
    assert_eq!(uca_remove(open), open);
    assert!(
        uca_insert(open, &entry(), &["mp4"])
            .unwrap()
            .contains("<name>Mine</name>")
    );
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
