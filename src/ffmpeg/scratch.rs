//! Temp files for a job: in a hidden `.chyk-tmp` folder next to its source video, so footage cut
//! from it never lands in a folder other users share (like `/tmp`). Where that folder can't be
//! used (read-only media, someone else's folder), a private per-user folder instead.

use super::*;
#[cfg(windows)]
#[path = "scratch_win.rs"]
mod win;
use std::collections::HashSet;
use std::fs::File;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, SystemTime};

/// The folder next to a source video. Removed again once empty.
pub(super) const DIR: &str = ".chyk-tmp";
/// Files older than this in a temp folder were left by a run that crashed.
const STALE: Duration = Duration::from_secs(24 * 60 * 60);

/// A new empty file `{name}-…{ext}` for a job on `src`, open for writing and owner-only on
/// Unix. ffmpeg may write over it (`-y` keeps its access); it is deleted on drop.
pub(super) fn create(src: &Path, name: &str, ext: &str) -> std::io::Result<(TempFile, File)> {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let mut file = format!("{name}-{}-{n}", std::process::id());
    if !ext.is_empty() {
        file = format!("{file}.{ext}");
    }
    let near = src.parent().filter(|p| !p.as_os_str().is_empty());
    let homes = near.and_then(trusted).map(|p| p.join(DIR));
    let mut last = std::io::Error::other("no temp folder");
    for dir in homes.into_iter().chain(private()) {
        match create_in(&dir, &file) {
            Ok(r) => return Ok(r),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// `file` in `dir`, making `dir` first. Twice: another run may remove the folder once it empties.
fn create_in(dir: &Path, file: &str) -> std::io::Result<(TempFile, File)> {
    let mut r = Err(std::io::ErrorKind::NotFound.into());
    for _ in 0..2 {
        let (pin, dir) = ensure(dir)?;
        sweep(&dir);
        let path = dir.join(file);
        r = new_file(&path).map(|f| (TempFile(path, pin), f));
        if !matches!(&r, Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
            break;
        }
    }
    r
}

fn new_file(path: &Path) -> std::io::Result<File> {
    let mut open = std::fs::OpenOptions::new();
    // Never through a link or file someone left at that name.
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    open.open(path)
}

/// `dir` with links resolved, if no other user can rename anything on the way to it: each folder
/// on the path is this user's or root's, and others can't write to it (or it is sticky, like
/// `/tmp`). Otherwise they could swap the temp folder for their own after it is checked.
/// On Windows the path is kept as given (canonical paths there are `\\?\` ones), and the folder
/// handle held by `win::pin` keeps it in place instead.
fn trusted(dir: &Path) -> Option<PathBuf> {
    #[cfg(not(unix))]
    let dir = dir.to_path_buf();
    #[cfg(unix)]
    let dir = std::fs::canonicalize(dir).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: as in `private`.
        let me = unsafe { libc::getuid() };
        for d in dir.ancestors() {
            let m = std::fs::symlink_metadata(d).ok()?;
            let owner = m.uid() == me || m.uid() == 0;
            let closed = m.mode() & 0o022 == 0 || m.mode() & 0o1000 != 0;
            if !(owner && closed) {
                return None;
            }
        }
    }
    Some(dir)
}

/// The per-user fallback folder.
fn private() -> Option<PathBuf> {
    #[cfg(unix)]
    // SAFETY: getuid has no preconditions and can't fail.
    let name = format!("chyk-{}", unsafe { libc::getuid() });
    #[cfg(not(unix))]
    let name = "chyk".to_string();
    trusted(&std::env::temp_dir()).map(|t| t.join(name))
}

/// Makes `dir` if needed, and checks it is a real folder only this user can open. On Windows it
/// is hidden, the folder it sits in decides who can read it, and the returned handle pins it; the
/// path to use from then on is the pinned folder's own, with links on the way resolved.
fn ensure(dir: &Path) -> std::io::Result<(Option<File>, PathBuf)> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    match b.create(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Err(e),
        _ => {}
    }
    let m = std::fs::symlink_metadata(dir)?;
    if !m.is_dir() {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: as in `private`.
        if m.uid() != unsafe { libc::getuid() } || m.mode() & 0o077 != 0 {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
    }
    #[cfg(windows)]
    {
        let pin = win::pin(dir)?;
        let real = win::real(&pin)?;
        win::hide(&real, &pin.metadata()?)?;
        Ok((Some(pin), real))
    }
    #[cfg(not(windows))]
    Ok((None, dir.to_path_buf()))
}

/// Once per folder per run: removes what crashed runs left behind. Only ever a folder under a
/// [`trusted`] path, so no one else can point it at other files.
fn sweep(dir: &Path) {
    static SEEN: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if !seen.get_or_insert_default().insert(dir.to_path_buf()) {
        return;
    }
    drop(seen);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| now.duration_since(t).is_ok_and(|age| age > STALE));
        if old && e.file_type().is_ok_and(|t| t.is_file()) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}
