//! "Edit with Chyk" in Explorer's right-click menu for videos (Windows). Written per user under
//! HKCU, so it needs no admin and no installer: the portable exe registers itself. The key sits
//! under `SystemFileAssociations\video`, which covers every extension Windows calls a video.

// The registry helpers are only called on Windows; elsewhere only the tests use them.
#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Whether this platform has the menu. Elsewhere [`set`] fails and [`is_on`] is false.
pub const SUPPORTED: bool = cfg!(windows);

/// The verb's key under HKCU.
const KEY: &str = r"Software\Classes\SystemFileAssociations\video\shell\Chyk";

#[cfg(test)]
mod tests;
#[cfg(windows)]
mod win;

/// One registry value: subkey under [`KEY`] ("" for the key itself), value name ("" for the
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

/// The full subkey path of a [`Value`].
pub(crate) fn key_path(sub: &str) -> OsString {
    if sub.is_empty() {
        KEY.into()
    } else {
        format!(r"{KEY}\{sub}").into()
    }
}

/// The menu entry is registered (for any copy of Chyk).
pub fn is_on() -> bool {
    #[cfg(windows)]
    return win::get(&key_path("command"), "").is_some();
    #[cfg(not(windows))]
    false
}

/// Add the entry for this exe, or remove it.
pub fn set(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let r = if on {
            values(&exe, crate::i18n::t!("shell.edit_with"))
                .iter()
                .try_for_each(|(sub, name, data)| win::put(&key_path(sub), name, data))
        } else {
            win::delete(OsStr::new(KEY))
        };
        win::notify();
        r
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("Explorer's menu exists only on Windows.".into())
    }
}

/// On launch: if the entry is on but points at another path (the exe was moved or renamed),
/// point it here. Debug builds leave it alone so `cargo run` doesn't take it over.
pub fn refresh() {
    #[cfg(windows)]
    if !cfg!(debug_assertions)
        && let Some(cmd) = win::get(&key_path("command"), "")
        && let Ok(exe) = std::env::current_exe()
        && !matches(&cmd, &exe)
    {
        let _ = set(true);
    }
}

/// `cmd`, a registered command, starts `exe`. Windows paths compare case-insensitively.
pub(crate) fn matches(cmd: &OsStr, exe: &Path) -> bool {
    let want = values(exe, "").pop().map(|v| v.2).unwrap_or_default();
    cmd.to_string_lossy()
        .eq_ignore_ascii_case(&want.to_string_lossy())
}

/// `chyk --register` / `chyk --unregister`: change the menu and quit with 0 on success, 1 on
/// failure. `None`: not one of these flags, start normally.
pub fn from_args() -> Option<i32> {
    let on = match std::env::args_os().nth(1)?.to_str()? {
        "--register" => true,
        "--unregister" => false,
        _ => return None,
    };
    Some(i32::from(set(on).is_err()))
}
