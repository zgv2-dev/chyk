use super::*;

impl App {
    pub(super) fn export_summary(&self, ui: &mut Ui, rate: &Rate) {
        ui.add_space(4.0);
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter()
            .hline(line.x_range(), line.center().y, Stroke::new(1.0, LINE));
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(5.0, 6.0);
            // Icon and text in one unwrapped label, so a full row moves them down together.
            let item = |ui: &mut Ui, glyph: &str, text: String, color: Color32| {
                let mut job = egui::text::LayoutJob::default();
                let format = |size, color| egui::TextFormat {
                    valign: Align::Center,
                    ..egui::TextFormat::simple(mono(size), color)
                };
                job.append(glyph, 0.0, format(13.0, GREEN));
                job.append(&text, 5.0, format(12.0, color));
                ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
                ui.add_space(10.0);
            };
            let retime = (self.speed - 1.0).abs() > 1e-4;
            match self.output_length() {
                Some(len) if retime => item(
                    ui,
                    icon::TIMER,
                    format!("{} · {}", fmt_time(len), speed_label(self.speed)),
                    TEXT,
                ),
                Some(len) => item(ui, icon::TIMER, fmt_time(len), TEXT),
                None => item(ui, icon::TIMER, t!("export.need_scan").into(), MUTED),
            }
            if let (Some(c), Some(v)) = (self.crop, &self.video) {
                let [w, h, _, _] = crop::to_pixels(c, v.size[0], v.size[1]);
                item(ui, icon::CROP, format!("{w}×{h}"), TEXT);
            }
            if let Some([w, h]) = self.scale(self.size) {
                item(ui, icon::FRAME_CORNERS, format!("{w}×{h}"), TEXT);
            }
            let note = self.size_note(self.size, rate);
            if !note.is_empty() {
                item(ui, icon::HARD_DRIVE, note, TEXT);
            }
            let method = if self.stream_copy(self.size) {
                t!("export.copy")
            } else {
                t!("export.reencode")
            };
            item(ui, icon::CPU, method.into(), TEXT);
        });
        if self.strip.enabled || (self.speed - 1.0).abs() > 1e-4 {
            ui.label(
                RichText::new(t!("export.retime_note"))
                    .font(mono(11.0))
                    .color(MUTED),
            );
        }
    }
}
