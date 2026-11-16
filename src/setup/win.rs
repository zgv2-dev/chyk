//! The Windows calls behind setup: registry values (HKCU or HKLM), shortcuts, the Desktop
//! folders, and starting a program elevated.

use super::{Scope, Value};
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HWND, WIN32_ERROR,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, IPersistFile,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_DWORD, REG_SZ, RRF_RT_REG_SZ,
    RegDeleteKeyValueW, RegDeleteTreeW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop,
    IShellLinkW, IsUserAnAdmin, KF_FLAG_DEFAULT, SEE_MASK_NOCLOSEPROCESS, SHCNE_ASSOCCHANGED,
    SHCNF_IDLIST, SHChangeNotify, SHELLEXECUTEINFOW, SHGetKnownFolderPath, ShellExecuteExW,
    ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{Interface, PCWSTR};

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain([0]).collect()
}

fn hive(scope: Scope) -> HKEY {
    match scope {
        Scope::User => HKEY_CURRENT_USER,
        Scope::Machine => HKEY_LOCAL_MACHINE,
    }
}

fn check(r: WIN32_ERROR) -> Result<(), String> {
    (r == ERROR_SUCCESS)
        .then_some(())
        .ok_or_else(|| r.to_hresult().message())
}

/// Writes a value, creating the key if needed.
pub fn put(scope: Scope, key: &str, name: &str, value: &Value) -> Result<(), String> {
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
            hive(scope),
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            kind.0,
            Some(bytes.as_ptr().cast()),
            bytes.len() as u32,
        )
    })
}

/// A string value, or `None` if the key or value is missing.
pub fn get(scope: Scope, key: &str, name: &str) -> Option<OsString> {
    let (k, n) = (wide(key.as_ref()), wide(name.as_ref()));
    let mut buf = vec![0u16; 2048];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `buf` holds `size` bytes; the names are NUL-terminated.
    let r = unsafe {
        RegGetValueW(
            hive(scope),
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
pub fn delete_tree(scope: Scope, key: &str) -> Result<(), String> {
    let k = wide(key.as_ref());
    // SAFETY: NUL-terminated key name.
    let r = unsafe { RegDeleteTreeW(hive(scope), PCWSTR(k.as_ptr())) };
    if r == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        check(r)
    }
}

/// Deletes one value; a missing one is fine.
pub fn delete_value(scope: Scope, key: &str, name: &str) -> Result<(), String> {
    let (k, n) = (wide(key.as_ref()), wide(name.as_ref()));
    // SAFETY: NUL-terminated names.
    let r = unsafe { RegDeleteKeyValueW(hive(scope), PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr())) };
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

/// The Desktop folder: the user's (it can be moved, e.g. into OneDrive) or the public one.
pub fn desktop(scope: Scope) -> Result<PathBuf, String> {
    known_folder(match scope {
        Scope::User => &FOLDERID_Desktop,
        Scope::Machine => &FOLDERID_PublicDesktop,
    })
}

/// The Start menu's Programs folder: the user's or the all-users one.
pub fn programs(scope: Scope) -> Result<PathBuf, String> {
    known_folder(match scope {
        Scope::User => &FOLDERID_Programs,
        Scope::Machine => &FOLDERID_CommonPrograms,
    })
}

fn known_folder(id: &windows::core::GUID) -> Result<PathBuf, String> {
    // SAFETY: the returned string is copied, then freed with CoTaskMemFree as documented.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(|e| e.message())?;
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

/// This process runs with administrator rights.
pub fn elevated() -> bool {
    // SAFETY: no arguments.
    unsafe { IsUserAnAdmin() }.as_bool()
}

/// Starts `exe` with the command line `params`, through the UAC prompt if `elevate`. With
/// `wait`, waits for it and returns its exit code. Fails if the prompt is declined.
pub fn shell_execute(
    exe: &Path,
    params: &OsStr,
    elevate: bool,
    wait: bool,
) -> std::io::Result<Option<u32>> {
    let (file, params) = (wide(exe.as_os_str()), wide(params));
    let verb = wide(if elevate { "runas" } else { "open" }.as_ref());
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: if wait { SEE_MASK_NOCLOSEPROCESS } else { 0 },
        hwnd: HWND::default(),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: every string is NUL-terminated and outlives the call; `info` is sized correctly.
    // The process handle is closed after the wait.
    unsafe {
        ShellExecuteExW(&mut info)
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0 & 0xFFFF))?;
        if !wait || info.hProcess.is_invalid() {
            return Ok(None);
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        let got = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        got.map_err(|e| std::io::Error::other(e.message()))?;
        Ok(Some(code))
    }
}
