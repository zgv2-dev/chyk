//! Applying the Interface size setting: egui's zoom, and windows that keep their layout.

use super::*;

/// Draw at `size`. egui's own Ctrl +/- zoom is off, so the setting is the one place it's set.
/// The zoom is one for all windows, so Auto follows the main window's screen only: windows on
/// screens of different scales would otherwise keep flipping it.
pub(super) fn set_ui_size(ctx: &egui::Context, size: crate::settings::UiSize) {
    let id = egui::Id::new("chyk-main-native-ppp");
    let native = if ctx.viewport_id() == egui::ViewportId::ROOT {
        let n = ctx.native_pixels_per_point().unwrap_or(1.0);
        ctx.data_mut(|d| d.insert_temp(id, n));
        n
    } else {
        ctx.data(|d| d.get_temp::<f32>(id))
            .or_else(|| ctx.native_pixels_per_point())
            .unwrap_or(1.0)
    };
    let z = size.zoom(native);
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    if (ctx.zoom_factor() - z).abs() > 1e-3 {
        ctx.set_zoom_factor(z);
    }
}

/// Keep this window's layout when the interface size changes: egui rescales the content but
/// not the window, so a bigger size would leave less room in points. Resize it by as much,
/// with its minimum (`min`, in points), staying within 90% of the monitor.
pub(super) fn follow_ui_size(ctx: &egui::Context, min: Vec2) {
    let id = egui::Id::new("chyk-ui-size").with(ctx.viewport_id());
    let z = ctx.zoom_factor();
    let (size, monitor) = ctx.input(|i| {
        let v = i.viewport();
        (v.inner_rect.map(|r| r.size()), v.monitor_size)
    });
    let Some(size) = size else { return };
    let seen = ctx.data(|d| d.get_temp::<(f32, Vec2)>(id));
    if let Some((was, was_size)) = seen
        && (was - z).abs() > 1e-3
    {
        // The window's current monitor, measured in points at the zoom the input was taken
        // with (`was`): in points at the new zoom it's `was / z` of that.
        let room = monitor.map_or(Vec2::INFINITY, |m| m * (0.9 * was / z));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min.min(room)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(was_size.min(room)));
        // Measured again once the window has its new size.
        ctx.data_mut(|d| d.remove::<(f32, Vec2)>(id));
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, (z, size)));
}
