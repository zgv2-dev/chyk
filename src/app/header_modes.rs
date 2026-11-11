use super::*;

impl App {
    /// Key caps on Crop, Strip and Export only while the header has room for them. Judged from the
    /// window width and the measured middle section, so showing them can't flip the verdict.
    pub(super) fn header_caps(&self, ctx: &egui::Context) -> bool {
        ctx.content_rect().width() - self.center_need >= 900.0
    }

    pub(super) fn mode_group(&mut self, ui: &mut Ui) {
        let show_caps = self.header_caps(ui.ctx());
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
