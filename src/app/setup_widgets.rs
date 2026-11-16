//! The setup window's widgets: page title, switch and radio cards, the list of steps, and the
//! footer buttons.

use super::*;
use crate::setup::Kind;

pub(super) const ROW_H: f32 = 50.0;
pub(super) const WARN: Color32 = Color32::from_rgb(232, 196, 106);

/// The list of what the install or uninstall did, in a card at the top of `r`, with the slow
/// step under way (`busy`) last, behind a spinner. Returns the card's bottom (`r.top()` with
/// nothing to list).
pub(super) fn steps(ui: &Ui, r: Rect, lines: &[(Kind, String)], busy: Option<&str>) -> f32 {
    if lines.is_empty() && busy.is_none() {
        return r.top();
    }
    let p = ui.painter();
    let wrap = r.width() - 52.0;
    let mut y = r.top() + 12.0;
    let mut rows = Vec::new();
    for (kind, text) in lines {
        let (glyph, color) = match kind {
            Kind::Ok => (icon::CHECK, GREEN),
            Kind::Warn => (icon::WARNING, WARN),
            Kind::Fail => (icon::X_CIRCLE, RED),
        };
        let text_color = if *kind == Kind::Ok { MUTED } else { color };
        let g = p.layout(text.clone(), mono(11.0), text_color, wrap);
        rows.push((pos2(r.left() + 14.0, y), glyph, color, g.clone()));
        y += g.size().y + 8.0;
    }
    let spinner = busy.map(|text| {
        let g = p.layout(text.to_owned(), mono(11.0), TEXT, wrap);
        let at = pos2(r.left() + 14.0, y);
        y += g.size().y + 8.0;
        (at, g)
    });
    let card = Rect::from_min_max(r.min, pos2(r.right(), (y + 4.0).min(r.bottom())));
    p.rect(
        card,
        9.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    for (at, glyph, color, g) in rows {
        p.text(
            at + vec2(0.0, 1.0),
            Align2::LEFT_TOP,
            glyph,
            mono(12.0),
            color,
        );
        p.galley(at + vec2(24.0, 0.0), g, TEXT);
    }
    if let Some((at, g)) = spinner {
        // A three-quarter ring turning once a second.
        let c = at + vec2(6.0, 7.5);
        let t = ui.input(|i| i.time) as f32 * std::f32::consts::TAU;
        let points = (0..=18)
            .map(|i| {
                let a = t + i as f32 / 18.0 * std::f32::consts::TAU * 0.75;
                c + vec2(a.cos(), a.sin()) * 5.0
            })
            .collect();
        p.add(egui::Shape::line(points, Stroke::new(1.5, GREEN)));
        p.galley(at + vec2(24.0, 0.0), g, TEXT);
        ui.ctx().request_repaint();
    }
    card.bottom()
}

/// The page title; returns the y below it.
pub(super) fn heading(ui: &Ui, r: Rect, text: &str) -> f32 {
    let g = ui.painter().layout_no_wrap(text.into(), bold(21.0), TEXT);
    let h = g.size().y;
    ui.painter().galley(r.min, g, TEXT);
    r.top() + h + 6.0
}

/// A card of rows, each an icon, a label and a switch.
pub(super) fn card<const N: usize>(
    ui: &mut Ui,
    at: Pos2,
    width: f32,
    rows: [(&str, &str, &mut bool); N],
) {
    let r = Rect::from_min_size(at, vec2(width, ROW_H * N as f32));
    ui.painter().rect(
        r,
        9.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    for (i, (glyph, label, on)) in rows.into_iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(r.left(), r.top() + i as f32 * ROW_H),
            vec2(width, ROW_H),
        );
        let resp = ui.interact(row, ui.id().with(("setup-row", label)), Sense::click());
        if resp.clicked() {
            *on = !*on;
        }
        let p = ui.painter();
        if i > 0 {
            p.hline(r.x_range(), row.top(), Stroke::new(1.0, LINE));
        }
        if resp.hovered() {
            p.rect_filled(row.shrink(1.0), 8.0, CARD_HI);
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let cy = row.center().y;
        p.text(
            pos2(row.left() + 14.0, cy),
            Align2::LEFT_CENTER,
            glyph,
            mono(18.0),
            MUTED,
        );
        p.text(
            pos2(row.left() + 46.0, cy),
            Align2::LEFT_CENTER,
            label,
            mono(12.5),
            TEXT,
        );
        let sw = Rect::from_center_size(pos2(row.right() - 29.0, cy), vec2(30.0, 18.0));
        p.rect_filled(sw, 9.0, if *on { GREEN } else { LINE });
        let knob = if *on {
            sw.right() - 9.0
        } else {
            sw.left() + 9.0
        };
        p.circle_filled(pos2(knob, cy), 7.0, if *on { INK } else { MUTED });
        if resp.has_focus() {
            focus_ring(p, row.shrink(2.0), 8.0);
        }
        let on = *on;
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label)
        });
    }
}

/// A card of rows, each an icon, a label, an optional line under it ("" for none) and a radio
/// dot; one is chosen. Returns the card's bottom.
pub(super) fn radios<T: Copy + PartialEq, const N: usize>(
    ui: &mut Ui,
    at: Pos2,
    width: f32,
    value: &mut T,
    rows: [(&str, &str, &str, T); N],
) -> f32 {
    let row_h = if rows.iter().any(|r| !r.2.is_empty()) {
        ROW_H + 12.0
    } else {
        ROW_H
    };
    let r = Rect::from_min_size(at, vec2(width, row_h * N as f32));
    ui.painter().rect(
        r,
        9.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    for (i, (glyph, label, under, this)) in rows.into_iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(r.left(), r.top() + i as f32 * row_h),
            vec2(width, row_h),
        );
        let resp = ui.interact(row, ui.id().with(("setup-radio", label)), Sense::click());
        if resp.clicked() {
            *value = this;
        }
        let on = *value == this;
        let p = ui.painter();
        if i > 0 {
            p.hline(r.x_range(), row.top(), Stroke::new(1.0, LINE));
        }
        if resp.hovered() {
            p.rect_filled(row.shrink(1.0), 8.0, CARD_HI);
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let cy = row.center().y;
        p.text(
            pos2(row.left() + 14.0, cy),
            Align2::LEFT_CENTER,
            glyph,
            mono(18.0),
            if on { GREEN } else { MUTED },
        );
        let label_y = if under.is_empty() { cy } else { cy - 9.0 };
        p.text(
            pos2(row.left() + 46.0, label_y),
            Align2::LEFT_CENTER,
            label,
            mono(12.5),
            TEXT,
        );
        if !under.is_empty() {
            p.text(
                pos2(row.left() + 46.0, cy + 10.0),
                Align2::LEFT_CENTER,
                under,
                mono(11.0),
                MUTED,
            );
        }
        let dot = pos2(row.right() - 24.0, cy);
        p.circle_stroke(dot, 8.0, Stroke::new(1.5, if on { GREEN } else { LINE }));
        if on {
            p.circle_filled(dot, 4.0, GREEN);
        }
        if resp.has_focus() {
            focus_ring(p, row.shrink(2.0), 8.0);
        }
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label)
        });
    }
    r.bottom()
}

pub(super) fn right(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Center), add);
}

/// The green call to action, sized to its label.
pub(super) fn primary(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    let w = ui
        .painter()
        .layout_no_wrap(label.into(), bold(12.5), INK)
        .size()
        .x;
    primary_caps(ui, glyph, label, vec2(w + 52.0, 34.0), false, &[])
}

/// The red outlined Uninstall button.
pub(super) fn danger(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    let p = ui.painter();
    let g = p.layout_no_wrap(label.into(), bold(12.5), RED);
    let size = vec2(g.size().x + 52.0, 34.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = RED.gamma_multiply(if resp.hovered() { 0.14 } else { 0.06 });
    let p = ui.painter();
    p.rect(
        r,
        7.0,
        fill,
        Stroke::new(1.0, RED.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    p.text(
        pos2(r.left() + 16.0, r.center().y),
        Align2::LEFT_CENTER,
        glyph,
        mono(14.0),
        RED,
    );
    p.galley(
        pos2(r.left() + 38.0, r.center().y - g.size().y / 2.0),
        g,
        RED,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.has_focus() {
        focus_ring(p, r, 7.0);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}
