//! Show a file in the OS file manager.
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};

/// Program and arguments that show `path` in the file manager of `os` (`std::env::consts::OS`).
pub fn reveal_command(path: &Path, os: &str) -> (String, Vec<OsString>) {
    match os {
        "macos" => ("open".into(), vec!["-R".into(), path.into()]),
        "windows" => {
            let mut arg = OsString::from("/select,");
            arg.push(path);
            ("explorer".into(), vec![arg])
        }
        // shortcut: Linux has no portable "select"; opens the folder, upgrade via the FileManager1 D-Bus call.
        _ => (
            "xdg-open".into(),
            vec![path.parent().unwrap_or(path).into()],
        ),
    }
}

/// Spawns the file manager; never waits, a failure to start is ignored.
pub fn reveal(path: &Path) {
    let (prog, args) = reveal_command(path, std::env::consts::OS);
    let mut cmd = Command::new(prog);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
    if let Ok(mut child) = cmd.spawn() {
        // Reap it off the UI thread so it doesn't linger as a zombie.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_selects_the_file() {
        let (p, a) = reveal_command(Path::new("/v/a.mp4"), "macos");
        assert_eq!(
            (p.as_str(), a),
            ("open", vec!["-R".into(), "/v/a.mp4".into()])
        );
    }

    #[test]
    fn windows_selects_the_file() {
        let (p, a) = reveal_command(Path::new("C:\\v\\a.mp4"), "windows");
        assert_eq!(
            (p.as_str(), a),
            ("explorer", vec!["/select,C:\\v\\a.mp4".into()])
        );
    }

    #[test]
    fn linux_opens_the_folder() {
        let (p, a) = reveal_command(Path::new("/v/a.mp4"), "linux");
        assert_eq!((p.as_str(), a), ("xdg-open", vec!["/v".into()]));
    }
}
