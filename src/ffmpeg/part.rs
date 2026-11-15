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
    let old = std::fs::metadata(dst).ok();
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
    f.set_permissions(old.permissions())?;
    #[cfg(target_os = "linux")]
    acl::copy(f, _dst)?;
    Ok(())
}

/// The POSIX access ACL: its group bits are only the mask, so mode and group alone can widen it.
#[cfg(target_os = "linux")]
mod acl;

#[cfg(windows)]
fn take_access(
    _: &std::fs::File,
    _: &std::fs::Metadata,
    dst: &Path,
    part: &Path,
) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
    };
    use windows::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
        UNPROTECTED_DACL_SECURITY_INFORMATION,
    };
    use windows::core::PCWSTR;
    let wide = |p: &Path| p.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();
    let (d, p) = (wide(dst), wide(part));
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: both names are NUL-terminated and outlive the calls; `sd` owns `dacl` and is
    // freed once, after its DACL has been applied.
    unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(d.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&raw mut dacl),
            None,
            &mut sd,
        )
        .ok()?;
        let (mut control, mut rev) = (0u16, 0u32);
        let protected = GetSecurityDescriptorControl(sd, &mut control, &mut rev).is_ok()
            && control & SE_DACL_PROTECTED.0 != 0;
        let inherit = if protected {
            PROTECTED_DACL_SECURITY_INFORMATION
        } else {
            UNPROTECTED_DACL_SECURITY_INFORMATION
        };
        let set = SetNamedSecurityInfoW(
            PCWSTR(p.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | inherit,
            None,
            None,
            Some(dacl.cast_const()),
            None,
        );
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        set.ok()?;
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn take_access(
    _: &std::fs::File,
    _: &std::fs::Metadata,
    _: &Path,
    _: &Path,
) -> std::io::Result<()> {
    Ok(())
}
