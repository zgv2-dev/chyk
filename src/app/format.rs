use super::*;

/// Editable time: drag to nudge, click to type. Returns true when the value changed.
pub(super) fn time_field(
    ui: &mut Ui,
    t: &mut f64,
    max: f64,
    prefix: impl Into<egui::WidgetText>,
    hint: &str,
) -> bool {
    ui.add(
        egui::DragValue::new(t)
            .range(0.0..=max)
            .speed(0.01)
            .prefix(prefix.into())
            .custom_formatter(|t, _| fmt_time(t))
            .custom_parser(parse_time)
            // Half-typed values like "1" would otherwise move the trim.
            .update_while_editing(false),
    )
    .on_hover_text(hint)
    .changed()
}

/// `trim` moved to start at `t`, same length, kept inside `0..=dur`.
pub(super) fn slide(trim: (f64, f64), t: f64, dur: f64) -> (f64, f64) {
    let len = trim.1 - trim.0;
    let a = t.min(dur - len).max(0.0);
    (a, a + len)
}

pub(crate) fn fmt_time(t: f64) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{f:03}")
    } else {
        format!("{m}:{s:02}.{f:03}")
    }
}

/// Display name of crop preset `i`; ratios like 16:9 read the same in every language.
pub(super) fn preset_label(i: usize) -> &'static str {
    match crop::PRESETS[i].0 {
        "Free" => t!("crop.free"),
        "Original" => t!("crop.original"),
        ratio => ratio,
    }
}

/// Preset application: Free keeps the current crop; aspect presets fit a new centered rect.
pub(super) fn crop_for_preset(preset: usize, current: Rect, lock: Option<f32>) -> Rect {
    if crop::PRESETS[preset].1.is_none() {
        current
    } else {
        lock.map_or_else(crop::full, crop::fit)
    }
}

/// Codec as people write it: "H.264", "HEVC", "ProRes".
pub(super) fn codec_label(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" | "h265" => "HEVC".into(),
        "prores" => "ProRes".into(),
        "mpeg4" => "MPEG-4".into(),
        "mpeg2video" => "MPEG-2".into(),
        other => other.to_uppercase(),
    }
}

/// "100%", "250%".
pub(super) fn zoom_label(z: f32) -> String {
    format!("{:.0}%", z * 100.0)
}

/// "0.5x", "1x", "1.25x".
pub(super) fn speed_label(speed: f64) -> String {
    let mut text = format!("{speed:.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    format!("{text}x")
}

/// "1536" → "1.5 KB".
pub(crate) fn fmt_size(n: u64) -> String {
    let units = [t!("size.b"), t!("size.kb"), t!("size.mb"), t!("size.gb")];
    let mut v = n as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < units.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} {}", units[0])
    } else {
        format!("{v:.1} {}", units[unit])
    }
}

/// "≈ 1.5 GB", for size estimates.
pub(super) fn approx(bytes: u64) -> String {
    format!("≈ {}", fmt_size(bytes))
}

/// Seconds from "s", "m:s" or "h:m:s", each with optional decimals.
pub(super) fn parse_time(s: &str) -> Option<f64> {
    let parts: Vec<_> = s.trim().split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let t = parts.iter().try_fold(0.0, |acc, p| {
        Some(acc * 60.0 + p.trim().parse::<f64>().ok()?)
    })?;
    (t.is_finite() && t >= 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_keeps_length() {
        assert_eq!(slide((2.0, 5.0), 4.0, 10.0), (4.0, 7.0));
        assert_eq!(slide((2.0, 5.0), 9.0, 10.0), (7.0, 10.0));
        assert_eq!(slide((2.0, 5.0), -1.0, 10.0), (0.0, 3.0));
    }

    #[test]
    fn free_preset_keeps_crop_rect() {
        let r = Rect::from_min_max(pos2(0.2, 0.3), pos2(0.6, 0.8));
        assert_eq!(crop_for_preset(0, r, None), r);
        assert_eq!(
            crop_for_preset(2, r, Some(16.0 / 9.0)),
            crop::fit(16.0 / 9.0)
        );
    }

    #[test]
    fn speed_text() {
        assert_eq!(speed_label(0.25), "0.25x");
        assert_eq!(speed_label(0.5), "0.5x");
        assert_eq!(speed_label(1.0), "1x");
        assert_eq!(speed_label(1.25), "1.25x");
        assert_eq!(speed_label(4.0), "4x");
        assert_eq!(speed_label(1.333), "1.33x");
    }

    #[test]
    fn size_format() {
        assert_eq!(fmt_size(999), "999 B");
        assert_eq!(fmt_size(1536), "1.5 KB");
        assert_eq!(fmt_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(fmt_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn time_round_trip() {
        assert_eq!(fmt_time(0.0), "0:00.000");
        assert_eq!(fmt_time(59.9996), "1:00.000");
        assert_eq!(fmt_time(3723.0456), "1:02:03.046");
        assert_eq!(parse_time("1:02:03.046"), Some(3723.046));
        assert_eq!(parse_time(" 2:05.5 "), Some(125.5));
        assert_eq!(parse_time("90.25"), Some(90.25));
        for bad in ["", "x", "1:2:3:4", "nan", "-1", "1::2"] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
    }
}
