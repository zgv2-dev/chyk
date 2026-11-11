use super::*;

/// Candidate (minor, major) tick steps in seconds, finest first.
const STEPS: &[(f64, f64)] = &[
    (0.02, 0.1),
    (0.05, 0.25),
    (0.1, 0.5),
    (0.2, 1.0),
    (0.5, 2.0),
    (1.0, 5.0),
    (2.0, 10.0),
    (5.0, 15.0),
    (5.0, 30.0),
    (10.0, 60.0),
    (30.0, 120.0),
    (60.0, 300.0),
    (120.0, 600.0),
    (300.0, 900.0),
    (300.0, 1800.0),
    (600.0, 3600.0),
];

/// (minor, major) tick steps in seconds for a view of `span` seconds over `width` points: the
/// first whose major ticks sit at least 60 pt apart; minor ticks under 8 pt apart merge into major.
pub(super) fn ruler_steps(span: f64, width: f32) -> (f64, f64) {
    let per_pt = span / f64::from(width.max(1.0));
    let &(minor, major) = STEPS
        .iter()
        .find(|s| s.1 / per_pt >= 60.0)
        .unwrap_or(&STEPS[STEPS.len() - 1]);
    if minor / per_pt < 8.0 {
        (major, major)
    } else {
        (minor, major)
    }
}

/// A major tick's label: `m:ss` or `h:mm:ss`, with one fixed number of decimals per step.
fn tick_label(t: f64, major: f64) -> String {
    let s = fmt_time(t);
    let decimals = if major >= 1.0 {
        0
    } else if (major * 10.0).fract().abs() < 1e-9 {
        1
    } else {
        2
    };
    // Drop the milliseconds' tail, and the point when no decimals remain.
    s[..s.len() - 3 + decimals]
        .trim_end_matches('.')
        .to_string()
}

impl App {
    /// Time ticks and labels over the trim bar, following the timeline view.
    pub(super) fn paint_ruler(&self, ui: &Ui, area: Rect, tv: &TrimView) {
        let p = ui.painter_at(area);
        let (minor, major) = ruler_steps(tv.span, tv.strip.width());
        let k0 = (tv.v0 / minor).ceil() as i64;
        let k1 = (tv.v1 / minor).floor() as i64;
        let y = area.max.y - 2.0;
        for k in k0..=k1 {
            let t = k as f64 * minor;
            let x = tv.x(t);
            let is_major = ((t / major).round() * major - t).abs() < minor * 1e-3;
            let (h, c) = if is_major { (7.0, MUTED) } else { (4.0, LINE) };
            p.vline(x.round() + 0.5, y - h..=y, Stroke::new(1.0, c));
            if is_major {
                let g = p.layout_no_wrap(tick_label(t, major), mono(9.5), MUTED);
                // A label that would run past the right edge is skipped.
                if x + 3.0 + g.size().x <= area.max.x {
                    p.galley(pos2(x + 3.0, y - 8.0 - g.size().y), g, MUTED);
                }
            }
        }
    }

    /// The playhead's time on a white tag above the bar, kept inside the ruler's width.
    pub(super) fn paint_playhead_tag(&self, ui: &Ui, area: Rect, x: f32) {
        let g = ui
            .painter()
            .layout_no_wrap(fmt_time(self.pos()), mono(9.5), Color32::BLACK);
        let size = g.size() + vec2(10.0, 2.0);
        let cx = x.clamp(area.min.x + size.x / 2.0, area.max.x - size.x / 2.0);
        let tag = Rect::from_center_size(pos2(cx, area.max.y - size.y / 2.0 - 1.0), size);
        let p = ui.painter();
        p.rect_filled(tag, 4.0, Color32::WHITE);
        p.galley(tag.center() - g.size() / 2.0, g, Color32::BLACK);
    }
}

#[cfg(test)]
mod tests;
