//! Replacing a file keeps its POSIX ACL. Needs `ffmpeg` and a filesystem with ACLs.

use super::fixture::{Fixture, wait};
use super::*;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;

const NAME: &std::ffi::CStr = c"system.posix_acl_access";

/// `u::rw-,u:65534:r--,g::---,m::r--,o::---`: mode 0640, yet the owning group can't read it.
fn acl() -> Vec<u8> {
    let mut b = 2u32.to_le_bytes().to_vec();
    for (tag, perm, id) in [
        (1u16, 6u16, u32::MAX),
        (2, 4, 65534),
        (4, 0, u32::MAX),
        (0x10, 4, u32::MAX),
        (0x20, 0, u32::MAX),
    ] {
        b.extend(
            tag.to_le_bytes()
                .into_iter()
                .chain(perm.to_le_bytes())
                .chain(id.to_le_bytes()),
        );
    }
    b
}

fn get(p: &Path) -> Option<Vec<u8>> {
    let p = CString::new(p.as_os_str().as_bytes()).unwrap();
    let mut buf = vec![0u8; 256];
    // SAFETY: NUL-terminated strings, and `buf` holds `buf.len()` bytes.
    let n = unsafe {
        libc::getxattr(
            p.as_ptr(),
            NAME.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    (n >= 0).then(|| buf[..n as usize].to_vec())
}

/// Set ACL `name` on `p`; false when this filesystem has no ACLs.
fn set(p: &Path, name: &std::ffi::CStr, acl: &[u8]) -> bool {
    let p = CString::new(p.as_os_str().as_bytes()).unwrap();
    // SAFETY: NUL-terminated strings, and `acl` holds `acl.len()` bytes.
    let r = unsafe { libc::setxattr(p.as_ptr(), name.as_ptr(), acl.as_ptr().cast(), acl.len(), 0) };
    if r != 0 {
        eprintln!(
            "skipped: no ACLs here ({})",
            std::io::Error::last_os_error()
        );
    }
    r == 0
}

#[test]
fn replacing_a_file_keeps_its_acl() {
    let f = Fixture::new("acl_dst");
    let job = f.job("acl.mp4");
    std::fs::write(&job.dst, b"old").unwrap();
    if !set(&job.dst, NAME, &acl()) {
        return;
    }
    wait(export(job.clone(), Context::default())).unwrap();
    assert_eq!(get(&job.dst), Some(acl()));
}

/// A file with no ACL in a folder that gained a default ACL after it was made stays ACL-free.
#[test]
fn replacing_a_file_drops_an_inherited_acl() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new("acl_inherit");
    let job = f.job("plain.mp4");
    std::fs::write(&job.dst, b"old").unwrap();
    std::fs::set_permissions(&job.dst, std::fs::Permissions::from_mode(0o640)).unwrap();
    if !set(
        job.dst.parent().unwrap(),
        c"system.posix_acl_default",
        &acl(),
    ) {
        return;
    }
    wait(export(job.clone(), Context::default())).unwrap();
    assert_eq!(get(&job.dst), None);
    let mode = std::fs::metadata(&job.dst).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o640);
}
