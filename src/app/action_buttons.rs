use super::*;

/// Green-outlined contextual action ("Rescan").
pub(super) fn accent(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    accent_caps(ui, glyph, label, &[])
}

/// `accent` with key caps right of the label.
pub(super) fn accent_caps(ui: &mut Ui, glyph: &str, label: &str, caps: &[String]) -> Response {
    let p = ui.painter();
    let caps_w = if caps.is_empty() {
        0.0
    } else {
        caps_width(p, caps) + 8.0
    };
    let text = if label.is_empty() {
        glyph.to_string()
    } else {
        format!("{glyph}  {label}")
    };
    let g = p.layout_no_wrap(text, mono(11.0), GREEN);
    let (rect, resp) = ui.allocate_exact_size(
        vec2((g.size().x + caps_w + 20.0).max(30.0), 26.0),
        Sense::click(),
    );
    let fill = if resp.hovered() {
        GREEN.gamma_multiply(0.18)
    } else {
        GREEN.gamma_multiply(0.1)
    };
    let p = ui.painter();
    p.rect(
        rect,
        6.0,
        fill,
        Stroke::new(1.0, GREEN.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    let at = pos2(rect.min.x + 10.0, rect.center().y - g.size().y / 2.0);
    let text_w = g.size().x;
    if caps.is_empty() {
        p.galley(rect.center() - g.size() / 2.0, g, GREEN);
    } else {
        p.galley(at, g, GREEN);
        paint_caps(p, pos2(at.x + text_w + 8.0, rect.center().y - 11.0), caps);
    }
    if resp.has_focus() {
        focus_ring(p, rect, 6.0);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// Round green play/pause button. The glyphs are drawn, so they sit dead center.
pub(super) fn play_button(ui: &mut Ui, paused: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
    let p = ui.painter();
    let c = rect.center();
    p.add(glow(GREEN.gamma_multiply(0.4), 18).as_shape(rect, 19));
    let fill = if resp.hovered() {
        Color32::from_rgb(120, 255, 180)
    } else {
        GREEN
    };
    p.circle_filled(c, 19.0, fill);
    if paused {
        // Optically centered: a triangle's visual center sits right of its box center.
        let pts = vec![
            c + vec2(-4.5, -7.0),
            c + vec2(7.5, 0.0),
            c + vec2(-4.5, 7.0),
        ];
        p.add(egui::Shape::convex_polygon(pts, INK, Stroke::NONE));
    } else {
        for dx in [-4.0, 4.0] {
            p.rect_filled(
                Rect::from_center_size(c + vec2(dx, 0.0), vec2(4.0, 13.0)),
                1.0,
                INK,
            );
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let label = if paused {
        t!("tip.play")
    } else {
        t!("tip.pause")
    };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    tip_keys(resp, label, Cmd::PlayPause)
}
