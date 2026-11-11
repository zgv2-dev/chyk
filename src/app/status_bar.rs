use super::*;

#[cfg(test)]
mod tests;

const HEIGHT: f32 = 26.0;

/// The facts that force a re-encode, as `App::stream_copy` reads them.
pub(super) struct ReasonInput {
    pub crop: bool,
    pub resize: bool,
    pub strip: bool,
    pub retime: bool,
    pub precise: bool,
    pub quality_high: bool,
}

/// i18n keys naming why the export can't stream-copy, in a fixed order.
pub(super) fn reencode_reasons(a: &ReasonInput) -> Vec<&'static str> {
    [
        (a.crop, "reason.crop"),
        (a.resize, "reason.resize"),
        (a.strip, "reason.strip"),
        (a.retime, "reason.speed"),
        (a.precise, "reason.precise"),
        (!a.quality_high, "reason.quality"),
    ]
    .into_iter()
    .filter_map(|(on, key)| on.then_some(key))
    .collect()
}

/// Time left at `frac` done after `elapsed`; `None` while too little is done to tell.
pub(super) fn time_left(elapsed: Duration, frac: f32) -> Option<Duration> {
    (frac >= 0.02).then(|| elapsed.mul_f32((1.0 - frac.min(1.0)) / frac))
}

pub(super) fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

impl App {
    fn status_text(&mut self) -> (String, Color32) {
        if self.job.is_some() {
            let frac = self.export_glide.tick().min(1.0) as f32;
            let name = self.export_started.as_ref().map_or("", |(_, n)| n.as_str());
            let mut s = t!(
                "status.exporting",
                name = name,
                pct = format!("{:.0}", frac * 100.0)
            )
            .to_string();
            if let Some(left) = self
                .export_started
                .as_ref()
                .and_then(|(at, _)| time_left(at.elapsed(), frac))
            {
                s += &format!(" · {}", t!("status.left", time = clock(left)));
            }
            return (s, GREEN);
        }
        if self.strip.job.is_some() {
            let (a, b) = self.strip.scan_range;
            let frac = ((self.strip.front.shown - a) / (b - a).max(1e-6)).clamp(0.0, 1.0);
            return (
                t!("strip.scanning", pct = format!("{:.0}", frac * 100.0)).to_string(),
                GREEN,
            );
        }
        if self.video.is_none() {
            return (t!("status.ready").to_string(), MUTED);
        }
        let reasons = reencode_reasons(&ReasonInput {
            crop: self.crop_active(),
            resize: self.scale(self.size).is_some(),
            strip: self.strip.enabled,
            retime: (self.speed - 1.0).abs() >= 1e-4,
            precise: self.precise,
            quality_high: self.quality == 0,
        });
        let mut s = t!("status.ready").to_string();
        if self.stream_copy(self.size) {
            s += &format!(" · {}", t!("export.copy"));
        } else {
            s += &format!(" · {}", t!("export.reencode"));
            let names: Vec<_> = reasons.iter().map(|k| i18n::tr(k)).collect();
            if !names.is_empty() {
                s += &format!(" · {}", names.join(", "));
            }
        }
        (s, MUTED)
    }

    /// The bottom-most bar, on the editor and the start screen: log, state, shortcuts hint, version.
    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        let (text, color) = self.status_text();
        let has_video = self.video.is_some();
        let bar = egui::Panel::bottom("status")
            .show_separator_line(false)
            .exact_size(HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    // Right group first so a long state text truncates instead of pushing it out.
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let ver = env!("CARGO_PKG_VERSION");
                        let v = if has_video {
                            format!("v{ver}")
                        } else {
                            format!("v{ver} · {}", t!("credit", name = "zgv2"))
                        };
                        ui.label(RichText::new(v).font(mono(11.0)).color(MUTED));
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(t!("status.shortcuts"))
                                .font(mono(11.0))
                                .color(MUTED),
                        );
                        keycaps(ui, &["?".to_string()]);
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let log = icon_button_look(
                                ui,
                                icon::TERMINAL_WINDOW,
                                t!("status.log"),
                                self.log_open,
                                ICON_SMALL,
                            );
                            tip_keys(log, t!("status.log"), Cmd::ToggleLog);
                            ui.add(
                                egui::Label::new(RichText::new(text).font(mono(11.0)).color(color))
                                    .truncate(),
                            );
                        });
                    });
                });
            });
        let r = bar.response.rect;
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0, LINE));
    }
}
