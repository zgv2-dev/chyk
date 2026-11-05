//! Export feedback outside the window: progress on the app's icon (macOS Dock badge, Windows
//! taskbar bar, Linux launcher bar) and a sound when an export finishes in the background.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::Cell;

/// Shows export progress (0..=1) on the app's icon; `None` clears it. Only reaches the OS when
/// the shown percent changes.
pub fn progress(window: &impl HasWindowHandle, p: Option<f32>) {
    thread_local!(static SHOWN: Cell<Option<u32>> = const { Cell::new(None) });
    let pct = p.map(|p| (p.clamp(0.0, 1.0) * 100.0) as u32);
    if SHOWN.replace(pct) == pct {
        return;
    }
    os::progress(window.window_handle().ok().map(|h| h.as_raw()), pct);
}

/// The system's "done" sound.
pub fn done_sound() {
    os::done_sound();
}

#[cfg(target_os = "macos")]
mod os {
    use super::RawWindowHandle;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSSound};
    use objc2_foundation::NSString;

    /// A "42%" badge on the Dock icon.
    pub fn progress(_: Option<RawWindowHandle>, pct: Option<u32>) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let label = pct.map(|n| NSString::from_str(&format!("{n}%")));
        NSApplication::sharedApplication(mtm)
            .dockTile()
            .setBadgeLabel(label.as_deref());
    }

    pub fn done_sound() {
        if let Some(s) = NSSound::soundNamed(&NSString::from_str("Glass")) {
            s.play();
        }
    }
}

#[cfg(windows)]
mod os {
    use super::RawWindowHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Shell::{ITaskbarList3, TBPF_NOPROGRESS, TaskbarList};
    use windows::core::w;

    thread_local! {
        // SAFETY: COM calls on the UI thread; initializing an already-initialized apartment is a no-op.
        static BAR: Option<ITaskbarList3> = unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
                .ok()
                .filter(|b| b.HrInit().is_ok())
        };
    }

    /// A progress bar across the taskbar button.
    pub fn progress(window: Option<RawWindowHandle>, pct: Option<u32>) {
        let Some(RawWindowHandle::Win32(h)) = window else {
            return;
        };
        let hwnd = HWND(h.hwnd.get() as *mut _);
        BAR.with(|bar| {
            let Some(bar) = bar else { return };
            // SAFETY: `hwnd` is this app's live window; called on its UI thread.
            unsafe {
                let _ = match pct {
                    Some(n) => bar.SetProgressValue(hwnd, n.into(), 100),
                    None => bar.SetProgressState(hwnd, TBPF_NOPROGRESS),
                };
            }
        });
    }

    pub fn done_sound() {
        // SAFETY: a static string; SND_ASYNC returns at once.
        unsafe {
            let _ = PlaySoundW(w!("Notification.Default"), None, SND_ALIAS | SND_ASYNC);
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod os {
    use super::RawWindowHandle;
    use std::collections::HashMap;
    use std::process::{Command, Stdio};
    use zbus::zvariant::Value;

    thread_local!(static BUS: Option<zbus::blocking::Connection> =
        zbus::blocking::Connection::session().ok());

    /// A progress bar on the launcher icon, through the Unity LauncherEntry API that KDE, GNOME's
    /// Dash to Dock / Ubuntu Dock and Plank read. Matched by the installed glint.desktop.
    pub fn progress(_: Option<RawWindowHandle>, pct: Option<u32>) {
        BUS.with(|bus| {
            let Some(bus) = bus else { return };
            let props = HashMap::from([
                ("progress", Value::from(f64::from(pct.unwrap_or(0)) / 100.0)),
                ("progress-visible", Value::from(pct.is_some())),
            ]);
            let _ = bus.emit_signal(
                None::<zbus::names::BusName>,
                "/com/canonical/unity/launcherentry/glint",
                "com.canonical.Unity.LauncherEntry",
                "Update",
                &("application://glint.desktop", props),
            );
        });
    }

    /// The freedesktop "complete" sound, through whichever player the desktop has.
    pub fn done_sound() {
        const SOUND: &str = "/usr/share/sounds/freedesktop/stereo/complete.oga";
        std::thread::spawn(|| {
            for cmd in [
                &["canberra-gtk-play", "-i", "complete"][..],
                &["paplay", SOUND],
                &["pw-play", SOUND],
            ] {
                let played = Command::new(cmd[0])
                    .args(&cmd[1..])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if played {
                    break;
                }
            }
        });
    }
}
