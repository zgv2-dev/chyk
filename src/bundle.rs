//! Windows builds pin libmpv and ffmpeg (and the Windows 11 menu entry's files) by SHA-256
//! and size at build time (see `build.rs`). A file on disk is used only when it matches.
//! - The portable build carries them inside chyk.exe, deflated, and unpacks them into
//!   `Chyk\<id>` next to itself, showing a small progress window (`app::unpack_window`). It checks
//!   them on every launch and unpacks a fresh copy of any that changed.
//! - The installer carries only the menu entry's files. It downloads libmpv and ffmpeg into the
//!   install folder from the `windows-libs` release, keeping a copy already there that matches.
//!   The installed copy loads them from beside itself. Started from anywhere else it is setup and
//!   loads nothing.

use flate2::read::{DeflateDecoder, GzDecoder};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::fs;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Hash of the pinned files, so each portable build unpacks to its own folder.
const ID: &str = env!("CHYK_BUNDLE_ID");
const LIBMPV: &str = "libmpv-2.dll";
/// Where the installer downloads from: one release holds every pinned version, named by hash.
/// `CHYK_LIBS_URL` points elsewhere for testing (a `file:///` folder works); the hash check
/// still applies.
const LIBS_URL: &str = "https://github.com/zgv2-dev/chyk/releases/download/windows-libs";

struct File {
    name: &'static str,
    /// Lowercase hex.
    sha256: &'static str,
    size: u64,
    from: From,
}

// A build may use only one of them: the portable one embeds all, the installer may download all.
#[allow(dead_code)]
enum From {
    /// Deflated into the exe.
    Embedded(&'static [u8]),
    /// Gzipped at `LIBS_URL/<asset>`, `packed` bytes.
    Download { asset: &'static str, packed: u64 },
}

const FILES: &[File] = include!(concat!(env!("OUT_DIR"), "/files.rs"));

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut std::ffi::c_void;
}

/// Where this copy's libraries are: `Chyk\<id>` for the portable build, else beside the exe.
pub fn dir() -> PathBuf {
    match crate::portable::data_dir() {
        Some(data) => data.join(ID),
        None => crate::portable::exe_dir(),
    }
}

/// The names of the files, for uninstall.
pub fn names() -> impl Iterator<Item = &'static str> {
    FILES.iter().map(|f| f.name)
}

/// The files' total size in bytes, unpacked.
pub fn size() -> u64 {
    FILES.iter().map(|f| f.size).sum()
}

/// Every file is in [`dir`]. Cheap: doesn't check the hashes.
pub fn ready() -> bool {
    let dir = dir();
    names().all(|n| dir.join(n).is_file())
}

/// Every file in [`dir`] matches its pinned hash.
pub fn verified() -> bool {
    let dir = dir();
    FILES.iter().all(|f| matches(&dir.join(f.name), f))
}

/// `path` is `f`: same size, same SHA-256.
fn matches(path: &Path, f: &File) -> bool {
    if fs::metadata(path).map_or(true, |m| m.len() != f.size) {
        return false;
    }
    let mut hash = Sha256::new();
    fs::File::open(path)
        .and_then(|mut file| io::copy(&mut file, &mut hash))
        .is_ok_and(|_| hex(&hash.finalize()) == f.sha256)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Puts every file into `dir`, calling `progress` with the share done (0 to 1). A file already
/// there that matches stays; any other is unpacked or downloaded, and checked against its hash
/// before it takes the real name. A file already there is renamed to `<name>.old` first, since a
/// running Chyk keeps it locked but lets it be renamed; [`clean_aside`] deletes those later.
pub fn unpack(dir: &Path, progress: &mut dyn FnMut(f32)) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let todo: Vec<&File> = FILES
        .iter()
        .filter(|f| !matches(&dir.join(f.name), f))
        .collect();
    // Progress counts bytes read: from the exe, or from the network.
    let cost = |f: &File| match f.from {
        From::Embedded(data) => data.len() as u64,
        From::Download { packed, .. } => packed,
    };
    let total = todo.iter().map(|f| cost(f)).sum::<u64>().max(1);
    let mut done = 0u64;
    for f in todo {
        let mut step = |n: u64| progress((done + n) as f32 / total as f32);
        match f.from {
            From::Embedded(data) => {
                let read = Cell::new(0);
                let mut src = DeflateDecoder::new(Counted { data, read: &read });
                place(dir, f, &mut src, &mut || step(read.get()))?;
            }
            From::Download { asset, packed } => {
                let gz = download(dir, asset, packed, &mut step)?;
                let placed = fs::File::open(&gz)
                    .and_then(|file| place(dir, f, &mut GzDecoder::new(file), &mut || {}));
                let _ = fs::remove_file(&gz);
                placed?;
            }
        }
        done += cost(f);
    }
    progress(1.0);
    Ok(())
}

/// Counts the compressed bytes read, for progress.
struct Counted<'a> {
    data: &'a [u8],
    read: &'a Cell<u64>,
}

impl Read for Counted<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.data.read(buf)?;
        self.read.set(self.read.get() + n as u64);
        Ok(n)
    }
}

/// Writes `src` beside `f`'s place, checks it against the pinned hash, then moves it there.
/// Writing aside means a crash never leaves a truncated file behind the real name.
fn place(dir: &Path, f: &File, src: &mut dyn Read, tick: &mut dyn FnMut()) -> io::Result<()> {
    let path = dir.join(f.name);
    let tmp = dir.join(format!("{}.{}.tmp", f.name, std::process::id()));
    let written = (|| {
        let mut out = fs::File::create(&tmp)?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            hash.update(&buf[..n]);
            tick();
        }
        drop(out);
        if hex(&hash.finalize()) != f.sha256 {
            return Err(io::Error::other(format!(
                "{} doesn't match the expected file",
                f.name
            )));
        }
        replace(&tmp, &path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// Downloads `asset` into `dir` with the system curl, reporting the bytes so far.
fn download(
    dir: &Path,
    asset: &str,
    packed: u64,
    step: &mut dyn FnMut(u64),
) -> io::Result<PathBuf> {
    static ATTEMPT: AtomicU32 = AtomicU32::new(0);
    let base = std::env::var(crate::setup::LIBS_URL_ENV).unwrap_or_else(|_| LIBS_URL.into());
    let part = dir.join(format!(
        "{asset}.{}.{}.part",
        std::process::id(),
        ATTEMPT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut child = crate::update::curl()
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&part)
        .arg(format!("{}/{asset}", base.trim_end_matches('/')))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        step(fs::metadata(&part).map_or(0, |m| m.len()).min(packed));
        std::thread::sleep(std::time::Duration::from_millis(150));
    };
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        let _ = fs::remove_file(&part);
        return Err(io::Error::other(match err.trim() {
            "" => format!("curl exited with {status}"),
            e => e.to_owned(),
        }));
    }
    step(packed);
    Ok(part)
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

/// The portable build: deletes other builds' folders in `Chyk`. A build that is still running
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
        .join(LIBMPV)
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
