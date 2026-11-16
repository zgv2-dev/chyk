//! Windows' classic menu ("Show more options"): one verb per extension under
//! `Software\Classes\SystemFileAssociations\.<ext>\shell\Chyk`, which Explorer reads whatever
//! app owns the type: in HKCU, or in HKLM for an all-users install.

use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Per-extension verbs live under `<BASE>\.<ext>\shell\Chyk`.
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
pub(super) use imp::{command, command_all_users, set};

#[cfg(windows)]
mod imp {
    use super::super::{exts, win};
    use super::*;

    /// The registered command (for any copy of Chyk). `.mp4` stands for all of them.
    pub fn command() -> Option<OsString> {
        win::get(&key_path("mp4", "command"), "")
    }

    /// The command registered for all users (HKLM), if any.
    pub fn command_all_users() -> Option<OsString> {
        win::get_lm_os(&key_path("mp4", "command"), "")
    }

    /// Write or delete the verbs for every video extension, under HKLM when `all_users`.
    pub fn set(exe: &Path, on: bool, label: &str, all_users: bool) -> Result<(), String> {
        let values = values(exe, label);
        let hive = win::hive(all_users);
        // The first build put one key under the `video` perceived type, which Explorer skips
        // for extensions whose ProgID (VLC, the Photos app) sets no PerceivedType. Drop it.
        // A per-user key: the all-users removal (maybe another account, elevated) leaves it alone.
        let mut r = if all_users && !on {
            Ok(())
        } else {
            win::delete(OsStr::new(OLD_KEY))
        };
        for ext in exts() {
            let step = if on {
                values.iter().try_for_each(|(sub, name, data)| {
                    win::put_in(hive, &key_path(ext, sub), name, data)
                })
            } else {
                win::delete_in(hive, &key_path(ext, ""))
            };
            r = r.and(step);
        }
        win::notify();
        r
    }
}
