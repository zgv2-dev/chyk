//! The Windows 11 top-level menu entry: an unsigned sparse package whose COM surrogate loads
//! `chyk_shell.dll` (the `shell/` crate). Both ride in the exe's bundle (`src/bundle.rs`), and the
//! package points at that folder. Registering a loose manifest needs Developer Mode.

use std::ffi::OsStr;
use std::path::Path;

#[cfg(any(windows, test))]
pub(crate) use super::manifest::manifest;

/// The package's name, for `Get-AppxPackage`.
pub(crate) const NAME: &str = "Chyk.ShellMenu";
/// The IExplorerCommand class; must match `CLSID` in `shell/src/lib.rs`.
pub(crate) const CLSID: &str = "010d3239-a0f9-413b-b13e-80de0d5aa0f9";
/// What the DLL reads: the exe to start, the entry's title, and the folder the package uses.
pub(crate) const SETTINGS: &str = r"Software\Chyk\Menu";

/// How the Windows 11 part went.
#[derive(Debug, PartialEq)]
pub enum Win11 {
    Added,
    Removed,
    /// Windows 10, or a build without the bundled DLL: the classic entry only.
    Unavailable,
    /// Loose packages need Developer Mode (Settings → System → For developers).
    NeedsDevMode,
    Failed(String),
}

/// `dir\AppxManifest.xml`, spelled with a backslash on any OS (the tests run elsewhere too).
fn manifest_path(dir: &Path) -> std::ffi::OsString {
    let mut p = dir.as_os_str().to_owned();
    p.push(r"\AppxManifest.xml");
    p
}

/// A PowerShell single-quoted string.
pub(crate) fn ps_quote(s: &OsStr) -> String {
    // PowerShell also ends a quoted string at the typographic quotes, so double those too.
    let mut out = String::from("'");
    for c in s.to_string_lossy().chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// The script that (re)registers the package from `dir`, or removes it.
pub(crate) fn script(dir: Option<&Path>) -> String {
    let remove = format!("Get-AppxPackage -Name {NAME} | Remove-AppxPackage");
    match dir {
        Some(d) => format!(
            "$ErrorActionPreference = 'Stop'; {remove}; Add-AppxPackage -Register {} -ExternalLocation {}",
            ps_quote(&manifest_path(d)),
            ps_quote(d.as_os_str())
        ),
        None => remove,
    }
}

#[cfg(windows)]
pub(super) use imp::{refresh, set};

#[cfg(windows)]
mod imp {
    use super::super::{exts, win};
    use super::*;
    use std::process::Command;

    fn windows_11() -> bool {
        let build = win::get_lm(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        );
        build
            .and_then(|b| b.to_str()?.parse::<u32>().ok())
            .is_some_and(|b| b >= 22000)
    }

    fn dev_mode() -> bool {
        let key = r"SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock";
        win::dword_lm(key, "AllowDevelopmentWithoutDevLicense") == Some(1)
    }

    fn powershell(script: &str) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        // By full path: a bare name would also search the folder chyk.exe sits in.
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        let ps = Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let out = Command::new(ps)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            return Ok(());
        }
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("Add-AppxPackage failed")
            .trim()
            .into())
    }

    /// The bundle folder, when this build carries the DLL.
    fn dir() -> Option<std::path::PathBuf> {
        #[cfg(shell_ext)]
        return Some(crate::bundle::dir());
        #[cfg(not(shell_ext))]
        None
    }

    /// Add the package for `exe` titled `label`, or remove it. Slow (PowerShell): call off the UI thread.
    pub fn set(on: bool, exe: &Path, label: &str) -> Win11 {
        let settings = OsStr::new(SETTINGS);
        if !on {
            let _ = win::delete(settings);
            return match powershell(&script(None)) {
                Ok(()) => Win11::Removed,
                Err(e) => Win11::Failed(e),
            };
        }
        let Some(dir) = dir().filter(|_| windows_11()) else {
            return Win11::Unavailable;
        };
        if !dev_mode() {
            return Win11::NeedsDevMode;
        }
        let files = [
            (
                "AppxManifest.xml",
                manifest(env!("CARGO_PKG_VERSION"), exts()).into_bytes(),
            ),
            ("chyk.png", include_bytes!("../../assets/chyk.png").to_vec()),
        ];
        for (name, data) in files {
            if let Err(e) = std::fs::write(dir.join(name), data) {
                return Win11::Failed(e.to_string());
            }
        }
        // The DLL reads these; `Dir` marks the package current, so it's written only once it registers.
        let values = [("Exe", exe.as_os_str()), ("Label", label.as_ref())];
        if let Some(Err(e)) = values
            .iter()
            .map(|(n, v)| win::put(settings, n, v))
            .find(Result::is_err)
        {
            return Win11::Failed(e);
        }
        match powershell(&script(Some(&dir))) {
            Ok(()) => match win::put(settings, "Dir", dir.as_os_str()) {
                Ok(()) => Win11::Added,
                Err(e) => Win11::Failed(e),
            },
            Err(e) => Win11::Failed(e),
        }
    }

    /// On launch: a package registered from another build's folder (Chyk updated or moved) is
    /// registered again from this one, in the background.
    pub fn refresh(exe: &Path, label: &str) {
        let Some(old) = win::get(OsStr::new(SETTINGS), "Dir") else {
            return;
        };
        let stale = dir().is_none_or(|d| d.as_os_str() != old)
            || win::get(OsStr::new(SETTINGS), "Exe").is_none_or(|e| e != exe.as_os_str());
        if stale {
            let (exe, label) = (exe.to_path_buf(), label.to_string());
            std::thread::spawn(move || set(true, &exe, &label));
        }
    }
}
