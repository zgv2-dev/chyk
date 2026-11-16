//! Windows 11: round the corners of Chyk's undecorated windows. Windows rounds only windows
//! with a native frame by default, so without this they stay square with a hard edge.

use std::collections::HashSet;
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::EnumThreadWindows;
use windows::core::BOOL;

/// Windows already asked to round, so each gets the call once.
static ROUNDED: Mutex<Option<HashSet<isize>>> = Mutex::new(None);

/// Round every top-level window of this (the UI) thread that isn't yet. Every viewport runs it
/// on its passes: the settings and log windows get a new window each time they open.
pub fn round_corners() {
    unsafe extern "system" fn each(hwnd: HWND, _: LPARAM) -> BOOL {
        let mut set = ROUNDED.lock().unwrap_or_else(|e| e.into_inner());
        if set.get_or_insert_default().insert(hwnd.0 as isize) {
            let pref = DWMWCP_ROUND;
            // SAFETY: a window of this thread, and a value of the attribute's type and size.
            // Before Windows 11 the attribute doesn't exist and the call just fails.
            let _ = unsafe {
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    (&raw const pref).cast(),
                    size_of_val(&pref) as u32,
                )
            };
        }
        true.into()
    }
    // SAFETY: the callback only reads its arguments and touches `ROUNDED`.
    unsafe {
        let _ = EnumThreadWindows(GetCurrentThreadId(), Some(each), LPARAM(0));
    }
}
