use super::*;

/// Icon and/or label button. Selected: green text on a faint green fill with a green edge.
pub(super) fn button(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    selected: bool,
    look: Look,
) -> Response {
    let p = ui.painter();
    let icon_g = (!glyph.is_empty())
        .then(|| p.layout_no_wrap(glyph.into(), mono(look.icon), Color32::PLACEHOLDER));
    let text_g = (!label.is_empty())
        .then(|| p.layout_no_wrap(label.into(), mono(look.text), Color32::PLACEHOLDER));
    let gap = if icon_g.is_some() && text_g.is_some() {
        look.gap
    } else {
        0.0
    };
    let content = icon_g.as_ref().map_or(0.0, |g| g.size().x)
        + gap
        + text_g.as_ref().map_or(0.0, |g| g.size().x);
    let width = if text_g.is_none() {
        look.height.max(content + 8.0)
    } else {
        content + look.pad * 2.0
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, look.height), Sense::click());
    let hovered = resp.hovered();
    let (fill, stroke, icon_c, text_c) = if selected {
        (
            GREEN.gamma_multiply(0.11),
            Stroke::new(1.0, GREEN.gamma_multiply(0.5)),
            GREEN,
            GREEN,
        )
    } else if hovered {
        (CARD_HI, Stroke::NONE, TEXT, TEXT)
    } else {
        (Color32::TRANSPARENT, Stroke::NONE, MUTED, look.idle)
    };
    let p = ui.painter();
    p.rect(rect, look.radius, fill, stroke, egui::StrokeKind::Inside);
    let mut x = rect.center().x - content / 2.0;
    if let Some(g) = icon_g {
        let w = g.size().x;
        p.galley_with_override_text_color(pos2(x, rect.center().y - g.size().y / 2.0), g, icon_c);
        x += w + gap;
    }
    if let Some(g) = text_g {
        p.galley_with_override_text_color(pos2(x, rect.center().y - g.size().y / 2.0), g, text_c);
    }
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let name = if label.is_empty() { glyph } else { label };
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, name));
    resp
}

/// Icon-only button with a tooltip and a screen-reader label.
pub(super) fn icon_button(ui: &mut Ui, glyph: &str, label: &str, selected: bool) -> Response {
    icon_button_look(ui, glyph, label, selected, ICON)
}

pub(super) fn icon_button_look(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    selected: bool,
    look: Look,
) -> Response {
    let resp = button(ui, glyph, "", selected, look);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    resp.on_hover_text(label)
}

/// Filled green call to action with a soft glow. `active` rings it while its popover is open.
pub(super) fn primary(ui: &mut Ui, glyph: &str, label: &str, size: Vec2, active: bool) -> Response {
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
    let w = icon_g.size().x + 7.0 + text_g.size().x;
    let x = rect.center().x - w / 2.0;
    let iw = icon_g.size().x;
    p.galley(
        pos2(x, rect.center().y - icon_g.size().y / 2.0),
        icon_g,
        INK,
    );
    p.galley(
        pos2(x + iw + 7.0, rect.center().y - text_g.size().y / 2.0),
        text_g,
        INK,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// Green-outlined contextual action ("Rescan").
pub(super) fn accent(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    let p = ui.painter();
    let text = if label.is_empty() {
        glyph.to_string()
    } else {
        format!("{glyph}  {label}")
    };
    let g = p.layout_no_wrap(text, mono(11.0), GREEN);
    let (rect, resp) =
        ui.allocate_exact_size(vec2((g.size().x + 20.0).max(30.0), 26.0), Sense::click());
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
    p.galley(rect.center() - g.size() / 2.0, g, GREEN);
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
    resp.on_hover_text(label)
}
