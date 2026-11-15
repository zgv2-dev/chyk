//! Windows' classic menu ("Show more options"): one verb per extension under
//! `HKCU\Software\Classes\SystemFileAssociations\.<ext>\shell\Chyk`, which Explorer reads whatever
//! app owns the type.

use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Per-extension verbs live under `<BASE>\.<ext>\shell\Chyk` in HKCU.
const BASE: &str = r"Software\Classes\SystemFileAssociations";
/// Where the first version of this put the verb (once, for the `video` perceived type).
#[cfg(windows)]
const OLD_KEY: &str = r"Software\Classes\SystemFileAssociations\video\shell\Chyk";

/// One registry value: subkey under the verb's key ("" for the key itself), value name ("" for the
/// default value) and its text.
pub(crate) type Value = (&'static str, &'static str, OsString);

/// The values that make the menu entry for `exe`, titled `label`. One file at a time, since a
/// Chyk window edits one video.
pub(crate) fn values(exe: &Path, label: &str) -> Vec<Value> {
    let quoted = |rest: &str| {
        let mut s = OsString::from("\"");
        s.push(exe);
        s.push(rest);
        s
    };
    vec![
        ("", "", label.into()),
        ("", "Icon", quoted("\",0")),
        ("", "MultiSelectModel", "Single".into()),
        ("command", "", quoted("\" \"%1\"")),
    ]
}

/// The verb's key for files ending in `.ext`.
pub(crate) fn key_path(ext: &str, sub: &str) -> OsString {
    let key = format!(r"{BASE}\.{ext}\shell\Chyk");
    if sub.is_empty() {
        key
    } else {
        format!(r"{key}\{sub}")
    }
    .into()
}

/// `cmd`, a registered command, starts `exe`. Windows paths compare case-insensitively.
pub(crate) fn matches(cmd: &OsStr, exe: &Path) -> bool {
    let want = values(exe, "").pop().map(|v| v.2).unwrap_or_default();
    cmd.to_string_lossy()
        .eq_ignore_ascii_case(&want.to_string_lossy())
}

#[cfg(windows)]
pub(super) use imp::{command, set};

#[cfg(windows)]
mod imp {
    use super::super::{exts, win};
    use super::*;

    /// The registered command (for any copy of Chyk). `.mp4` stands for all of them.
    pub fn command() -> Option<OsString> {
        win::get(&key_path("mp4", "command"), "")
    }

    /// Write or delete the verbs for every video extension.
    pub fn set(exe: &Path, on: bool, label: &str) -> Result<(), String> {
        let values = values(exe, label);
        // The first build put one key under the `video` perceived type, which Explorer skips
        // for extensions whose ProgID (VLC, the Photos app) sets no PerceivedType. Drop it.
        let mut r = win::delete(OsStr::new(OLD_KEY));
        for ext in exts() {
            let step = if on {
                values
                    .iter()
                    .try_for_each(|(sub, name, data)| win::put(&key_path(ext, sub), name, data))
            } else {
                win::delete(&key_path(ext, ""))
            };
            r = r.and(step);
        }
        win::notify();
        r
    }
}
