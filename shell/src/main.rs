//! The sparse package needs an app exe; Windows never starts it for the menu. Run by hand, it
//! opens its arguments in Chyk like the menu entry does.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if let Some(exe) = chyk_shell_exe() {
        let _ = std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .spawn();
    }
}

/// The Chyk exe that registered the menu, read like the DLL does (`lib.rs`).
#[cfg(windows)]
fn chyk_shell_exe() -> Option<std::ffi::OsString> {
    let out = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Chyk\Menu", "/v", "Exe"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.trim_start().starts_with("Exe"))?;
    Some(line.split("REG_SZ").nth(1)?.trim().into())
}
