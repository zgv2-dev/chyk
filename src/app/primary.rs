use super::*;

/// Filled green call to action with a soft glow. `active` rings it while its popover is open.
pub(super) fn primary(ui: &mut Ui, glyph: &str, label: &str, size: Vec2, active: bool) -> Response {
    primary_caps(ui, glyph, label, size, active, &[])
}

/// `primary` with key caps right of the label (the caller sizes `size` to fit them).
pub(super) fn primary_caps(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    size: Vec2,
    active: bool,
    caps: &[String],
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.add(glow(GREEN.gamma_multiply(0.3), 22).as_shape(rect, 8));
    if active {
        p.rect_stroke(
            rect.expand(3.0),
            10.0,
            Stroke::new(3.0, GREEN.gamma_multiply(0.25)),
            egui::StrokeKind::Inside,
        );
    }
    let fill = if resp.is_pointer_button_down_on() {
        GREEN.gamma_multiply(0.8)
    } else if resp.hovered() {
        Color32::from_rgb(120, 255, 180)
    } else {
        GREEN
    };
    p.rect_filled(rect, 8.0, fill);
    let icon_g = p.layout_no_wrap(glyph.into(), mono(14.0), INK);
    let text_g = p.layout_no_wrap(label.into(), bold(12.5), INK);
    let caps_w = if caps.is_empty() {
        0.0
    } else {
        caps_width(p, caps) + 8.0
    };
    let w = icon_g.size().x + 7.0 + text_g.size().x + caps_w;
    let x = rect.center().x - w / 2.0;
    let iw = icon_g.size().x;
    p.galley(
        pos2(x, rect.center().y - icon_g.size().y / 2.0),
        icon_g,
        INK,
    );
    let tw = text_g.size().x;
    p.galley(
        pos2(x + iw + 7.0, rect.center().y - text_g.size().y / 2.0),
        text_g,
        INK,
    );
    if !caps.is_empty() {
        let at = pos2(x + iw + 7.0 + tw + 8.0, rect.center().y - 11.0);
        paint_caps(p, at, caps);
    }
    if resp.has_focus() {
        focus_ring(p, rect, 8.0);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}
