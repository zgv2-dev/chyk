use super::*;

#[cfg(test)]
mod tests;

/// One edit that differs from the last export.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Chip {
    Trim(f64, f64),
    Crop(usize),
    /// The zone count with Strip on; `None`: turned off.
    Strip(Option<usize>),
    Speed(f64),
}

/// The edits in `now` that differ from `saved`, in trim, crop, strip, speed order.
pub(super) fn changed(saved: &Edits, now: &Edits, preset: usize) -> Vec<Chip> {
    let mut out = vec![];
    if saved.0 != now.0 {
        out.push(Chip::Trim(now.0.0, now.0.1));
    }
    if saved.1 != now.1 {
        out.push(Chip::Crop(preset));
    }
    if saved.2 != now.2 {
        out.push(Chip::Strip(now.2.as_ref().map(|s| s.0.len())));
    }
    if saved.3 != now.3 {
        out.push(Chip::Speed(now.3));
    }
    out
}

impl Chip {
    fn parts(&self, crop_removed: bool) -> (&'static str, String) {
        match *self {
            Chip::Trim(a, b) => (icon::SCISSORS, format!("{} – {}", fmt_time(a), fmt_time(b))),
            Chip::Crop(_) if crop_removed => (icon::CROP, t!("crop.none").into()),
            Chip::Crop(p) => (icon::CROP, preset_label(p).into()),
            Chip::Strip(None) => (icon::MAGIC_WAND, t!("chip.strip_off").into()),
            Chip::Strip(Some(0)) => (icon::MAGIC_WAND, t!("mode.strip").into()),
            Chip::Strip(Some(n)) => (icon::MAGIC_WAND, tn!("strip.zones", n).to_string()),
            Chip::Speed(s) => (icon::GAUGE, speed_label(s)),
        }
    }
}

/// A row of chips, one per changed edit.
pub(super) fn chips_row(ui: &mut Ui, chips: &[Chip], crop_removed: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for chip in chips {
            let (glyph, text) = chip.parts(crop_removed);
            egui::Frame::new()
                .fill(CARD)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(7.0)
                .inner_margin(egui::Margin::symmetric(9, 5))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(glyph).font(mono(12.0)).color(GREEN));
                        ui.label(RichText::new(text).font(mono(11.5)).color(TEXT));
                    });
                });
        }
    });
}
