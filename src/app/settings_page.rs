//! The settings window's content: the page title, then cards of rows. A search shows the
//! matching rows of every page instead.

use super::settings_controls::control;
use super::settings_items::{Group, Item, PAGES, Page, matches};
use super::settings_text::{label, reset};
use super::settings_window::SettingsView;
use super::*;

const ROW_H: f32 = 54.0;

pub(super) fn page(ui: &mut Ui, view: &mut SettingsView) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin {
            left: 26,
            right: 26,
            top: 18,
            bottom: 18, // a long page ends this far above the window's edge, like its bar
        }))
        .show(ui, |ui| {
            let query = view.query.trim().to_string();
            if query.is_empty() {
                title(ui, view);
            }
            // A new page, or a search, starts at the top.
            let (key, shown) = (egui::Id::new("settings_shown"), (view.page, query.clone()));
            let old =
                ui.data_mut(|d| std::mem::replace(d.get_temp_mut_or_default(key), shown.clone()));
            let mut area = egui::ScrollArea::vertical().auto_shrink([false, true]);
            if old != shown {
                area = area.vertical_scroll_offset(0.0);
            }
            area.show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                if query.is_empty() {
                    if view.page == Page::About {
                        super::settings_widgets::hero(ui);
                    }
                    for g in view.page.groups() {
                        group(ui, view, g, g.items);
                    }
                } else {
                    results(ui, view, &query);
                }
                ui.add_space(6.0);
            });
        });
}

fn title(ui: &mut Ui, view: &mut SettingsView) {
    let (glyph, key) = view.page.text();
    ui.horizontal(|ui| {
        ui.set_height(30.0);
        ui.label(RichText::new(glyph).font(mono(20.0)).color(GREEN));
        ui.label(RichText::new(i18n::tr(key)).font(bold(18.0)).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if view.page.resettable()
                && button(
                    ui,
                    icon::ARROW_COUNTER_CLOCKWISE,
                    t!("settings.reset"),
                    false,
                    GHOST,
                )
                .clicked()
            {
                let defaults = crate::settings::Settings::default();
                for g in view.page.groups() {
                    for &item in g.items {
                        reset(item, &mut view.settings, &defaults);
                    }
                }
            }
        });
    });
    ui.add_space(12.0);
}

/// Every page's matching rows, under the page's name.
fn results(ui: &mut Ui, view: &mut SettingsView, query: &str) {
    let mut any = false;
    for page in PAGES {
        let hits: Vec<(&Group, Vec<Item>)> = page
            .groups()
            .iter()
            .map(|g| {
                (
                    g,
                    g.items
                        .iter()
                        .copied()
                        .filter(|&i| i.available() && matches(i, query))
                        .collect(),
                )
            })
            .filter(|(_, items): &(_, Vec<Item>)| !items.is_empty())
            .collect();
        if hits.is_empty() {
            continue;
        }
        any = true;
        let (glyph, key) = page.text();
        ui.horizontal(|ui| {
            ui.label(RichText::new(glyph).font(mono(15.0)).color(GREEN));
            ui.label(RichText::new(i18n::tr(key)).font(bold(14.0)).color(TEXT));
        });
        ui.add_space(10.0);
        for (g, items) in hits {
            group(ui, view, g, &items);
        }
    }
    if !any {
        let none = RichText::new(t!("settings.no_match"))
            .font(mono(12.0))
            .color(MUTED);
        ui.add_space(40.0);
        ui.vertical_centered(|ui| ui.label(none));
    }
}

/// A group's caps title, its note, and its rows in one card.
fn group(ui: &mut Ui, view: &mut SettingsView, g: &Group, items: &[Item]) {
    let items: Vec<Item> = items.iter().copied().filter(|i| i.available()).collect();
    if items.is_empty() {
        return;
    }
    let caps = i18n::tr(g.title).to_uppercase();
    ui.label(
        RichText::new(caps)
            .font(bold(11.0))
            .color(MUTED)
            .extra_letter_spacing(1.3),
    );
    ui.add_space(6.0);
    if let Some(note) = g.note {
        ui.label(
            RichText::new(i18n::tr(note))
                .font(mono(11.0))
                .color(GREEN_DIM),
        );
        ui.add_space(6.0);
    }
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(9.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, &item) in items.iter().enumerate() {
                if i > 0 {
                    let y = ui.cursor().top();
                    ui.painter()
                        .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, LINE));
                }
                row(ui, view, item);
            }
        });
    ui.add_space(18.0);
}

/// Icon, title and hint on the left; the control on the right.
fn row(ui: &mut Ui, view: &mut SettingsView, item: Item) {
    let (glyph, ..) = item.text();
    let (title, hint) = label(item, view);
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            // A fixed height: a right-to-left layout would otherwise take all the room left.
            let size = vec2(ui.available_width(), ROW_H - 16.0);
            ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                control(ui, item, view);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    // A fixed icon column keeps the titles lined up.
                    let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                    let p = ui.painter();
                    p.text(r.center(), Align2::CENTER_CENTER, glyph, mono(18.0), MUTED);
                    let title = RichText::new(title).font(mono(12.5)).color(TEXT);
                    match hint {
                        // Alone, the title sits on the row's middle like the control.
                        None => {
                            ui.label(title);
                        }
                        Some(h) => {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                ui.label(title);
                                ui.label(RichText::new(h).font(mono(11.0)).color(MUTED));
                            });
                        }
                    }
                });
            });
        });
}
