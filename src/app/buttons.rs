use super::*;

/// Icon and/or label button. Selected: green text on a faint green fill with a green edge.
pub(super) fn button(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    selected: bool,
    look: Look,
) -> Response {
    button_caps(ui, glyph, label, selected, look, &[])
}

/// `button` with key caps right of the label.
pub(super) fn button_caps(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    selected: bool,
    look: Look,
    caps: &[String],
) -> Response {
    let p = ui.painter();
    let caps_w = if caps.is_empty() {
        0.0
    } else {
        caps_width(p, caps) + 8.0
    };
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
        + text_g.as_ref().map_or(0.0, |g| g.size().x)
        + caps_w;
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
        let w = g.size().x;
        p.galley_with_override_text_color(pos2(x, rect.center().y - g.size().y / 2.0), g, text_c);
        x += w + 8.0;
    }
    if !caps.is_empty() {
        paint_caps(p, pos2(x, rect.center().y - 11.0), caps);
    }
    if resp.has_focus() {
        focus_ring(ui.painter(), rect, look.radius);
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
    let resp = labeled_icon(ui, glyph, label, selected, look);
    resp.on_hover_text(label)
}

/// `icon_button_look` whose tooltip also shows the keys of `cmds`.
pub(super) fn icon_button_keys(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    selected: bool,
    look: Look,
    cmds: &[Cmd],
) -> Response {
    let resp = labeled_icon(ui, glyph, label, selected, look);
    tip_cmds(resp, label, cmds)
}

fn labeled_icon(ui: &mut Ui, glyph: &str, label: &str, selected: bool, look: Look) -> Response {
    let resp = button(ui, glyph, "", selected, look);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    resp
}

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
