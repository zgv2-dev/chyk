//! Window controls inside the header. macOS keeps its native traffic lights (the content view
//! runs under a transparent titlebar). Windows and Linux can't embed native buttons with winit,
//! so the window is undecorated and Glint draws minimize/maximize/close itself, on the side the
//! desktop puts them, and handles edge resizing.

use eframe::egui::{self, CursorIcon, ResizeDirection, ViewportCommand};

/// Whether Glint draws its own window controls and resize edges.
pub const CUSTOM: bool = !cfg!(target_os = "macos");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Minimize,
    Maximize,
    Close,
}

/// Window buttons for the left and right ends of the header, in left-to-right order.
pub struct Layout {
    pub left: Vec<Button>,
    pub right: Vec<Button>,
}

impl Layout {
    pub fn detect() -> Self {
        if !CUSTOM {
            return Self {
                left: vec![],
                right: vec![],
            };
        }
        #[cfg(target_os = "linux")]
        if let Some(l) = linux_layout() {
            return l;
        }
        parse("appmenu:minimize,maximize,close")
    }
}

/// The desktop's button layout: GNOME-style `gsettings`, else KDE's kwinrc.
#[cfg(target_os = "linux")]
fn linux_layout() -> Option<Layout> {
    let gnome = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    if let Some(s) = gnome.filter(|s| s.contains(':')) {
        return Some(parse(s.trim().trim_matches('\'')));
    }
    let home = std::env::var_os("HOME")?;
    let rc = std::fs::read_to_string(std::path::Path::new(&home).join(".config/kwinrc")).ok()?;
    let get = |key: &str| {
        rc.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(str::trim)
    };
    // KDE letters: M = menu, S = on all desktops, H/I/A/X = help, minimize, maximize, close.
    let kde = |s: &str| {
        s.chars()
            .filter_map(|c| match c {
                'I' => Some(Button::Minimize),
                'A' => Some(Button::Maximize),
                'X' => Some(Button::Close),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let (left, right) = (get("ButtonsOnLeft"), get("ButtonsOnRight"));
    (left.is_some() || right.is_some()).then(|| Layout {
        left: kde(left.unwrap_or("")),
        right: kde(right.unwrap_or("")),
    })
}

/// GNOME `button-layout` syntax: `left:right`, comma-separated names.
pub fn parse(s: &str) -> Layout {
    let names = |side: &str| {
        side.split(',')
            .filter_map(|n| match n.trim() {
                "minimize" => Some(Button::Minimize),
                "maximize" => Some(Button::Maximize),
                "close" => Some(Button::Close),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let (left, right) = s.split_once(':').unwrap_or(("", s));
    Layout {
        left: names(left),
        right: names(right),
    }
}

/// Glyph and label for a window button.
pub fn look(b: Button, maximized: bool) -> (&'static str, &'static str) {
    use crate::i18n::t;
    use egui_phosphor::regular as icon;
    match b {
        Button::Minimize => (icon::MINUS, t!("window.minimize")),
        Button::Maximize if maximized => (icon::CORNERS_IN, t!("window.restore")),
        Button::Maximize => (icon::CORNERS_OUT, t!("window.maximize")),
        Button::Close => (icon::X, t!("window.close")),
    }
}

pub fn maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

pub fn press(ctx: &egui::Context, b: Button) {
    ctx.send_viewport_cmd(match b {
        Button::Minimize => ViewportCommand::Minimized(true),
        Button::Maximize => ViewportCommand::Maximized(!maximized(ctx)),
        Button::Close => ViewportCommand::Close,
    });
}

/// Tag the window as sRGB, so macOS converts Glint's colors (and the video) to the display, the
/// way a browser does. Untagged, sRGB values land raw on a wide-gamut (P3) panel and every
/// green comes out oversaturated.
#[cfg(target_os = "macos")]
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

/// Center the native traffic lights vertically in the `height`-point header. AppKit puts them
/// at a fixed spot in a 28 pt titlebar and resets them on every relayout (resize, fullscreen),
/// so this runs each frame: grow the titlebar container to the header's height, then place the
/// buttons inside it. Same approach as Electron's `trafficLightPosition`.
#[cfg(target_os = "macos")]
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

/// Resize from the window edges, which an undecorated window doesn't get from every OS.
pub fn resize_edges(ctx: &egui::Context) {
    if !CUSTOM || maximized(ctx) {
        return;
    }
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let r = ctx.content_rect();
    const M: f32 = 5.0;
    let (w, e, n, s) = (
        p.x - r.min.x < M,
        r.max.x - p.x < M,
        p.y - r.min.y < M,
        r.max.y - p.y < M,
    );
    use ResizeDirection as D;
    let (dir, cursor) = match (w, e, n, s) {
        (true, _, true, _) => (D::NorthWest, CursorIcon::ResizeNwSe),
        (_, true, _, true) => (D::SouthEast, CursorIcon::ResizeNwSe),
        (_, true, true, _) => (D::NorthEast, CursorIcon::ResizeNeSw),
        (true, _, _, true) => (D::SouthWest, CursorIcon::ResizeNeSw),
        (true, ..) => (D::West, CursorIcon::ResizeHorizontal),
        (_, true, ..) => (D::East, CursorIcon::ResizeHorizontal),
        (_, _, true, _) => (D::North, CursorIcon::ResizeVertical),
        (.., true) => (D::South, CursorIcon::ResizeVertical),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Button::*;

    #[test]
    fn parses_gnome_layouts() {
        let l = parse("appmenu:minimize,maximize,close");
        assert_eq!((l.left, l.right), (vec![], vec![Minimize, Maximize, Close]));
        let l = parse("close,minimize,maximize:");
        assert_eq!((l.left, l.right), (vec![Close, Minimize, Maximize], vec![]));
        let l = parse("icon:close");
        assert_eq!((l.left, l.right), (vec![], vec![Close]));
        let l = parse("minimize, close");
        assert_eq!((l.left, l.right), (vec![], vec![Minimize, Close]));
    }
}
