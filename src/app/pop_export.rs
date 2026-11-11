use super::*;

/// Width of the label column; the control fills the rest of the row.
const LABEL_W: f32 = 78.0;

/// Short encoder name for the quality note: "VideoToolbox", "x264".
fn encoder_label(enc: &str) -> String {
    let name = match enc.rsplit('_').next().unwrap_or(enc) {
        "videotoolbox" => "VideoToolbox",
        "nvenc" => "NVENC",
        "qsv" => "QSV",
        "amf" => "AMF",
        "mf" => "Media Foundation",
        "libx264" => "x264",
        "libx265" => "x265",
        "libopenh264" => "OpenH264",
        other => other,
    };
    name.into()
}

/// A label on the left, then `add` in the rest of the row.
fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, 36.0), Sense::hover());
        ui.painter().text(
            r.left_center(),
            Align2::LEFT_CENTER,
            label,
            mono(12.0),
            MUTED,
        );
        add(ui);
    });
}

impl App {
    /// "{hint} · {codec}, {encoder}" under the quality row; a stream copy names no encoder.
    fn quality_note(&self) -> String {
        let hint = match self.quality {
            0 => t!("quality.high_hint"),
            1 => t!("quality.medium_hint"),
            2 => t!("quality.low_hint"),
            _ => t!("quality.compress_hint"),
        };
        let Some(v) = &self.video else {
            return hint.into();
        };
        if self.stream_copy(self.size) {
            return format!("{hint} · {}", t!("export.copy"));
        }
        match ffmpeg::first_encoder(&v.codec, self.quality == ffmpeg::COMPRESS) {
            Some(e) => {
                let codec = if e.starts_with("hevc") || e == "libx265" {
                    "HEVC"
                } else {
                    "H.264"
                };
                format!("{hint} · {codec}, {}", encoder_label(e))
            }
            None => hint.into(),
        }
    }

    pub(super) fn export_pop(&mut self, ui: &mut Ui, ctx: &egui::Context, max_h: f32) {
        ui.spacing_mut().item_spacing.y = 8.0;
        self.pop_head(ui, icon::EXPORT, t!("export.title"), |_| {});
        let rate = match self.export_job(PathBuf::new()) {
            Some(job) => self.sampler.rate(job, ctx),
            None => Rate::Failed,
        };
        // Title and footer stay put; the settings between them scroll on short windows.
        egui::ScrollArea::vertical()
            .max_height((max_h - 50.0 - 52.0).max(60.0))
            .show(ui, |ui| {
                ui.add_space(6.0);
                let formats: Vec<String> =
                    ffmpeg::FORMATS.iter().map(|f| f.to_uppercase()).collect();
                row(ui, t!("export.format"), |ui| {
                    if let Some(i) = segmented(ui, "format", &formats, self.format) {
                        self.format = i;
                    }
                });
                let qualities: Vec<String> = ffmpeg::QUALITIES
                    .iter()
                    .map(|q| i18n::tr(q).to_string())
                    .collect();
                row(ui, t!("export.quality"), |ui| {
                    if let Some(i) = segmented(ui, "quality", &qualities, self.quality) {
                        self.quality = i;
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(LABEL_W + ui.spacing().item_spacing.x);
                    let note = RichText::new(self.quality_note())
                        .font(mono(11.0))
                        .color(MUTED);
                    ui.add(egui::Label::new(note).wrap_mode(egui::TextWrapMode::Wrap));
                });
                let sizes: Vec<String> = ffmpeg::SIZES
                    .iter()
                    .map(|s| s.1.map_or(t!("crop.original"), |_| s.0).to_string())
                    .collect();
                let notes: Vec<String> =
                    (0..sizes.len()).map(|i| self.size_note(i, &rate)).collect();
                row(ui, t!("export.resolution"), |ui| {
                    if let Some(i) = dropdown(ui, "size", &sizes, &notes, self.size) {
                        self.size = i;
                    }
                });
                let mac = cfg!(target_os = "macos");
                let (p, m) = (
                    caps(Modifiers::NONE, Key::P, mac),
                    caps(Modifiers::NONE, Key::M, mac),
                );
                let (precise, hint) = (t!("export.precise"), t!("export.precise_hint"));
                if toggle_row(ui, icon::CROSSHAIR, precise, hint, self.precise, &p) {
                    self.precise = !self.precise;
                }
                let (mute, hint) = (t!("export.mute"), t!("export.mute_hint"));
                if toggle_row(ui, icon::SPEAKER_SLASH, mute, hint, self.mute, &m) {
                    self.toggle_mute();
                }
                self.export_summary(ui, &rate);
            });
        ui.add_space(6.0);
        let mac = cfg!(target_os = "macos");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let esc = caps(Modifiers::NONE, Key::Escape, mac);
            if button_caps(ui, "", t!("common.cancel"), false, GHOST, &esc).clicked() {
                self.pop = None;
            }
            let save = caps(Modifiers::COMMAND, Key::S, mac);
            let w = ui.available_width();
            if primary_caps(
                ui,
                icon::DOWNLOAD_SIMPLE,
                t!("export.save"),
                vec2(w, 40.0),
                false,
                &save,
            )
            .clicked()
            {
                self.pop = None;
                self.export(ctx);
            }
        });
    }
}
