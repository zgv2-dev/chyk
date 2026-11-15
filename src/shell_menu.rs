//! "Edit with Chyk" in the file manager's right-click menu for videos. Written per user, so it
//! needs no admin rights and no installer: the app registers itself.
//! - Windows: Explorer's classic menu (`classic`) and Windows 11's main menu (`package`).
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
    #[cfg(windows)]
    {
        let exe = exe()?;
        classic::set(&exe, on, label())?;
        Ok(package::set(on, &exe, label()))
    }
    #[cfg(target_os = "linux")]
    {
        linux::set(on, &exe()?, label())?;
        Ok(Win11::Unavailable)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = on;
        Err("There is no menu to change on this platform.".into())
    }
}

/// On launch: if the entry is on but points at another path (the exe or AppImage was moved or
/// renamed, or Chyk updated), point it here. Debug builds leave it alone so `cargo run` doesn't
/// take it over.
pub fn refresh() {
    if cfg!(debug_assertions) {
        return;
    }
    let Ok(exe) = exe() else { return };
    #[cfg(windows)]
    if let Some(cmd) = classic::command() {
        if !classic::matches(&cmd, &exe) {
            // Rewrites the classic keys; the package follows below if it is stale too.
            let _ = classic::set(&exe, true, label());
        }
        package::refresh(&exe, label());
    }
    #[cfg(target_os = "linux")]
    linux::refresh(&exe, label());
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = exe;
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
