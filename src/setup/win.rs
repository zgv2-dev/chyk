//! The Windows calls behind setup: HKCU registry values, shortcuts, the Desktop folder.

use super::Value;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, IPersistFile,
};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegDeleteTreeW,
    RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, IShellLinkW, KF_FLAG_DEFAULT, SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
    SHChangeNotify, SHGetKnownFolderPath, ShellLink,
};
use windows::core::{Interface, PCWSTR};

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain([0]).collect()
}

fn check(r: WIN32_ERROR) -> Result<(), String> {
    (r == ERROR_SUCCESS)
        .then_some(())
        .ok_or_else(|| r.to_hresult().message())
}

/// Writes a value under HKCU, creating the key if needed.
pub fn put(key: &str, name: &str, value: &Value) -> Result<(), String> {
    let (k, n) = (wide(key.as_ref()), wide(name.as_ref()));
    let (kind, bytes): (_, Vec<u8>) = match value {
        Value::Str(s) => (
            REG_SZ,
            wide(s).iter().flat_map(|c| c.to_le_bytes()).collect(),
        ),
        Value::Dword(d) => (REG_DWORD, d.to_le_bytes().to_vec()),
    };
    // SAFETY: the names are NUL-terminated and `bytes` outlives the call; the size is in bytes.
    check(unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            kind.0,
            Some(bytes.as_ptr().cast()),
            bytes.len() as u32,
        )
    })
}

/// A string value under HKCU, or `None` if the key or value is missing.
pub fn get(key: &str, name: &str) -> Option<OsString> {
    let (k, n) = (wide(key.as_ref()), wide(name.as_ref()));
    let mut buf = vec![0u16; 2048];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `buf` holds `size` bytes; the names are NUL-terminated.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if r != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1);
    Some(OsString::from_wide(&buf[..len]))
}

/// Deletes a key and everything under it; a missing key is fine.
pub fn delete_tree(key: &str) -> Result<(), String> {
    let k = wide(key.as_ref());
    // SAFETY: NUL-terminated key name.
    let r = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr())) };
    if r == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        check(r)
    }
}

/// Deletes one value; a missing one is fine.
pub fn delete_value(key: &str, name: &str) -> Result<(), String> {
    let (k, n) = (wide(key.as_ref()), wide(name.as_ref()));
    // SAFETY: NUL-terminated names.
    let r =
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr())) };
    if r == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        check(r)
    }
}

/// Tells Explorer that file types changed, so menus and icons update without a restart.
pub fn notify() {
    // SAFETY: no item pointers are passed with SHCNE_ASSOCCHANGED.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

/// The user's Desktop folder (it can be moved, e.g. into OneDrive).
pub fn desktop() -> Result<PathBuf, String> {
    // SAFETY: the returned string is copied, then freed with CoTaskMemFree as documented.
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None)
            .map_err(|e| e.message())?;
        let path = PathBuf::from(OsString::from_wide(p.as_wide()));
        CoTaskMemFree(Some(p.0.cast()));
        Ok(path)
    }
}

/// Writes a shortcut at `lnk` that starts `exe`.
pub fn shortcut(lnk: &Path, exe: &Path) -> Result<(), String> {
    if let Some(dir) = lnk.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let (target, link) = (wide(exe.as_os_str()), wide(lnk.as_os_str()));
    let dir = wide(exe.parent().unwrap_or(exe).as_os_str());
    // SAFETY: COM is initialized on this thread first (a second init returns S_FALSE, which is
    // fine); all strings are NUL-terminated and outlive the calls.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link_obj: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.message())?;
        link_obj
            .SetPath(PCWSTR(target.as_ptr()))
            .map_err(|e| e.message())?;
        link_obj
            .SetWorkingDirectory(PCWSTR(dir.as_ptr()))
            .map_err(|e| e.message())?;
        let file: IPersistFile = link_obj.cast().map_err(|e| e.message())?;
        file.Save(PCWSTR(link.as_ptr()), true)
            .map_err(|e| e.message())
    }
}
