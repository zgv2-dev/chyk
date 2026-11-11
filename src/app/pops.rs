use super::*;

impl App {
    /// P and M flip Precise and Mute while the export popover is open.
    pub(super) fn export_keys(&mut self, ctx: &egui::Context) {
        if self.pop != Some(Pop::Export) {
            return;
        }
        let key = |k| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        if key(Key::P) {
            self.precise = !self.precise;
        }
        if key(Key::M) {
            self.toggle_mute();
        }
    }

    pub(super) fn toggle_mute(&mut self) {
        self.mute = !self.mute;
        let _ = self.player.mpv.set_property("mute", self.mute);
    }

    pub(super) fn info_pop(&mut self, ui: &mut Ui, ctx: &egui::Context, max_h: f32) {
        let Some(v) = &self.video else { return };
        let name = v.path.file_name().map_or_else(
            || t!("file.untitled").to_string(),
            |n| n.to_string_lossy().into(),
        );
        if self.meta.is_none() {
            self.meta = Some(meta::read(&self.player, &v.path, v.dur, v.size));
        }
        let text = meta::to_text(&name, self.meta.as_deref().unwrap_or_default());
        self.pop_head(ui, icon::INFO, &name, |ui| {
            if icon_button(ui, icon::COPY, t!("meta.copy"), false).clicked() {
                ctx.copy_text(text);
            }
        });
        let sections = self.meta.as_deref().unwrap_or_default();
        // Grids read the minimum row height when created; the default 28 pt control height
        // would space the rows far apart.
        ui.spacing_mut().interact_size.y = 16.0;
        egui::ScrollArea::vertical()
            .max_height(max_h.max(120.0))
            .show(ui, |ui| {
                for (n, sec) in sections.iter().enumerate() {
                    ui.add_space(8.0);
                    let (line, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter()
                        .hline(line.x_range(), line.center().y, Stroke::new(1.0, LINE));
                    ui.add_space(6.0);
                    field_label(ui, sec.icon, &sec.title);
                    egui::Grid::new(("meta", n))
                        .num_columns(2)
                        .min_col_width(110.0)
                        .spacing([10.0, 5.0])
                        .show(ui, |ui| {
                            for (k, val) in &sec.rows {
                                ui.label(RichText::new(k).font(mono(12.0)).color(MUTED));
                                ui.label(RichText::new(val).font(mono(12.0)).color(TEXT));
                                ui.end_row();
                            }
                        });
                }
            });
    }
}
