//! The top of the About page: the mark, name, what Chyk does, and the version.

use super::settings_window::SettingsView;
use super::*;

pub(super) fn hero(ui: &mut Ui, _view: &SettingsView) {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(9.0)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                logo(ui);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.label(RichText::new("C H Y K").font(bold(16.0)).color(GREEN));
                    let made = t!("settings.made_by", name = "zgv2");
                    ui.label(RichText::new(made).font(mono(11.0)).color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let v = format!("v{}", env!("CARGO_PKG_VERSION"));
                    ui.label(RichText::new(v).font(mono(13.0)).color(GREEN));
                });
            });
        });
    ui.add_space(18.0);
}
