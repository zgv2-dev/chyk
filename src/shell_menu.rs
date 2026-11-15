//! "Edit with Chyk" in Explorer's right-click menu for videos (Windows). Written per user under
//! HKCU, so it needs no admin and no installer: the portable exe registers itself. One key per
//! extension under `SystemFileAssociations\.<ext>`, which Explorer reads whatever app owns the type.

// The registry helpers are only called on Windows; elsewhere only the tests use them.
#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Whether this platform has the menu. Elsewhere [`set`] fails and [`is_on`] is false.
pub const SUPPORTED: bool = cfg!(windows);

/// Per-extension verbs live under `<BASE>\.<ext>\shell\Chyk` in HKCU.
const BASE: &str = r"Software\Classes\SystemFileAssociations";
/// Where the first version of this put the verb (once, for the `video` perceived type).
const OLD_KEY: &str = r"Software\Classes\SystemFileAssociations\video\shell\Chyk";

mod manifest;
mod package;
#[cfg(test)]
mod tests;

pub use package::Win11;
#[cfg(windows)]
mod win;

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

/// The extensions the entry is registered for: every video Chyk opens.
fn exts() -> &'static [&'static str] {
    crate::app::videos::VIDEO_EXTS
}

/// The menu entry is registered (for any copy of Chyk). `.mp4` stands for all of them.
pub fn is_on() -> bool {
    #[cfg(windows)]
    return win::get(&key_path("mp4", "command"), "").is_some();
    #[cfg(not(windows))]
    false
}

/// Add the entry for this exe, or remove it: the classic one (an error fails the whole call),
/// then the Windows 11 one. Slow (PowerShell for the package): keep it off the UI thread.
pub fn set(on: bool) -> Result<Win11, String> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        set_classic(&exe, on)?;
        Ok(package::set(on, &exe, crate::i18n::t!("shell.edit_with")))
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("Explorer's menu exists only on Windows.".into())
    }
}

/// The per-extension registry verbs ("Show more options" on Windows 11).
#[cfg(windows)]
fn set_classic(exe: &Path, on: bool) -> Result<(), String> {
    let values = values(exe, crate::i18n::t!("shell.edit_with"));
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

/// On launch: if the entry is on but points at another path (the exe was moved or renamed),
/// point it here. Debug builds leave it alone so `cargo run` doesn't take it over.
pub fn refresh() {
    #[cfg(windows)]
    if !cfg!(debug_assertions)
        && let Some(cmd) = win::get(&key_path("mp4", "command"), "")
        && let Ok(exe) = std::env::current_exe()
    {
        if !matches(&cmd, &exe) {
            // Rewrites the classic keys; the package follows below if it is stale too.
            let _ = set_classic(&exe, true);
        }
        package::refresh(&exe, crate::i18n::t!("shell.edit_with"));
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
