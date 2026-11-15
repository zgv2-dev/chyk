//! Create the part with the destination's DACL from the start, so there's no moment when the
//! folder's inherited access applies to it.

use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use windows::Win32::Foundation::{GENERIC_WRITE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows::Win32::Security::{
    DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
    SetSecurityDescriptorControl,
};
use windows::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE,
};
use windows::core::PCWSTR;

/// Create `part` (failing if anything is there) carrying exactly the DACL of `dst`. It is marked
/// protected, so the folder adds nothing: `dst` may have been moved in from a stricter folder.
pub(super) fn create(part: &Path, dst: &Path) -> std::io::Result<std::fs::File> {
    let wide = |p: &Path| p.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();
    let (d, p) = (wide(dst), wide(part));
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: both names are NUL-terminated and outlive the calls; `sd` is freed once, after
    // CreateFileW has copied it, and the returned handle is owned by the File alone.
    unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(d.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            &mut sd,
        )
        .ok()?;
        if let Err(e) = SetSecurityDescriptorControl(sd, SE_DACL_PROTECTED, SE_DACL_PROTECTED) {
            let _ = LocalFree(Some(HLOCAL(sd.0)));
            return Err(e.into());
        }
        let attrs = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: false.into(),
        };
        let h = CreateFileW(
            PCWSTR(p.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            Some(&raw const attrs),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            None,
        );
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        Ok(std::fs::File::from_raw_handle(h?.0))
    }
}
