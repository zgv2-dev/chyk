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

/// Where to get an ffmpeg that can re-encode. Fedora's own ffmpeg-free leaves the patented
/// encoders out; RPM Fusion's ffmpeg has them.
pub(super) fn encoder_hint() -> &'static str {
    let fedora = cfg!(target_os = "linux")
        && std::fs::read_to_string("/etc/os-release").is_ok_and(|r| {
            r.lines().any(|l| {
                l.strip_prefix("ID=")
                    .or_else(|| l.strip_prefix("ID_LIKE="))
                    .is_some_and(|v| v.trim_matches('"').split(' ').any(|id| id == "fedora"))
            })
        });
    if fedora {
        t!("export.no_encoder_fedora")
    } else {
        t!("export.no_encoder_install")
    }
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
    /// True when ffmpeg can't re-encode (see [`ffmpeg::can_encode`]): Quality, Resolution and
    /// Precise go back to the choices a lossless copy allows.
    pub(super) fn copy_only(&mut self, ctx: &egui::Context) -> bool {
        let only = ffmpeg::can_encode(ctx) == Some(false);
        if only {
            (self.quality, self.size, self.precise) = (0, 0, false);
        }
        only
    }

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
        let copy_only = self.copy_only(ctx);
        // Crop, speed or Strip still ask for a re-encode: nothing can save this export.
        let blocked = copy_only && !self.stream_copy(self.size);
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
                let enabled: Vec<bool> =
                    (0..qualities.len()).map(|i| i == 0 || !copy_only).collect();
                row(ui, t!("export.quality"), |ui| {
                    if let Some(i) =
                        segmented_some(ui, "quality", &qualities, self.quality, &enabled)
                    {
                        self.quality = i;
                    }
                });
                let note = if copy_only {
                    format!("{} {}", t!("export.no_encoder"), encoder_hint())
                } else {
                    self.quality_note()
                };
                ui.horizontal(|ui| {
                    ui.add_space(LABEL_W + ui.spacing().item_spacing.x);
                    let color = if blocked { RED } else { MUTED };
                    let note = RichText::new(note).font(mono(11.0)).color(color);
                    ui.add(egui::Label::new(note).wrap_mode(egui::TextWrapMode::Wrap));
                });
                let sizes: Vec<String> = ffmpeg::SIZES
                    .iter()
                    .map(|s| s.1.map_or(t!("crop.original"), |_| s.0).to_string())
                    .collect();
                let notes: Vec<String> =
                    (0..sizes.len()).map(|i| self.size_note(i, &rate)).collect();
                let enabled: Vec<bool> = (0..sizes.len()).map(|i| i == 0 || !copy_only).collect();
                row(ui, t!("export.resolution"), |ui| {
                    if let Some(i) = dropdown_some(ui, "size", &sizes, &notes, self.size, &enabled)
                    {
                        self.size = i;
                    }
                });
                let mac = cfg!(target_os = "macos");
                let (p, m) = (
                    caps(Modifiers::NONE, Key::P, mac),
                    caps(Modifiers::NONE, Key::M, mac),
                );
                let (precise, hint) = (t!("export.precise"), t!("export.precise_hint"));
                ui.add_enabled_ui(!copy_only, |ui| {
                    if toggle_row(ui, icon::CROSSHAIR, precise, hint, self.precise, &p) {
                        self.precise = !self.precise;
                    }
                });
                let (mute, hint) = (t!("export.mute"), t!("export.mute_hint"));
                if toggle_row(ui, icon::SPEAKER_SLASH, mute, hint, self.mute, &m) {
                    self.toggle_mute();
                }
                self.export_summary(ui, &rate);
            });
        ui.add_space(6.0);
        let mac = cfg!(target_os = "macos");
        // A fixed-height row: `horizontal` centers each item on the row as tall as it is so far,
        // which left Cancel above Save's middle.
        let row = vec2(ui.available_width(), 40.0);
        ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let esc = caps(Modifiers::NONE, Key::Escape, mac);
            if button_caps(ui, "", t!("common.cancel"), false, GHOST, &esc).clicked() {
                self.pop = None;
            }
            let save = caps(Modifiers::COMMAND, Key::S, mac);
            let need = ui
                .painter()
                .layout_no_wrap(t!("export.save").into(), bold(12.5), INK)
                .size()
                .x
                + 56.0;
            let w = ui.available_width();
            // Caps give way when the label doesn't fit beside Cancel.
            let save = if w >= need + caps_width(ui.painter(), &save) + 8.0 {
                save
            } else {
                vec![]
            };
            let saved = ui
                .add_enabled_ui(!blocked, |ui| {
                    let label = t!("export.save");
                    primary_caps(
                        ui,
                        icon::DOWNLOAD_SIMPLE,
                        label,
                        vec2(w, 40.0),
                        false,
                        &save,
                    )
                })
                .inner;
            if saved.clicked() {
                self.pop = None;
                self.export(ctx);
            }
        });
    }
}
