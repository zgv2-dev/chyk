//! The settings window's header (title, window buttons) and its sidebar of pages.

use super::settings_items::PAGES;
use super::settings_window::SettingsView;
use super::*;

/// Header height; on macOS the traffic lights are centered in it.
pub(super) const HEAD_H: f32 = 40.0;
const SIDEBAR_W: f32 = 214.0;

pub(super) fn head(ui: &mut Ui, view: &SettingsView) {
    let bar = egui::Panel::top("settings-head")
        .show_separator_line(false)
        .exact_size(HEAD_H)
        .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin {
            left: 12,
            right: 8,
            top: 0,
            bottom: 0,
        }))
        .show(ui, |ui| {
            // The header is the titlebar; registered before the buttons, so they keep their clicks.
            let bar = ui.max_rect().expand2(vec2(12.0, 0.0));
            title_bar(&ui.interact(bar, ui.id().with("titlebar"), Sense::click_and_drag()));
            ui.horizontal_centered(|ui| {
                if cfg!(target_os = "macos") {
                    ui.add_space(73.0);
                }
                if !view.buttons.left.is_empty() {
                    window_buttons(ui, view.buttons.left.iter());
                    ui.add_space(10.0);
                }
                let title = RichText::new(t!("settings.title"))
                    .font(bold(12.5))
                    .color(TEXT);
                ui.label(title);
                let note = match &view.error {
                    Some(e) => RichText::new(t!("settings.save_failed", error = e)).color(RED),
                    None => RichText::new(t!("settings.instant")).color(MUTED),
                };
                ui.add(egui::Label::new(note.font(mono(11.0))).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !view.buttons.right.is_empty() {
                        window_buttons(ui, view.buttons.right.iter().rev());
                    }
                });
            });
        });
    let r = bar.response.rect;
    ui.painter()
        .hline(r.x_range(), r.bottom(), Stroke::new(1.0, LINE));
}

/// Search, the pages, and Open settings file at the bottom.
pub(super) fn sidebar(ui: &mut Ui, view: &mut SettingsView) {
    let side = egui::Panel::left("settings-nav")
        .resizable(false)
        .show_separator_line(false)
        .exact_size(SIDEBAR_W)
        .frame(
            egui::Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(10, 12)),
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            search(ui, view);
            ui.add_space(10.0);
            for page in PAGES {
                // A search shows every page's matches, so no page is the current one then.
                let on = view.page == page && view.query.trim().is_empty();
                let (glyph, key) = page.text();
                if nav_button(ui, glyph, i18n::tr(key), on).clicked() {
                    view.page = page;
                    view.query.clear();
                }
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                let file = button(ui, icon::FILE_CODE, t!("settings.open_file"), false, GHOST);
                if file.clicked() {
                    open_file(view);
                }
            });
        });
    let r = side.response.rect;
    ui.painter()
        .vline(r.right(), r.y_range(), Stroke::new(1.0, LINE));
}

fn search(ui: &mut Ui, view: &mut SettingsView) {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::MAGNIFYING_GLASS)
                        .font(mono(13.0))
                        .color(MUTED),
                );
                let id = ui.id().with("settings-search");
                ui.add(
                    egui::TextEdit::singleline(&mut view.query)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .hint_text(t!("settings.search"))
                        .font(mono(12.0))
                        .desired_width(f32::INFINITY),
                );
                // ⌘F / Ctrl+F jumps here.
                if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F)) {
                    ui.memory_mut(|m| m.request_focus(id));
                }
            });
        });
}

/// A full-width page button; the current page gets a green bar on its left edge.
fn nav_button(ui: &mut Ui, glyph: &str, label: &str, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let p = ui.painter();
    let (fill, color) = if on {
        (GREEN.gamma_multiply(0.10), GREEN)
    } else if resp.hovered() {
        (CARD_HI, TEXT)
    } else {
        (Color32::TRANSPARENT, MUTED)
    };
    p.rect_filled(rect, 7.0, fill);
    if on {
        let bar = Rect::from_min_max(
            pos2(rect.left(), rect.top() + 9.0),
            pos2(rect.left() + 3.0, rect.bottom() - 9.0),
        );
        p.rect_filled(bar, 2.0, GREEN);
    }
    let x = rect.left() + 12.0;
    p.text(
        pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        glyph,
        mono(16.0),
        color,
    );
    p.text(
        pos2(x + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        mono(12.5),
        color,
    );
    if resp.has_focus() {
        focus_ring(p, rect, 7.0);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, label));
    resp
}

/// Opens `settings.txt` in the default text editor, writing it first if it doesn't exist yet.
fn open_file(view: &mut SettingsView) {
    let Some(path) = crate::settings::path() else {
        return;
    };
    if !path.exists()
        && let Err(e) = view.settings.save()
    {
        view.error = Some(e.to_string());
        return;
    }
    crate::reveal::open(path.as_os_str());
}
