use super::*;

#[cfg(test)]
mod tests;

/// Extra room asked before turning the caps on, so the header doesn't flicker at the boundary.
const CAPS_HYSTERESIS: f32 = 24.0;

/// Show the caps when the slack (measured with them hidden) covers what they add; once shown,
/// keep them until it no longer does, and only turn them back on with `CAPS_HYSTERESIS` to spare.
pub(super) fn caps_fit(slack_hidden: f32, extra: f32, shown: bool) -> bool {
    // An overflowing section reads as zero slack (egui clamps its measured width), so exactly
    // `extra` left while shown means it no longer fits.
    if shown {
        slack_hidden > extra
    } else {
        slack_hidden >= extra + CAPS_HYSTERESIS
    }
}

impl App {
    /// What all the header's caps add together.
    pub(super) fn caps_extra(&self, p: &egui::Painter) -> f32 {
        // No Export button (so no caps on it) while its progress pill shows.
        let cmds = if self.job.is_some() {
            vec![Cmd::Crop, Cmd::Strip]
        } else {
            vec![Cmd::Crop, Cmd::Strip, Cmd::Export]
        };
        cmds.into_iter()
            .map(|c| caps_width(p, &cmd_caps(c)) + 8.0)
            .sum()
    }

    /// Decide next frame's caps from this frame's measured slack in the header's middle.
    /// `slack`: width left beside the middle section; `extra`: what all the caps add together.
    pub(super) fn update_header_caps(&mut self, ui: &Ui, slack: f32) {
        let extra = self.caps_extra(ui.painter());
        // Slack as it would be with the caps hidden.
        let hidden = slack + if self.header_caps { extra } else { 0.0 };
        let on = caps_fit(hidden, extra, self.header_caps);
        if on != self.header_caps {
            self.header_caps = on;
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn mode_group(&mut self, ui: &mut Ui) {
        let show_caps = self.header_caps;
        let look = Look {
            height: (HEADER_ROW - 4.0).min(MODE.height),
            ..MODE
        };
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
                let resp = button_caps(
                    ui,
                    icon::CROP,
                    t!("mode.crop"),
                    crop_open,
                    look,
                    &caps(Cmd::Crop),
                );
                applied_dot(ui, resp.rect, self.crop.is_some() && !crop_open);
                if resp.clicked() {
                    self.toggle_crop(&ctx);
                }
                let strip_open = self.panel == Some(Panel::Strip);
                let resp = button_caps(
                    ui,
                    icon::MAGIC_WAND,
                    t!("mode.strip"),
                    strip_open,
                    look,
                    &caps(Cmd::Strip),
                );
                applied_dot(ui, resp.rect, self.strip.enabled && !strip_open);
                if resp.clicked() {
                    self.toggle_strip(&ctx);
                }
            });
    }
}
