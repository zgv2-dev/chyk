//! The text of each Linux file manager's menu entry. Pure, so the tests can check it; `linux`
//! writes it out.

use std::path::Path;

/// Video MIME types (shared-mime-info names) for Open With, matching `VIDEO_EXTS`. Desktop files
/// may not use wildcards here; the menus below can and use `video/*`.
pub(crate) const MIME_TYPES: &[&str] = &[
    "video/mp4",
    "video/x-m4v",
    "video/quicktime",
    "video/x-matroska",
    "video/webm",
    "video/x-msvideo",
    "video/x-ms-wmv",
    "video/x-ms-asf",
    "video/x-flv",
    "video/mpeg",
    "video/mp2t",
    "video/ogg",
    "video/3gpp",
    "video/3gpp2",
    "application/mxf",
    "video/dv",
    "application/vnd.rn-realmedia",
    "application/vnd.rn-realmedia-vbr",
];

/// What every entry needs: the program, the title and an icon file.
pub(crate) struct Entry<'a> {
    pub exe: &'a Path,
    pub label: &'a str,
    pub icon: &'a Path,
}

/// `exe` as one argument of a desktop-file `Exec` line: double-quoted with `"`, `` ` ``, `$` and
/// `\` backslash-escaped, then every backslash escaped again for the key file (so `\\$`, and
/// `\\\\` for one backslash) and `%` doubled. GLib's shell parser (Nemo, Caja, Thunar) reads the
/// same double-quote rules.
pub(crate) fn exec_arg(exe: &Path) -> String {
    let mut out = String::from("\"");
    for c in exe.to_string_lossy().chars() {
        match c {
            '"' | '`' | '$' => out.extend(['\\', '\\', c]),
            '\\' => out.push_str(r"\\\\"),
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A key-file value: no line breaks, backslashes escaped.
fn value(s: &str) -> String {
    s.replace('\\', r"\\").replace('\n', " ")
}

/// Open With, for the AppImage (the .deb installs `assets/chyk.desktop` instead).
pub(crate) fn open_with(e: &Entry) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Chyk\nComment=Trim and crop videos\n\
         Exec={} %f\nIcon={}\nCategories=AudioVideo;Video;\nMimeType={};\n",
        exec_arg(e.exe),
        value(&e.icon.to_string_lossy()),
        MIME_TYPES.join(";")
    )
}

/// Dolphin (KDE): a service menu, at the top level of the menu.
pub(crate) fn dolphin(e: &Entry) -> String {
    format!(
        "[Desktop Entry]\nType=Service\nMimeType=video/*;\nActions=chykEdit;\n\
         X-KDE-Priority=TopLevel\nX-KDE-MaxNumberOfUrls=1\n\n\
         [Desktop Action chykEdit]\nName={}\nIcon={}\nExec={} %f\n",
        value(e.label),
        value(&e.icon.to_string_lossy()),
        exec_arg(e.exe)
    )
}

/// Nemo (Cinnamon): an action for one file with a video extension.
pub(crate) fn nemo(e: &Entry, exts: &[&str]) -> String {
    format!(
        "[Nemo Action]\nName={}\nComment={}\nExec={} %F\nIcon-Name={}\nSelection=s\nExtensions={};\n",
        value(e.label),
        value(e.label),
        exec_arg(e.exe),
        value(&e.icon.to_string_lossy()),
        exts.join(";")
    )
}

/// The FileManager-Actions format, read by Caja (MATE) and PCManFM-Qt (LXQt).
pub(crate) fn fm_action(e: &Entry) -> String {
    format!(
        "[Desktop Entry]\nType=Action\nName={}\nIcon={}\nProfiles=edit;\n\n\
         [X-Action-Profile edit]\nMimeTypes=video/*;\nSelectionCount==1\nExec={} %f\n",
        value(e.label),
        value(&e.icon.to_string_lossy()),
        exec_arg(e.exe)
    )
}

/// A Python string literal (JSON's escapes are valid Python).
fn py_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' => out.extend(['\\', c]),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Nautilus (GNOME) with nautilus-python: a top-level item for one video.
pub(crate) fn nautilus_py(e: &Entry, exts: &[&str]) -> String {
    let exts: Vec<_> = exts.iter().map(|x| py_str(x)).collect();
    format!(
        r#"# Edit with Chyk. Written by Chyk; untick the option in Chyk to remove it.
import os
import subprocess
from gi.repository import GObject, Nautilus

EXE = {exe}
LABEL = {label}
EXTS = {{{exts}}}


class ChykEdit(GObject.GObject, Nautilus.MenuProvider):
    def get_file_items(self, *args):
        files = args[-1]
        if len(files) != 1:
            return []
        path = files[0].get_location().get_path()
        if not path or os.path.splitext(path)[1][1:].lower() not in EXTS:
            return []
        item = Nautilus.MenuItem(name="ChykEdit::edit", label=LABEL)
        item.connect("activate", lambda *_: subprocess.Popen([EXE, path]))
        return [item]
"#,
        exe = py_str(&e.exe.to_string_lossy()),
        label = py_str(e.label),
        exts = exts.join(", ")
    )
}

/// The line that marks Chyk's Nautilus script, whatever its (translated) file name.
pub(crate) const SCRIPT_MARK: &str = "# chyk-edit";

/// Nautilus without nautilus-python: a script under Scripts (shown for any file).
pub(crate) fn nautilus_script(e: &Entry) -> String {
    let exe = e.exe.to_string_lossy().replace('\'', r"'\''");
    format!("#!/bin/sh\n{SCRIPT_MARK}\nexec '{exe}' \"$1\"\n")
}
