//! Windows builds carry libmpv and ffmpeg inside chyk.exe, deflated (see `build.rs`).
//! - The portable build unpacks them into `data\<id>` next to itself on first run, showing a
//!   small progress window (`app::unpack_window`).
//! - The installer unpacks them into the install folder as one of its steps, and the installed
//!   copy loads them from beside itself. Started from anywhere else it is setup and loads nothing.

use flate2::read::DeflateDecoder;
use std::fs;
use std::io::{self, Read};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Hash of the embedded files, so each portable build unpacks to its own folder.
const ID: &str = env!("CHYK_BUNDLE_ID");
const FILES: &[(&str, &[u8])] = &[
    (
        "libmpv-2.dll",
        include_bytes!(concat!(env!("OUT_DIR"), "/libmpv-2.dll.deflate")),
    ),
    (
        "ffmpeg.exe",
        include_bytes!(concat!(env!("OUT_DIR"), "/ffmpeg.exe.deflate")),
    ),
    // The Windows 11 menu entry: the sparse package registers from this folder (src/shell_menu).
    #[cfg(shell_ext)]
    (
        "chyk_shell.dll",
        include_bytes!(concat!(env!("OUT_DIR"), "/chyk_shell.dll.deflate")),
    ),
    #[cfg(shell_ext)]
    (
        "chyk-shell.exe",
        include_bytes!(concat!(env!("OUT_DIR"), "/chyk-shell.exe.deflate")),
    ),
];

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut std::ffi::c_void;
}

/// Where this copy's libraries are: `data\<id>` for the portable build, else beside the exe.
pub fn dir() -> PathBuf {
    match crate::portable::data_dir() {
        Some(data) => data.join(ID),
        None => crate::portable::exe_dir(),
    }
}

/// The names of the unpacked files, for uninstall.
pub fn names() -> impl Iterator<Item = &'static str> {
    FILES.iter().map(|(name, _)| *name)
}

/// Every file is in [`dir`].
pub fn ready() -> bool {
    let dir = dir();
    names().all(|n| dir.join(n).is_file())
}

/// Counts the compressed bytes read, for progress.
struct Counted<'a> {
    data: &'a [u8],
    read: &'a mut u64,
}

impl Read for Counted<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.data.read(buf)?;
        *self.read += n as u64;
        Ok(n)
    }
}

/// Unpacks every file into `dir`, calling `progress` with the share done (0 to 1). A file that is
/// already there is renamed to `<name>.old` first, since a running Chyk keeps it locked but lets
/// it be renamed; [`clean_aside`] deletes those later.
pub fn unpack(dir: &Path, progress: &mut dyn FnMut(f32)) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let total: u64 = FILES.iter().map(|(_, d)| d.len() as u64).sum();
    let mut read = 0u64;
    for &(name, data) in FILES {
        let path = dir.join(name);
        // Write aside and rename, so a crash never leaves a truncated file behind the real name.
        let tmp = dir.join(format!("{name}.{}.tmp", std::process::id()));
        let mut out = fs::File::create(&tmp)?;
        let mut src = DeflateDecoder::new(Counted {
            data,
            read: &mut read,
        });
        let mut buf = vec![0u8; 1 << 20];
        let copied = loop {
            match src.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(n) => {
                    if let Err(e) = io::Write::write_all(&mut out, &buf[..n]) {
                        break Err(e);
                    }
                    progress(*src.get_ref().read as f32 / total as f32);
                }
                Err(e) => break Err(e),
            }
        };
        drop(out);
        if let Err(e) = copied.and_then(|()| replace(&tmp, &path)) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    }
    progress(1.0);
    Ok(())
}

/// Puts `new` at `path`, moving a file already there to `<path>.old`.
fn replace(new: &Path, path: &Path) -> io::Result<()> {
    if path.exists() {
        let old = aside(path);
        let _ = fs::remove_file(&old);
        fs::rename(path, &old)?;
        if let Err(e) = fs::rename(new, path) {
            // Put the old library back rather than leave none.
            let _ = fs::rename(&old, path);
            return Err(e);
        }
        return Ok(());
    }
    fs::rename(new, path)
}

fn aside(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

/// Deletes the `.old` copies an update left in [`dir`]. One still in use stays for next time.
pub fn clean_aside() {
    let dir = dir();
    for name in names() {
        let _ = fs::remove_file(aside(&dir.join(name)));
    }
}

/// The portable build: deletes other builds' folders in `data`. A build that is still running
/// keeps its files locked; it gets cleaned up next time.
pub fn clean_old_builds() {
    let Some(data) = crate::portable::data_dir() else {
        return;
    };
    if let Ok(builds) = fs::read_dir(data) {
        for b in builds.flatten() {
            if b.file_name().to_str().is_some_and(old_build) {
                let _ = fs::remove_dir_all(b.path());
            }
        }
    }
}

/// Loads libmpv from [`dir`], so its delay-loaded imports resolve.
pub fn load() -> io::Result<()> {
    let dll: Vec<u16> = dir()
        .join(FILES[0].0)
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect();
    // SAFETY: `dll` is a NUL-terminated UTF-16 path.
    if unsafe { LoadLibraryW(dll.as_ptr()) }.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Another build's unpack folder: named by its 16-hex-digit bundle ID, like [`ID`].
fn old_build(name: &str) -> bool {
    name != ID && name.len() == ID.len() && name.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_other_builds_are_old() {
        use super::{ID, old_build};
        assert!(old_build("0123456789abcdef") || ID == "0123456789abcdef");
        assert!(!old_build(ID));
        assert!(!old_build("update"));
        assert!(!old_build("recent"));
        assert!(!old_build("session.txt"));
    }
}
