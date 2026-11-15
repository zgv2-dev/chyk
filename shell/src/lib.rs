//! "Edit with Chyk" in the Windows 11 Explorer menu. Explorer loads this DLL (through dllhost, as
//! the sparse package's COM surrogate) and asks it for the entry; invoking it starts Chyk with
//! the file. Chyk writes its exe path and the localized title to `HKCU\Software\Chyk\Menu` when
//! it registers the package (`src/shell_menu/package.rs`).
#![cfg(windows)]

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_NOTIMPL, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{IBindCtx, IClassFactory, IClassFactory_Impl};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::UI::Shell::{
    ECF_DEFAULT, ECS_ENABLED, IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl,
    IShellItemArray, SHStrDupW, SIGDN_FILESYSPATH,
};
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, Ref, implement};

/// Must match `CLSID` in `src/shell_menu/package.rs`.
const CLSID: GUID = GUID::from_u128(0x010d3239_a0f9_413b_b13e_80de0d5aa0f9);
/// `LockServer` locks, and live `Command`/`Factory` objects: the DLL stays loaded while any exist.
static LOCKS: AtomicUsize = AtomicUsize::new(0);
static OBJECTS: AtomicUsize = AtomicUsize::new(0);

/// Counts an object in [`OBJECTS`] from creation until it drops.
struct Live;

impl Live {
    fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::Relaxed);
        Self
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::Relaxed);
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// A value under `HKCU\Software\Chyk\Menu`.
fn setting(name: &str) -> Option<OsString> {
    let (key, name) = (wide(r"Software\Chyk\Menu"), wide(name));
    let mut buf = vec![0u16; 4096];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `buf` holds `size` bytes; the names are NUL-terminated.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if r.is_err() {
        return None;
    }
    buf.truncate((size as usize / 2).saturating_sub(1));
    Some(OsString::from_wide(&buf))
}

/// A string Explorer frees with CoTaskMemFree.
fn shell_string(s: &OsString) -> windows::core::Result<PWSTR> {
    let w: Vec<u16> = s.encode_wide().chain([0]).collect();
    // SAFETY: `w` is NUL-terminated.
    unsafe { SHStrDupW(PCWSTR(w.as_ptr())) }
}

#[implement(IExplorerCommand)]
struct Command(#[allow(dead_code)] Live);

impl IExplorerCommand_Impl for Command_Impl {
    fn GetTitle(&self, _: Ref<IShellItemArray>) -> windows::core::Result<PWSTR> {
        shell_string(&setting("Label").unwrap_or_else(|| "Edit with Chyk".into()))
    }
    fn GetIcon(&self, _: Ref<IShellItemArray>) -> windows::core::Result<PWSTR> {
        let mut icon = setting("Exe").ok_or(E_FAIL)?;
        icon.push(",0");
        shell_string(&icon)
    }
    fn GetToolTip(&self, _: Ref<IShellItemArray>) -> windows::core::Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }
    fn GetCanonicalName(&self) -> windows::core::Result<GUID> {
        Ok(CLSID)
    }
    fn GetState(&self, items: Ref<IShellItemArray>, _: BOOL) -> windows::core::Result<u32> {
        // One video at a time, like the classic entry (MultiSelectModel = Single).
        let one = items
            .ok()
            .and_then(|a| unsafe { a.GetCount() })
            .is_ok_and(|n| n == 1);
        Ok(if one { ECS_ENABLED.0 as u32 } else { 0x2 }) // ECS_HIDDEN
    }
    fn Invoke(&self, items: Ref<IShellItemArray>, _: Ref<IBindCtx>) -> windows::core::Result<()> {
        let exe = setting("Exe").ok_or(E_FAIL)?;
        // SAFETY: plain COM calls on Explorer's item array; the path is freed below.
        let path = unsafe {
            let item = items.ok()?.GetItemAt(0)?;
            let p = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let s = OsString::from_wide(p.as_wide());
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const c_void));
            s
        };
        std::process::Command::new(exe)
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|_| E_FAIL.into())
    }
    fn GetFlags(&self) -> windows::core::Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }
    fn EnumSubCommands(&self) -> windows::core::Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IClassFactory)]
struct Factory(#[allow(dead_code)] Live);

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let cmd: IExplorerCommand = Command(Live::new()).into();
        // SAFETY: COM contract: `riid` and `out` are valid for this call.
        unsafe { cmd.query(riid, out).ok() }
    }
    fn LockServer(&self, lock: BOOL) -> windows::core::Result<()> {
        if lock.as_bool() {
            LOCKS.fetch_add(1, Ordering::Relaxed);
        } else {
            LOCKS.fetch_sub(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

/// COM entry point: the factory for [`CLSID`].
///
/// # Safety
/// Called by COM with valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    // SAFETY: COM passes valid pointers.
    unsafe {
        *out = std::ptr::null_mut();
        if *clsid != CLSID {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory(Live::new()).into();
        factory.query(riid, out)
    }
}

/// COM asks whether the DLL may be unloaded. dllhost exits soon after anyway.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LOCKS.load(Ordering::Relaxed) == 0 && OBJECTS.load(Ordering::Relaxed) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}
