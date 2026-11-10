use super::*;

impl App {
    pub(super) fn export_pop(&mut self, ui: &mut Ui, ctx: &egui::Context, max_h: f32) {
        ui.spacing_mut().item_spacing.y = 8.0;
        self.pop_head(ui, icon::EXPORT, t!("export.title"), |_| {});
        let rate = match self.export_job(PathBuf::new()) {
            Some(job) => self.sampler.rate(job, ctx),
            None => Rate::Failed,
        };
        // Title and Save stay put; the settings between them scroll on short windows.
        egui::ScrollArea::vertical()
            .max_height((max_h - 50.0 - 52.0).max(60.0))
            .show(ui, |ui| {
                ui.add_space(6.0);
                field_label(ui, icon::PACKAGE, t!("export.format"));
                let formats: Vec<String> =
                    ffmpeg::FORMATS.iter().map(|f| f.to_uppercase()).collect();
                if let Some(i) = segmented(ui, "format", &formats, self.format) {
                    self.format = i;
                }
                ui.add_space(8.0);
                field_label(ui, icon::SLIDERS_HORIZONTAL, t!("export.quality"));
                let qualities: Vec<String> = ffmpeg::QUALITIES
                    .iter()
                    .map(|q| i18n::tr(q).to_string())
                    .collect();
                if let Some(i) = segmented(ui, "quality", &qualities, self.quality) {
                    self.quality = i;
                }
                ui.add_space(8.0);
                field_label(ui, icon::FRAME_CORNERS, t!("export.resolution"));
                let sizes: Vec<String> = ffmpeg::SIZES
                    .iter()
                    .map(|s| s.1.map_or(t!("crop.original"), |_| s.0).to_string())
                    .collect();
                let notes: Vec<String> =
                    (0..sizes.len()).map(|i| self.size_note(i, &rate)).collect();
                if let Some(i) = dropdown(ui, "size", &sizes, &notes, self.size) {
                    self.size = i;
                }
                ui.add_space(8.0);
                if toggle_row(
                    ui,
                    icon::CROSSHAIR,
                    t!("export.precise"),
                    t!("export.precise_hint"),
                    self.precise,
                ) {
                    self.precise = !self.precise;
                }
                if toggle_row(
                    ui,
                    icon::SPEAKER_SLASH,
                    t!("export.mute"),
                    t!("export.mute_hint"),
                    self.mute,
                ) {
                    self.mute = !self.mute;
                    let _ = self.player.mpv.set_property("mute", self.mute);
                }
                ui.add_space(4.0);
                let (line, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
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
                    let note = self.size_note(self.size, &rate);
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
            });
        ui.add_space(6.0);
        let w = ui.available_width();
        if primary(
            ui,
            icon::DOWNLOAD_SIMPLE,
            t!("export.save"),
            vec2(w, 40.0),
            false,
        )
        .clicked()
        {
            self.pop = None;
            self.export(ctx);
        }
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
