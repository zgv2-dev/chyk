//! The `?` sheet: every shortcut, grouped, with a filter.

use super::*;

mod body;
#[cfg(test)]
mod tests;

/// One row per command name: all the bindings of a family (crop presets, recent files) share one.
/// Matches `filter` against the English or current-language name, ignoring case.
pub(super) fn sheet_rows(filter: &str) -> Vec<(Group, Cmd, Vec<Binding>)> {
    let mut rows: Vec<(Group, Cmd, Vec<Binding>)> = Vec::new();
    for b in bindings() {
        match rows.iter_mut().find(|r| label(r.1) == label(b.cmd)) {
            Some(r) => r.2.push(b),
            None => rows.push((b.group, b.cmd, vec![b])),
        }
    }
    rows.sort_by_key(|r| r.0);
    let f = filter.trim().to_lowercase();
    rows.retain(|r| {
        let l = label(r.1);
        i18n::en(l).to_lowercase().contains(&f) || i18n::tr(l).to_lowercase().contains(&f)
    });
    rows
}

/// The chords a row lists. A Shift variant of a key already listed, and `=` beside `+`, exist
/// only for keyboard layouts, so they fold into the first.
pub(super) fn shown(bs: &[Binding]) -> Vec<&Binding> {
    let mut out: Vec<&Binding> = Vec::new();
    for b in bs {
        let twin = |s: &&Binding| s.key == b.key || (b.key == Key::Equals && s.key == Key::Plus);
        if !out.iter().any(twin) {
            out.push(b);
        }
    }
    out
}

impl App {
    pub(super) fn sheet_ui(&mut self, ctx: &egui::Context) {
        let Some(mut filter) = self.sheet.take() else {
            return;
        };
        let screen = ctx.content_rect();
        let scrim = egui::Area::new("sheet_scrim".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(r, 0.0, Color32::from_black_alpha(150));
                resp
            })
            .inner;
        let width = (screen.width() - 32.0).min(1000.0);
        // Below the header, so the window's own buttons stay clear.
        let top = self.anchors.header_bottom + 12.0;
        let shown = egui::Area::new("sheet".into())
            .fixed_pos(pos2(screen.center().x - width / 2.0, top))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(14.0)
                    .inner_margin(egui::Margin::symmetric(18, 14))
                    .shadow(shadow())
                    .show(ui, |ui| {
                        ui.set_width(width - 36.0);
                        let id = ui.id().with("filter");
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon::KEYBOARD).font(mono(17.0)).color(GREEN));
                            ui.label(
                                RichText::new(t!("sheet.title"))
                                    .font(bold(14.0))
                                    .color(TEXT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut filter)
                                        .id(id)
                                        .hint_text(t!("sheet.filter"))
                                        .font(mono(12.0))
                                        .desired_width(220.0),
                                );
                            });
                        });
                        // The filter keeps the keyboard while the sheet is open.
                        ui.memory_mut(|m| m.request_focus(id));
                        ui.add_space(8.0);
                        // Title row and frame margins take ~72 pt; the rows scroll in the rest.
                        let room = (screen.max.y - top - 12.0 - 72.0).max(80.0);
                        egui::ScrollArea::vertical()
                            .max_height(room)
                            .auto_shrink([false, true])
                            .show(ui, |ui| self.sheet_body(ui, &filter));
                    });
            });
        ctx.move_to_top(shown.response.layer_id);
        // Clicking outside closes it, like the popovers.
        let outside = scrim.clicked()
            && ctx
                .pointer_interact_pos()
                .is_some_and(|p| !shown.response.rect.contains(p));
        if !outside {
            self.sheet = Some(filter);
        }
    }
}
