use super::*;

/// Small animated switch.
pub(super) fn switch(ui: &mut Ui, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    let t = ui.ctx().animate_bool(resp.id, on);
    let p = ui.painter();
    let (fill, edge) = if on {
        (GREEN.gamma_multiply(0.85), GREEN)
    } else {
        (CARD, LINE)
    };
    p.rect(
        rect,
        11.0,
        fill,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    let cx = rect.left() + 11.0 + t * (rect.width() - 22.0);
    p.circle_filled(pos2(cx, rect.center().y), 7.0, if on { INK } else { MUTED });
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            true,
            on,
            if on {
                t!("common.on")
            } else {
                t!("common.off")
            },
        )
    });
    resp
}

/// Equal-width segmented choice across the available width. Returns the clicked index.
pub(super) fn segmented(
    ui: &mut Ui,
    id: &str,
    options: &[String],
    selected: usize,
) -> Option<usize> {
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    let p = ui.painter();
    p.rect(
        outer,
        8.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    let inner = outer.shrink(3.0);
    let w = (inner.width() - 2.0 * (options.len() as f32 - 1.0)) / options.len() as f32;
    let mut clicked = None;
    for (i, label) in options.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(inner.min.x + i as f32 * (w + 2.0), inner.min.y),
            vec2(w, inner.height()),
        );
        let resp = ui.interact(r, ui.id().with(id).with(i), Sense::click());
        let on = i == selected;
        let (fill, stroke, color) = if on {
            (
                GREEN.gamma_multiply(0.11),
                Stroke::new(1.0, GREEN.gamma_multiply(0.5)),
                GREEN,
            )
        } else if resp.hovered() {
            (CARD_HI, Stroke::NONE, TEXT)
        } else {
            (Color32::TRANSPARENT, Stroke::NONE, MUTED)
        };
        let p = ui.painter();
        p.rect(r, 6.0, fill, stroke, egui::StrokeKind::Inside);
        p.text(r.center(), Align2::CENTER_CENTER, label, mono(12.0), color);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label)
        });
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Muted label with a dim green icon, above a setting.
pub(super) fn field_label(ui: &mut Ui, glyph: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(glyph).font(mono(15.0)).color(GREEN_DIM));
        ui.label(RichText::new(text).font(mono(12.0)).color(MUTED));
    });
}

/// Icon, label and a switch on the right. Returns true when toggled.
pub(super) fn toggle_row(ui: &mut Ui, glyph: &str, title: &str, hint: &str, on: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(glyph).font(mono(15.0)).color(GREEN_DIM));
        ui.label(RichText::new(title).font(mono(12.0)).color(TEXT))
            .on_hover_text(hint);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if switch(ui, on).on_hover_text(hint).clicked() {
                changed = true;
            }
        });
    });
    changed
}

/// Keyboard key drawn as a key cap.
pub(super) fn keycap(p: &egui::Painter, at: Pos2, key: &str) -> f32 {
    let g = p.layout_no_wrap(key.into(), mono(11.0), TEXT);
    let r = Rect::from_min_size(at, vec2((g.size().x + 12.0).max(22.0), 22.0));
    p.rect(
        r,
        5.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    p.hline(
        r.x_range().shrink(3.0),
        r.max.y - 1.0,
        Stroke::new(1.0, LINE),
    );
    p.galley(r.center() - g.size() / 2.0, g, TEXT);
    r.width()
}

/// Framed row of controls (crop ratios, strip, video zoom), `height` tall overall. Returns its
/// width.
pub(super) fn bar_frame(ui: &mut Ui, height: f32, add: impl FnOnce(&mut Ui)) -> f32 {
    let margin = ((height - 26.0) / 2.0).max(0.0);
    egui::Frame::new()
        .fill(GLASS)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7)
        .inner_margin(margin)
        .show(ui, |ui| {
            // Without this the frame grows to the row's full height.
            ui.set_height(26.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            add(ui);
        })
        .response
        .rect
        .width()
}

/// Fill the first `frac` of `bar`. Painted at full size and clipped, so near 0% it is a sliver
/// of the rounded end, not a rounded blob wider than the progress it shows.
pub(super) fn progress_fill(p: &egui::Painter, bar: Rect, frac: f32, radius: f32, color: Color32) {
    let mut done = bar;
    done.max.x = bar.min.x + bar.width() * frac.clamp(0.0, 1.0);
    if done.width() > 0.0 {
        p.with_clip_rect(done.intersect(p.clip_rect()))
            .rect_filled(bar, radius, color);
    }
}

/// Thin vertical separator.
pub(super) fn vline(ui: &mut Ui, height: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
    ui.painter()
        .vline(r.center().x, r.y_range(), Stroke::new(1.0, LINE));
}
