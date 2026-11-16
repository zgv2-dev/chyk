//! Windows side of temp folders: holding the folder open so its path can't be swapped.

use std::fs::File;
use std::path::{Path, PathBuf};

/// Opens `dir` itself, not what a link points to, without letting anyone delete or rename it
/// while open. Windows also refuses to rename a folder with an open handle anywhere below it,
/// so the whole path stays put until the temp file is gone.
pub(super) fn pin(dir: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let f = std::fs::OpenOptions::new()
        .read(true)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE).0)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .open(dir)?;
    let m = f.metadata()?;
    if !m.is_dir() || m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    Ok(f)
}

pub(super) fn hide(dir: &Path, m: &std::fs::Metadata) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::MetadataExt;
    use windows::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
    };
    let attrs = m.file_attributes();
    if attrs & FILE_ATTRIBUTE_HIDDEN.0 != 0 {
        return Ok(());
    }
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    let all = FILE_FLAGS_AND_ATTRIBUTES(attrs | FILE_ATTRIBUTE_HIDDEN.0);
    // SAFETY: `wide` is NUL-terminated and outlives the call.
    unsafe { SetFileAttributesW(windows::core::PCWSTR(wide.as_ptr()), all) }.map_err(Into::into)
}

/// The pinned folder's own path, with every junction or link on the way resolved. Each folder in
/// it is real and above an open handle, so none can be renamed, unlike a junction on the given
/// path. Drive paths lose the `\\?\` prefix, which ffmpeg and the user needn't see.
pub(super) fn real(pin: &File) -> std::io::Result<PathBuf> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{GetFinalPathNameByHandleW, VOLUME_NAME_DOS};
    let h = HANDLE(pin.as_raw_handle());
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: `h` is an open handle and `buf` a valid buffer for the call.
        let n = unsafe { GetFinalPathNameByHandleW(h, &mut buf, VOLUME_NAME_DOS) } as usize;
        if n == 0 {
            return Err(std::io::Error::last_os_error());
        }
        if n < buf.len() {
            return Ok(plain(&buf[..n]));
        }
        buf.resize(n + 1, 0);
    }
}

/// Kept as UTF-16 throughout: a lossy conversion could name a different folder.
fn plain(p: &[u16]) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
    let p = match p.strip_prefix(&verbatim[..]) {
        Some(rest) if rest.get(1..3) == Some(&[b':'.into(), b'\\'.into()][..]) => rest,
        _ => p,
    };
    std::ffi::OsString::from_wide(p).into()
}
