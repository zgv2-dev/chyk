//! Small controls for the settings rows, each editing one value in place.

use super::settings_items::Item;
use super::*;

/// Segmented choice of `values`, as wide as its labels need.
pub(super) fn seg<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    names: &[String],
    values: &[T],
    value: &mut T,
) {
    let text_w: f32 = names
        .iter()
        .map(|n| {
            ui.painter()
                .layout_no_wrap(n.clone(), mono(12.0), TEXT)
                .size()
                .x
                + 18.0
        })
        .sum();
    let w = text_w + 2.0 * names.len() as f32 + 6.0;
    let at = values.iter().position(|v| v == value).unwrap_or(0);
    ui.allocate_ui(vec2(w, 36.0), |ui| {
        if let Some(i) = segmented(ui, id, names, at) {
            *value = values[i];
        }
    });
}

/// Drop-down choice of `values`, `width` wide.
pub(super) fn pick_list<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    names: &[String],
    values: &[T],
    value: &mut T,
    width: f32,
) {
    let at = values.iter().position(|v| v == value).unwrap_or(0);
    ui.allocate_ui(vec2(width, 36.0), |ui| {
        if let Some(i) = dropdown(ui, id, names, &[], at) {
            *value = values[i];
        }
    });
}

pub(super) fn flip(ui: &mut Ui, on: &mut bool) {
    if switch(ui, *on).clicked() {
        *on = !*on;
    }
}

/// − value + over `len` steps, at step `at`. Returns the new step.
pub(super) fn stepper(ui: &mut Ui, text: &str, at: usize, len: usize) -> Option<usize> {
    let mut to = None;
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::same(1))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // Built right to left inside the row's layout: + first, then the value, then −.
            ui.add_enabled_ui(at + 1 < len, |ui| {
                if icon_button_look(ui, icon::PLUS, "+", false, ICON_SMALL).clicked() {
                    to = Some(at + 1);
                }
            });
            let (r, _) = ui.allocate_exact_size(vec2(64.0, 24.0), Sense::hover());
            let p = ui.painter();
            p.vline(r.left(), r.y_range(), Stroke::new(1.0, LINE));
            p.vline(r.right(), r.y_range(), Stroke::new(1.0, LINE));
            p.text(r.center(), Align2::CENTER_CENTER, text, mono(12.0), TEXT);
            ui.add_enabled_ui(at > 0, |ui| {
                if icon_button_look(ui, icon::MINUS, "−", false, ICON_SMALL).clicked() {
                    to = Some(at - 1);
                }
            });
        });
    to
}

/// The row's link in green with an arrow; a click opens it in the browser.
pub(super) fn link(ui: &mut Ui, item: Item) {
    let Some(url) = item.url() else { return };
    let shown = if item == Item::License {
        "GPL-3.0-or-later"
    } else {
        url.trim_start_matches("https://")
    };
    let text = format!("{shown} {}", icon::ARROW_UP_RIGHT);
    let resp = ui.add(
        egui::Label::new(RichText::new(text).font(mono(11.5)).color(GREEN)).sense(Sense::click()),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.on_hover_text(url).clicked() {
        crate::reveal::open(std::ffi::OsStr::new(url));
    }
}

impl Item {
    /// The link a row opens, shown on its right.
    pub(super) fn url(self) -> Option<&'static str> {
        match self {
            Item::Website => Some("https://chyk.zgv2.dev"),
            Item::Source => Some(crate::update::REPO),
            Item::Report => Some(concat!("https://github.com/zgv2-dev/chyk", "/issues")),
            Item::License => Some("https://www.gnu.org/licenses/gpl-3.0.html"),
            _ => None,
        }
    }
}
