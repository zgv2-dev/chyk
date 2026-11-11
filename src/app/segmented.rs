use super::*;

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
    // Width follows the text, so a long label doesn't clip beside short ones.
    let want: Vec<f32> = options
        .iter()
        .map(|o| {
            ui.painter()
                .layout_no_wrap(o.clone(), mono(12.0), TEXT)
                .size()
                .x
                + 14.0
        })
        .collect();
    let spare = inner.width() - 2.0 * (options.len() as f32 - 1.0);
    let scale = spare / want.iter().sum::<f32>();
    let mut x = inner.min.x;
    let mut clicked = None;
    for (i, label) in options.iter().enumerate() {
        let w = want[i] * scale;
        let r = Rect::from_min_size(pos2(x, inner.min.y), vec2(w, inner.height()));
        x += w + 2.0;
        let seg = ui.id().with(id).with(i);
        let resp = ui.interact(r, seg, Sense::click());
        let on = i == selected;
        // ←/→ move the choice (and the focus) along; egui must not steer focus with them.
        if resp.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    seg,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        ..Default::default()
                    },
                )
            });
            let step = ui.input_mut(|inp| {
                i32::from(inp.consume_key(Modifiers::NONE, Key::ArrowRight))
                    - i32::from(inp.consume_key(Modifiers::NONE, Key::ArrowLeft))
            });
            let to = i as i32 + step;
            if step != 0 && (0..options.len() as i32).contains(&to) {
                clicked = Some(to as usize);
                // The new choice and focus draw on the next frame; no input will wake it.
                ui.ctx().request_repaint();
                ui.memory_mut(|m| m.request_focus(ui.id().with(id).with(to as usize)));
            }
        }
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
        if resp.has_focus() {
            focus_ring(p, r, 6.0);
        }
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
