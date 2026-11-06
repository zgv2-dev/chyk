//! Putting a downloaded release in place of the running copy, and starting the new copy.
//! A running app can be renamed or replaced on every system, so this works from inside Chyk.

use super::Kind;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;

/// Replace this install with `file`, a finished download of its release file.
pub fn swap(kind: &Kind, file: &Path) -> io::Result<()> {
    match kind {
        Kind::MacApp(bundle) => mac(bundle, file),
        Kind::WindowsExe(exe) => windows(exe, file),
        Kind::AppImage(path) => appimage(path, file),
        Kind::Package => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "package install",
        )),
    }
}

/// Remove what an earlier swap left behind (Windows can't delete a running exe, only rename it).
pub fn cleanup(kind: &Kind) {
    if let Kind::WindowsExe(exe) = kind {
        let _ = std::fs::remove_file(exe.with_extension("old.exe"));
    }
}

/// Start the (new) copy at this install's path with `args`.
pub fn relaunch(kind: &Kind, args: &[OsString]) -> io::Result<()> {
    let mut cmd = match kind {
        Kind::MacApp(bundle) => {
            let mut c = Command::new("open");
            c.arg("-n").arg(bundle).arg("--args");
            c
        }
        Kind::WindowsExe(path) | Kind::AppImage(path) => Command::new(path),
        Kind::Package => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "package install",
            ));
        }
    };
    cmd.args(args).spawn().map(drop)
}

fn run(cmd: &mut Command) -> io::Result<()> {
    let out = cmd.output()?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    Err(io::Error::other(
        err.lines().last().unwrap_or("failed").to_string(),
    ))
}

/// Mount the DMG, check the app in it is intact, then trade bundles by renaming.
fn mac(bundle: &Path, dmg: &Path) -> io::Result<()> {
    let mnt = dmg.with_extension("mnt");
    std::fs::create_dir_all(&mnt)?;
    run(Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
        ])
        .arg(&mnt)
        .arg(dmg))?;
    let result = (|| {
        let src = mnt.join("Chyk.app");
        let new = bundle.with_file_name(".Chyk-update.app");
        let old = bundle.with_file_name(".Chyk-old.app");
        let _ = std::fs::remove_dir_all(&new);
        let _ = std::fs::remove_dir_all(&old);
        run(Command::new("ditto").arg(&src).arg(&new))?;
        if let Err(e) = run(Command::new("codesign")
            .args(["--verify", "--deep"])
            .arg(&new))
        {
            let _ = std::fs::remove_dir_all(&new);
            return Err(e);
        }
        std::fs::rename(bundle, &old)?;
        if let Err(e) = std::fs::rename(&new, bundle) {
            let _ = std::fs::rename(&old, bundle);
            return Err(e);
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(())
    })();
    let _ = run(Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mnt));
    let _ = std::fs::remove_dir(&mnt);
    result
}

/// Rename the running exe aside and move the new one into its place (a rename when both are on
/// one drive, else a copy); undo the first rename on failure.
fn windows(exe: &Path, new: &Path) -> io::Result<()> {
    let old = exe.with_extension("old.exe");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old)?;
    if let Err(e) = std::fs::rename(new, exe).or_else(|_| std::fs::copy(new, exe).map(drop)) {
        let _ = std::fs::remove_file(exe);
        let _ = std::fs::rename(&old, exe);
        return Err(e);
    }
    Ok(())
}

/// Copy next to the AppImage, make it executable, then rename over it in one step.
fn appimage(path: &Path, new: &Path) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".new");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::copy(new, &tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
