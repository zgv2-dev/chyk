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

pub(super) fn flip(ui: &mut Ui, on: &mut bool) -> Response {
    let resp = switch(ui, *on);
    if resp.clicked() {
        *on = !*on;
    }
    resp
}

/// − value + over `len` steps, at step `at`. Returns the new step.
pub(super) fn stepper(ui: &mut Ui, text: &str, at: usize, len: usize) -> Option<usize> {
    let mut to = None;
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7.0)
        // The buttons' hover fill sits inside the frame, not against its edge.
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // Built right to left inside the row's layout: + first, then the value, then −.
            ui.add_enabled_ui(at + 1 < len, |ui| {
                if icon_button_look(ui, icon::PLUS, t!("settings.more"), false, ICON_SMALL)
                    .clicked()
                {
                    to = Some(at + 1);
                }
            });
            let (r, _) = ui.allocate_exact_size(vec2(64.0, 24.0), Sense::hover());
            let p = ui.painter();
            let ys = r.shrink2(vec2(0.0, 3.0)).y_range();
            p.vline(r.left() + 3.0, ys, Stroke::new(1.0, LINE));
            p.vline(r.right() - 3.0, ys, Stroke::new(1.0, LINE));
            p.text(r.center(), Align2::CENTER_CENTER, text, mono(12.0), TEXT);
            ui.add_enabled_ui(at > 0, |ui| {
                if icon_button_look(ui, icon::MINUS, t!("settings.less"), false, ICON_SMALL)
                    .clicked()
                {
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
            Item::Ffmpeg => Some("https://ffmpeg.org"),
            Item::Mpv => Some("https://mpv.io"),
            _ => None,
        }
    }

    /// Rows this system has: the right-click menu entry only where Chyk can add one.
    pub(super) fn available(self) -> bool {
        self != Item::ShellMenu || crate::shell_menu::SUPPORTED
    }
}

/// The top of the About page: the mark, name, what Chyk does, and the version.
pub(super) fn hero(ui: &mut Ui) {
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

/// A burst of confetti from `from`, `start` seconds into the app; false once it has fallen.
pub(super) fn confetti(ui: &Ui, start: f64, from: Pos2) -> bool {
    const LIFE: f32 = 1.4;
    let t = (ui.input(|i| i.time) - start) as f32;
    if t > LIFE {
        return false;
    }
    // Repeatable noise in 0..1 for piece `i`, property `k`.
    let rnd = |i: u32, k: u32| {
        let x = (i.wrapping_mul(7919) ^ k.wrapping_mul(104_729)).wrapping_mul(2_654_435_761);
        (x >> 8) as f32 / (1 << 24) as f32
    };
    let colors = [GREEN, TEXT, GREEN_DIM, Color32::from_rgb(255, 214, 102)];
    let fade = ((LIFE - t) / (LIFE * 0.4)).min(1.0);
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("confetti"));
    let p = ui.ctx().layer_painter(layer);
    for i in 0..28 {
        // Up and out in a fan, then down.
        let angle = (-90.0 + (rnd(i, 1) - 0.5) * 110.0_f32).to_radians();
        let speed = 160.0 + rnd(i, 2) * 180.0;
        let pos = from + vec2(angle.cos(), angle.sin()) * speed * t + vec2(0.0, 420.0 * t * t);
        let spin = rnd(i, 3) * 6.0 + t * (4.0 + rnd(i, 4) * 8.0);
        let (c, s) = (spin.cos(), spin.sin());
        let corner = |x: f32, y: f32| pos + vec2(x * c - y * s, x * s + y * c);
        let color = colors[i as usize % colors.len()].gamma_multiply(fade);
        let pts = vec![
            corner(-3.0, -1.5),
            corner(3.0, -1.5),
            corner(3.0, 1.5),
            corner(-3.0, 1.5),
        ];
        p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
    }
    ui.ctx().request_repaint();
    true
}
