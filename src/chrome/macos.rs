//! macOS: native traffic lights in the header, sRGB output, and ⌘Q asking first.

/// Tag the window as sRGB, so macOS converts Chyk's colors (and the video) to the display, the
/// way a browser does. Untagged, sRGB values land raw on a wide-gamut (P3) panel and every
/// green comes out oversaturated.
pub fn use_srgb(window: &impl raw_window_handle::HasWindowHandle) {
    use objc2_app_kit::{NSColorSpace, NSView};
    use raw_window_handle::RawWindowHandle;
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
        return;
    };
    // SAFETY: called from eframe's app creation on the main thread; the view is alive.
    let view: &NSView = unsafe { h.ns_view.cast().as_ref() };
    if let Some(window) = view.window() {
        window.setColorSpace(Some(&NSColorSpace::sRGBColorSpace()));
    }
}

/// ⌘Q (the app menu's Quit) terminates the app outright, skipping the window's close request,
/// so unexported edits couldn't be caught. Point it at the key window's `performClose:` instead:
/// it asks like the red button does, and the app exits once its window closes. Runs until the
/// menu exists, then never again.
pub fn route_quit_to_close() {
    use objc2::{MainThreadMarker, sel};
    use objc2_app_kit::NSApplication;
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.load(Ordering::Relaxed) {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(menu) = NSApplication::sharedApplication(mtm).mainMenu() else {
        return;
    };
    // Quit lives in the first (application) menu.
    let Some(app_menu) = menu.itemAtIndex(0).and_then(|item| item.submenu()) else {
        return;
    };
    for i in 0..app_menu.numberOfItems() {
        if let Some(item) = app_menu.itemAtIndex(i)
            && item.action() == Some(sel!(terminate:))
        {
            // SAFETY: main thread; a nil target sends the action down the responder chain, where
            // the key window answers `performClose:`.
            unsafe {
                item.setAction(Some(sel!(performClose:)));
                item.setTarget(None);
            }
        }
    }
    DONE.store(true, Ordering::Relaxed);
}

/// Center the native traffic lights vertically in the `height`-point header. AppKit puts them
/// at a fixed spot in a 28 pt titlebar and resets them on every relayout (resize, fullscreen),
/// so this runs each frame: grow the titlebar container to the header's height, then place the
/// buttons inside it. Same approach as Electron's `trafficLightPosition`.
pub fn place_traffic_lights(window: &impl raw_window_handle::HasWindowHandle, height: f64) {
    use objc2_app_kit::{NSView, NSWindowButton};
    use objc2_foundation::NSPoint;
    use raw_window_handle::RawWindowHandle;
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
        return;
    };
    // SAFETY: eframe calls `ui` on the main thread, and the view lives as long as the window.
    let view: &NSView = unsafe { h.ns_view.cast().as_ref() };
    let Some(window) = view.window() else {
        return;
    };
    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|b| window.standardWindowButton(b));
    let [Some(close), Some(mini), Some(zoom)] = buttons else {
        return;
    };
    // close → NSTitlebarView → NSTitlebarContainerView.
    // SAFETY: main thread (see above); superview is only read while the window is alive.
    let Some(container) = (unsafe { close.superview() }).and_then(|v| unsafe { v.superview() })
    else {
        return;
    };
    // AppKit moves the window on a drag anywhere in the top ~28 pt, before egui sees it, which
    // stole drags from header controls (the strip padding field). The header moves the window
    // through egui instead (`title_bar`, which leaves controls alone).
    if window.isMovable() {
        window.setMovable(false);
    }
    let mut frame = container.frame();
    let window_h = window.frame().size.height;
    if (frame.size.height - height).abs() > 0.5
        || (frame.origin.y - (window_h - height)).abs() > 0.5
    {
        frame.size.height = height;
        frame.origin.y = window_h - height;
        container.setFrame(frame);
    }
    let b = close.frame();
    let gap = mini.frame().origin.x - b.origin.x;
    // AppKit coordinates start at the bottom: center the buttons on the header's middle.
    let y = (height - b.size.height) / 2.0;
    for (i, button) in [close, mini, zoom].iter().enumerate() {
        let x = 14.0 + i as f64 * gap;
        let o = button.frame().origin;
        if (o.x - x).abs() > 0.5 || (o.y - y).abs() > 0.5 {
            button.setFrameOrigin(NSPoint::new(x, y));
        }
    }
}
