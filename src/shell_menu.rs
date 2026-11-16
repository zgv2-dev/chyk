//! "Edit with Chyk" in the file manager's right-click menu for videos. Written per user, so it
//! needs no admin rights: the app registers itself.
//! - Windows: Explorer's classic menu (`classic`) and Windows 11's main menu (`package`). An
//!   all-users install writes the classic entry under HKLM instead (`--register --all-users`); the
//!   Windows 11 entry is a package, which Windows only registers per user, so each user gets it on
//!   their first launch.
//! - Linux: Dolphin, Nemo, Nautilus, Caja, PCManFM-Qt and Thunar, plus Open With (`linux`).
//! - macOS: none to toggle. The bundle declares Open With and a Quick Action (`crate::mac_open`).

// The helpers are only called on some platforms; elsewhere only the tests use them.
#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]

#[cfg(any(windows, test))]
mod classic;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", test))]
mod linux_files;
#[cfg(any(windows, test))]
mod manifest;
mod package;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_linux;
#[cfg(any(target_os = "linux", test))]
mod thunar;
#[cfg(windows)]
mod win;

pub use package::Win11;

/// Whether this platform has the toggle. Elsewhere [`set`] fails and [`is_on`] is false.
pub const SUPPORTED: bool = cfg!(any(windows, target_os = "linux"));

/// The extensions the entry is registered for: every video Chyk opens.
fn exts() -> &'static [&'static str] {
    crate::app::videos::VIDEO_EXTS
}

/// The entry's title.
fn label() -> &'static str {
    crate::i18n::t!("shell.edit_with")
}

/// The exe the entry starts: on Linux the AppImage itself, not its temporary mount.
fn exe() -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "linux")]
    if let Some(a) = std::env::var_os("APPIMAGE") {
        return Ok(a.into());
    }
    std::env::current_exe().map_err(|e| e.to_string())
}

/// The menu entry is registered (for any copy of Chyk).
pub fn is_on() -> bool {
    #[cfg(windows)]
    return classic::command().is_some();
    #[cfg(target_os = "linux")]
    return linux::is_on();
    #[cfg(not(any(windows, target_os = "linux")))]
    false
}

/// Add the entry for this exe, or remove it. On Windows the classic entry (an error fails the
/// whole call), then the Windows 11 one; elsewhere the extra part is [`Win11::Unavailable`].
/// Slow (PowerShell for the package): keep it off the UI thread.
pub fn set(on: bool) -> Result<Win11, String> {
    set_for(on, false)
}

/// [`set`], with the classic entry for all users (HKLM, needs admin) when `all_users`. Removing
/// for all users also removes this user's own entry.
pub fn set_for(on: bool, all_users: bool) -> Result<Win11, String> {
    #[cfg(windows)]
    {
        let exe = exe()?;
        if all_users && !on {
            classic::set(&exe, false, label(), false)?;
        }
        classic::set(&exe, on, label(), all_users)?;
        Ok(package::set(on, &exe, label()))
    }
    #[cfg(target_os = "linux")]
    {
        let _ = all_users; // Linux entries are always per user.
        linux::set(on, &exe()?, label())?;
        Ok(Win11::Unavailable)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (on, all_users);
        Err("There is no menu to change on this platform.".into())
    }
}

/// Removes only the all-users classic entry (HKLM, needs admin), leaving every account's own
/// entries and Windows 11 package alone. An elevated uninstall runs this: its account may not be
/// the one that asked, so per-user removal waits for the original process.
#[cfg(windows)]
pub fn remove_all_users() -> Result<(), String> {
    classic::set(&exe()?, false, label(), true)
}

/// Writes the Windows 11 package's files into `dir`, so a later per-user registration needs no
/// write access there (an all-users install in Program Files). Setup calls this.
#[cfg(windows)]
pub fn write_package_files(dir: &std::path::Path) -> std::io::Result<()> {
    manifest::write_files(dir)
}

/// On launch: if the entry is on but points at another path (the exe or AppImage was moved or
/// renamed, or Chyk updated), point it here. Under an all-users entry for this exe, add the
/// Windows 11 entry for this user once. Debug builds leave it alone so `cargo run` doesn't take
/// it over.
pub fn refresh() {
    if cfg!(debug_assertions) {
        return;
    }
    let Ok(exe) = exe() else { return };
    #[cfg(windows)]
    if let Some(cmd) = classic::command() {
        if !classic::matches(&cmd, &exe) {
            // Rewrites the classic keys; the package follows below if it is stale too.
            let _ = classic::set(&exe, true, label(), false);
        }
        package::refresh(&exe, label());
    } else if classic::command_all_users().is_some_and(|c| classic::matches(&c, &exe)) {
        package::first_run(&exe, label());
    }
    #[cfg(target_os = "linux")]
    linux::refresh(&exe, label());
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = exe;
}

/// `chyk --register` / `chyk --unregister` (`--all-users` after it for HKLM): change the menu and
/// quit with 0 on success, 1 on failure, and, when only the classic entry was added, 2 if the Windows 11 one needs Developer
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
