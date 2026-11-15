//! The part file an export writes before it replaces the destination.

use super::*;

/// The file an export writes before it is renamed to `dst`: hidden, in the same folder (so the
/// rename never crosses drives), with the same extension (ffmpeg picks the container by it).
pub(crate) fn path(dst: &Path) -> PathBuf {
    let stem = dst.file_stem().unwrap_or_default().to_string_lossy();
    let pid = std::process::id();
    let name = match dst.extension() {
        Some(e) => format!(".{stem}.{pid}.part.{}", e.to_string_lossy()),
        None => format!(".{stem}.{pid}.part"),
    };
    dst.with_file_name(name)
}

/// Create the part file for `dst`, refusing one that already exists. When it will replace a
/// file, it takes that file's access first (mode, group and ACL, or DACL), before ffmpeg writes a
/// byte, so replacing a private file never exposes the export. If that fails, so does the export.
pub(super) fn claim(part: &Path, dst: &Path) -> Result<(), String> {
    // Only a missing destination means there is no access to keep.
    let old = match std::fs::metadata(dst) {
        Ok(m) => Some(m),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(t!("error.save", error = e)),
    };
    #[cfg(windows)]
    if old.is_some() {
        // Created with the destination's DACL; CREATE_NEW leaves nothing behind on failure.
        return win::create(part, dst)
            .map(drop)
            .map_err(|e| t!("error.save", error = e));
    }
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    if old.is_some() {
        // Owner-only until the destination's access is applied.
        std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    }
    let f = open.open(part).map_err(|e| t!("error.save", error = e))?;
    let Some(old) = old else { return Ok(()) };
    let kept = take_access(&f, &old, dst, part);
    drop(f);
    // This call created the part, so removing it never touches a file it found.
    kept.map_err(|e| {
        let _ = std::fs::remove_file(part);
        t!("error.save", error = e)
    })
}

#[cfg(unix)]
fn take_access(
    f: &std::fs::File,
    old: &std::fs::Metadata,
    _dst: &Path,
    _: &Path,
) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    std::os::unix::fs::fchown(f, None, Some(old.gid()))?;
    // ACL first: the part is still 0600, so an inherited ACL is masked off until it is gone.
    #[cfg(target_os = "linux")]
    acl::copy(f, _dst)?;
    f.set_permissions(old.permissions())
}

/// The POSIX access ACL: its group bits are only the mask, so mode and group alone can widen it.
#[cfg(target_os = "linux")]
mod acl;

#[cfg(windows)]
mod win;

#[cfg(not(unix))]
fn take_access(
    _: &std::fs::File,
    _: &std::fs::Metadata,
    _: &Path,
    _: &Path,
) -> std::io::Result<()> {
    Ok(())
}
