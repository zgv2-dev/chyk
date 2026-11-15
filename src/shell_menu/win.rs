//! The registry calls behind [`super`]: HKCU, or HKLM for an all-users install.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
    RegDeleteTreeW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows::core::PCWSTR;

/// NUL-terminated UTF-16.
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain([0]).collect()
}

/// The hive for the user's own entry, or for all users.
pub(super) fn hive(all_users: bool) -> HKEY {
    if all_users {
        HKEY_LOCAL_MACHINE
    } else {
        HKEY_CURRENT_USER
    }
}

/// Writes a string value under HKCU, creating the key if needed.
pub(super) fn put(key: &OsStr, name: &str, data: &OsStr) -> Result<(), String> {
    put_in(HKEY_CURRENT_USER, key, name, data)
}

pub(super) fn put_in(hive: HKEY, key: &OsStr, name: &str, data: &OsStr) -> Result<(), String> {
    let (key, name, data) = (wide(key), wide(name.as_ref()), wide(data));
    // SAFETY: all three buffers are NUL-terminated and outlive the call; the size is in bytes.
    let r = unsafe {
        RegSetKeyValueW(
            hive,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
    };
    (r == ERROR_SUCCESS)
        .then_some(())
        .ok_or_else(|| r.to_hresult().message())
}

/// A string value under HKCU, or `None` if the key or value is missing.
pub(super) fn get(key: &OsStr, name: &str) -> Option<OsString> {
    get_in(HKEY_CURRENT_USER, key, name)
}

/// A string value under HKLM.
pub(super) fn get_lm(key: &str, name: &str) -> Option<OsString> {
    get_in(HKEY_LOCAL_MACHINE, key.as_ref(), name)
}

/// A string value under HKLM, by an `OsStr` key.
pub(super) fn get_lm_os(key: &OsStr, name: &str) -> Option<OsString> {
    get_in(HKEY_LOCAL_MACHINE, key, name)
}

/// A DWORD value under HKLM.
pub(super) fn dword_lm(key: &str, name: &str) -> Option<u32> {
    let (key, name) = (wide(key.as_ref()), wide(name.as_ref()));
    let mut v = 0u32;
    let mut size = 4u32;
    // SAFETY: `v` holds `size` bytes; the names are NUL-terminated.
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut v).cast()),
            Some(&mut size),
        )
    };
    (r == ERROR_SUCCESS).then_some(v)
}

fn get_in(hive: HKEY, key: &OsStr, name: &str) -> Option<OsString> {
    let (key, name) = (wide(key), wide(name.as_ref()));
    let mut buf = vec![0u16; 1024];
    let mut size;
    loop {
        size = (buf.len() * 2) as u32;
        // SAFETY: `buf` holds `size` bytes; the names are NUL-terminated.
        let r = unsafe {
            RegGetValueW(
                hive,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        match r {
            ERROR_SUCCESS => break,
            // A long (\\?\) path: `size` now holds the bytes needed. Retry with room for it.
            ERROR_MORE_DATA => buf.resize((size as usize).div_ceil(2) + 1, 0),
            _ => return None,
        }
    }
    // `size` counts the terminating NUL.
    buf.truncate((size as usize / 2).saturating_sub(1));
    Some(OsString::from_wide(&buf))
}

/// Deletes the key under HKCU and everything under it. A missing key is fine.
pub(super) fn delete(key: &OsStr) -> Result<(), String> {
    delete_in(HKEY_CURRENT_USER, key)
}

pub(super) fn delete_in(hive: HKEY, key: &OsStr) -> Result<(), String> {
    let key = wide(key);
    // SAFETY: `key` is NUL-terminated and outlives the call.
    let r = unsafe { RegDeleteTreeW(hive, PCWSTR(key.as_ptr())) };
    match r {
        ERROR_SUCCESS => Ok(()),
        e if e.0 == 2 => Ok(()), // ERROR_FILE_NOT_FOUND
        e => Err(e.to_hresult().message()),
    }
}

/// Tells Explorer the associations changed, so the menu updates without a restart.
pub(super) fn notify() {
    // SAFETY: SHCNE_ASSOCCHANGED takes no items.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}
