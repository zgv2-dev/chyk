//! "Edit with Chyk" in Explorer's right-click menu for videos (Windows). Written per user under
//! HKCU, so it needs no admin: the portable exe registers itself. An all-users install writes the
//! classic entry under HKLM instead (`--register --all-users`); the Windows 11 entry is a package,
//! which Windows only registers per user, so each user gets it on their first launch. One key per
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
    set_for(on, false)
}

/// [`set`], with the classic entry for all users (HKLM, needs admin) when `all_users`. Removing
/// for all users also removes this user's own entry.
pub fn set_for(on: bool, all_users: bool) -> Result<Win11, String> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        if all_users && !on {
            set_classic(&exe, false, false)?;
        }
        set_classic(&exe, on, all_users)?;
        Ok(package::set(on, &exe, crate::i18n::t!("shell.edit_with")))
    }
    #[cfg(not(windows))]
    {
        let _ = (on, all_users);
        Err("Explorer's menu exists only on Windows.".into())
    }
}

/// Writes the Windows 11 package's files into `dir`, so a later per-user registration needs no
/// write access there (an all-users install in Program Files). Setup calls this.
pub fn write_package_files(dir: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    return package::write_files(dir);
    #[cfg(not(windows))]
    {
        let _ = dir;
        Ok(())
    }
}

/// The per-extension registry verbs ("Show more options" on Windows 11).
#[cfg(windows)]
fn set_classic(exe: &Path, on: bool, all_users: bool) -> Result<(), String> {
    let values = values(exe, crate::i18n::t!("shell.edit_with"));
    let hive = win::hive(all_users);
    // The first build put one key under the `video` perceived type, which Explorer skips
    // for extensions whose ProgID (VLC, the Photos app) sets no PerceivedType. Drop it.
    let mut r = win::delete(OsStr::new(OLD_KEY));
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

/// On launch: if the entry is on but points at another path (the exe was moved or renamed),
/// point it here. Under an all-users entry for this exe, add the Windows 11 entry for this user
/// once. Debug builds leave it alone so `cargo run` doesn't take it over.
pub fn refresh() {
    #[cfg(windows)]
    if !cfg!(debug_assertions)
        && let Ok(exe) = std::env::current_exe()
    {
        let label = crate::i18n::t!("shell.edit_with");
        if let Some(cmd) = win::get(&key_path("mp4", "command"), "") {
            if !matches(&cmd, &exe) {
                // Rewrites the classic keys; the package follows below if it is stale too.
                let _ = set_classic(&exe, true, false);
            }
            package::refresh(&exe, label);
        } else if win::get_lm_os(&key_path("mp4", "command"), "").is_some_and(|c| matches(&c, &exe))
        {
            package::first_run(&exe, label);
        }
    }
}

/// `cmd`, a registered command, starts `exe`. Windows paths compare case-insensitively.
pub(crate) fn matches(cmd: &OsStr, exe: &Path) -> bool {
    let want = values(exe, "").pop().map(|v| v.2).unwrap_or_default();
    cmd.to_string_lossy()
        .eq_ignore_ascii_case(&want.to_string_lossy())
}

/// `chyk --register` / `chyk --unregister` (`--all-users` after it for HKLM): change the menu and quit with 0 on success, 1 on
/// failure, and, when only the classic entry was added, 2 if the Windows 11 one needs Developer
/// Mode or 3 if it failed (setup reads these). `None`: not one of these flags, start normally.
pub fn from_args() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    let on = match args.next()?.to_str()? {
        "--register" => true,
        "--unregister" => false,
        _ => return None,
    };
    let all_users = args.next().is_some_and(|a| a == "--all-users");
    Some(match set_for(on, all_users) {
        Ok(Win11::NeedsDevMode) => 2,
        Ok(Win11::Failed(_)) => 3,
        Ok(_) => 0,
        Err(_) => 1,
    })
}
