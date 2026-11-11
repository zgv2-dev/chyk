//! The log window's header: title and read-only note, then the kind filter and Copy.

use super::log_text::{entry_text, kind_label};
use super::log_window::{KINDS, LogView};
use super::*;

/// Title and read-only note, then the kind filter and Copy at the right, in reading order.
pub(super) fn head(ui: &mut Ui, view: &mut LogView) {
    let bar = egui::Panel::top("log-head")
        .show_separator_line(false)
        .exact_size(40.0)
        .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin {
            left: 12,
            right: 8,
            top: 0,
            bottom: 0,
        }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let narrow = ui.available_width() < 640.0;
                let title_w = (ui.available_width() - view.right).max(0.0);
                ui.allocate_ui_with_layout(
                    vec2(title_w, ui.available_height()),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_width(title_w);
                        let title = RichText::new(t!("log.title")).font(bold(12.5)).color(TEXT);
                        ui.add(egui::Label::new(title).truncate());
                        let note = RichText::new(t!("log.readonly"))
                            .font(mono(11.0))
                            .color(MUTED);
                        ui.add(egui::Label::new(note).truncate());
                    },
                );
                let x = ui.cursor().min.x;
                filter(ui, view);
                let label = if narrow { "" } else { t!("meta.copy") };
                let copy =
                    button(ui, icon::COPY, label, false, GHOST).on_hover_text(t!("meta.copy"));
                if copy.clicked() {
                    let text: String = view.shown().into_iter().map(entry_text).collect();
                    ui.ctx().copy_text(text);
                }
                let right = ui.cursor().min.x - x;
                if (right - view.right).abs() > 0.5 {
                    view.right = right;
                    ui.ctx().request_repaint();
                }
            });
        });
    let r = bar.response.rect;
    ui.painter()
        .hline(r.x_range(), r.bottom(), Stroke::new(1.0, LINE));
}

fn filter(ui: &mut Ui, view: &mut LogView) {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            // Without this the frame grows to the header row's full height.
            ui.set_height(SEG.height);
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, kind) in KINDS.into_iter().enumerate() {
                if button(ui, "", kind_label(kind), i == view.filter, SEG).clicked() {
                    view.filter = i;
                }
            }
        });
}
