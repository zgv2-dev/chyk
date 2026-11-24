use super::*;

#[cfg(test)]
mod tests;

/// Extra room asked before turning the caps on, so the header doesn't flicker at the boundary.
const CAPS_HYSTERESIS: f32 = 24.0;

/// Room asked beyond what a squeeze level gave before taking it back. Smaller than the caps':
/// a level gives back a fixed width, so any margin keeps it from flickering.
const SQUEEZE_HYSTERESIS: f32 = 8.0;

/// Show the caps when the slack (measured with them hidden) covers what they add; once shown,
/// keep them until it no longer does, and only turn them back on with `CAPS_HYSTERESIS` to spare.
pub(super) fn caps_fit(slack_hidden: f32, extra: f32, shown: bool) -> bool {
    // Exactly `extra` left while shown counts as no longer fitting, to stay off the boundary.
    if shown {
        slack_hidden > extra
    } else {
        slack_hidden >= extra + CAPS_HYSTERESIS
    }
}

/// How far the header gives way so its middle section fits: 0 nothing, 1 the running export
/// drops its time left, 2 the mode buttons also drop their labels. `overflow`: how much wider
/// the middle is than its room with nothing given (negative when there's room to spare);
/// `savings`: the width each level gives back; `now`: the current level. Going back down needs
/// `SQUEEZE_HYSTERESIS` to spare, so the header doesn't flicker at a boundary.
pub(super) fn squeeze_level(overflow: f32, savings: &[f32], now: u8) -> u8 {
    let mut left = overflow;
    for (level, saving) in (0u8..).zip(savings) {
        let spare = if level < now { SQUEEZE_HYSTERESIS } else { 0.0 };
        if left + spare <= 0.0 {
            return level;
        }
        left -= saving;
    }
    savings.len() as u8
}

/// The mode buttons' look: as tall as the header row allows.
fn mode_look() -> Look {
    Look {
        height: (HEADER_ROW - 4.0).min(MODE.height),
        ..MODE
    }
}

impl App {
    /// What all the header's caps add together.
    pub(super) fn caps_extra(&self, p: &egui::Painter) -> f32 {
        // No Export button (so no caps on it) while its progress pill shows.
        let cmds = if self.job.is_some() {
            vec![Cmd::Crop, Cmd::Strip, Cmd::Join]
        } else {
            vec![Cmd::Crop, Cmd::Strip, Cmd::Join, Cmd::Export]
        };
        cmds.into_iter()
            .map(|c| caps_width(p, &cmd_caps(c)) + 8.0)
            .sum()
    }

    /// Width the header gives back at each squeeze level, in order: the export's time left,
    /// then the mode labels.
    fn squeeze_savings(&self, p: &egui::Painter) -> [f32; 2] {
        let chip = if self.job.is_some() {
            export_chip_width(p, false) - export_chip_width(p, true)
        } else {
            0.0
        };
        let look = mode_look();
        let labels = [
            (icon::CROP, t!("mode.crop")),
            (icon::MAGIC_WAND, t!("mode.strip")),
            (icon::STACK, t!("mode.join")),
        ]
        .into_iter()
        .map(|(g, l)| button_width(p, g, l, look) - button_width(p, g, "", look))
        .sum();
        [chip, labels]
    }

    /// Decide next frame's caps and squeeze from this frame's middle section: `avail` is the
    /// width it had, `need` the width it took (more than `avail` when it overflowed).
    pub(super) fn update_header_fit(&mut self, ui: &Ui, avail: f32, need: f32) {
        // A bar too wide for its room redraws compact; judge the redrawn one.
        if ui.ctx().will_discard() {
            return;
        }
        let p = ui.painter();
        let extra = self.caps_extra(p);
        let savings = self.squeeze_savings(p);
        let given: f32 = savings[..usize::from(self.header_squeeze)].iter().sum();
        // Overflow as it would be with no caps and nothing squeezed.
        let caps = if self.header_caps { extra } else { 0.0 };
        let overflow = need - avail - caps + given;
        let squeeze = squeeze_level(overflow, &savings, self.header_squeeze);
        // Caps are the first to go: only with nothing squeezed.
        let caps = squeeze == 0 && caps_fit(-overflow, extra, self.header_caps);
        if (caps, squeeze) != (self.header_caps, self.header_squeeze) {
            (self.header_caps, self.header_squeeze) = (caps, squeeze);
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn mode_group(&mut self, ui: &mut Ui) {
        let show_caps = self.header_caps;
        let icons = self.header_squeeze >= 2;
        let look = mode_look();
        egui::Frame::new()
            .fill(CARD)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(2))
            .show(ui, |ui| {
                // Without this the frame grows to the header row's full height.
                ui.set_height(look.height);
                ui.spacing_mut().item_spacing.x = 2.0;
                let ctx = ui.ctx().clone();
                let crop_open = self.panel == Some(Panel::Crop);
                let caps = |c| if show_caps { cmd_caps(c) } else { Vec::new() };
                let resp = mode_button(
                    ui,
                    icon::CROP,
                    t!("mode.crop"),
                    crop_open,
                    look,
                    (icons, &caps(Cmd::Crop)),
                );
                applied_dot(ui, resp.rect, self.crop.is_some() && !crop_open);
                if resp.clicked() {
                    self.toggle_crop(&ctx);
                }
                let strip_open = self.panel == Some(Panel::Strip);
                let resp = mode_button(
                    ui,
                    icon::MAGIC_WAND,
                    t!("mode.strip"),
                    strip_open,
                    look,
                    (icons, &caps(Cmd::Strip)),
                );
                applied_dot(ui, resp.rect, self.strip.enabled && !strip_open);
                if resp.clicked() {
                    self.toggle_strip(&ctx);
                }
                self.join_mode_button(ui, look, (icons, &caps(Cmd::Join)));
            });
    }
}

/// A mode button; `icons` (from `squeeze`) leaves out its label and shows it as a tooltip.
pub(super) fn mode_button(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    open: bool,
    look: Look,
    (icons, caps): (bool, &[String]),
) -> Response {
    if !icons {
        return button_caps(ui, glyph, label, open, look, caps);
    }
    let resp = button_caps(ui, glyph, "", open, look, caps);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, label));
    resp.on_hover_text(label)
}
