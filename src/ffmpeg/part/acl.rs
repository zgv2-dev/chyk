//! Copy a file's POSIX access ACL, so the part grants exactly what the destination did.

use std::ffi::CStr;
use std::io::{Error, Result};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

const NAME: &CStr = c"system.posix_acl_access";

/// Give `f` the access ACL of `dst`. With none on `dst`, drop any the part inherited from a
/// default ACL on the folder. Runs before the mode is set, so the part never grants more meanwhile.
pub(super) fn copy(f: &std::fs::File, dst: &Path) -> Result<()> {
    let Some(acl) = read(dst)? else {
        // SAFETY: valid fd and NUL-terminated name.
        let r = unsafe { libc::fremovexattr(f.as_raw_fd(), NAME.as_ptr()) };
        return absent_ok(r);
    };
    // SAFETY: valid fd, NUL-terminated name, and `acl` holds `acl.len()` bytes.
    let r = unsafe {
        libc::fsetxattr(
            f.as_raw_fd(),
            NAME.as_ptr(),
            acl.as_ptr().cast(),
            acl.len(),
            0,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

/// The raw ACL of `dst`, or `None` when it has none or its filesystem has no ACLs.
fn read(dst: &Path) -> Result<Option<Vec<u8>>> {
    let path = std::ffi::CString::new(dst.as_os_str().as_bytes()).map_err(Error::other)?;
    loop {
        // SAFETY: NUL-terminated strings; a null buffer of size 0 asks for the size.
        let n = unsafe { libc::getxattr(path.as_ptr(), NAME.as_ptr(), std::ptr::null_mut(), 0) };
        if n < 0 {
            return absent_ok(-1).map(|()| None);
        }
        let mut buf = vec![0u8; n as usize];
        // SAFETY: `buf` holds `buf.len()` writable bytes.
        let m = unsafe {
            libc::getxattr(
                path.as_ptr(),
                NAME.as_ptr(),
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        match m {
            0.. => {
                buf.truncate(m as usize);
                return Ok(Some(buf));
            }
            // It grew between the two calls; ask again.
            _ if Error::last_os_error().raw_os_error() == Some(libc::ERANGE) => continue,
            _ => return absent_ok(-1).map(|()| None),
        }
    }
}

/// Success, or an error that only says there is no ACL here.
fn absent_ok(r: libc::c_int) -> Result<()> {
    if r == 0 {
        return Ok(());
    }
    let e = Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::ENODATA | libc::EOPNOTSUPP) => Ok(()),
        _ => Err(e),
    }
}
