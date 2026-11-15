//! A join on the filmstrip: the seams, each clip's cut parts dimmed, the other clips' kept parts
//! outlined, and each clip's own tiles.

use super::*;

impl App {
    /// Seams between clips, each clip's cut parts dimmed, other clips' kept parts outlined.
    pub(in crate::app) fn paint_reel(&self, p: &egui::Painter, tv: &TrimView) {
        let Some(r) = self.reel_now() else { return };
        let (rr, strip) = (tv.r, tv.strip);
        let x = |t: f64| tv.x(t).clamp(rr.min.x, rr.max.x);
        let band = |a: f64, b: f64| Rect::from_x_y_ranges(x(a)..=x(b), rr.y_range());
        for i in 0..r.clips.len() {
            let ((a, b), (ka, kb)) = (r.bounds(i), r.kept_range(i));
            let (a, b) = (
                if i == 0 { tv.v0.min(a) } else { a },
                if i + 1 == r.clips.len() {
                    tv.v1.max(b)
                } else {
                    b
                },
            );
            if ka > a {
                p.rect_filled(band(a, ka), 0.0, DIM);
            }
            if kb < b {
                p.rect_filled(band(kb, b), 0.0, DIM);
            }
            if i > 0 && strip.x_range().contains(tv.x(r.start(i))) {
                p.vline(tv.x(r.start(i)), rr.y_range(), Stroke::new(3.0, PANEL));
            }
        }
    }

    /// The other clips' kept parts, over the seams so an edge on a seam stays visible. An
    /// outline that reaches an end of the filmstrip follows its rounded corners.
    pub(in crate::app) fn paint_ghosts(&self, p: &egui::Painter, tv: &TrimView) {
        let Some(r) = self.reel_now() else { return };
        let strip = tv.strip;
        let ghost = Color32::from_rgba_unmultiplied(77, 255, 155, 110);
        for i in (0..r.clips.len()).filter(|&i| i != r.sel) {
            let (ka, kb) = r.kept_range(i);
            let k = Rect::from_x_y_ranges(tv.x(ka)..=tv.x(kb), strip.y_range()).intersect(strip);
            let end = |x: f32, at: f32| {
                if (x - at).abs() < 0.5 {
                    STRIP_R as u8
                } else {
                    0
                }
            };
            let (w, e) = (end(k.min.x, strip.min.x), end(k.max.x, strip.max.x));
            let round = egui::CornerRadius {
                nw: w,
                sw: w,
                ne: e,
                se: e,
            };
            if k.width() > 0.0 {
                p.rect_stroke(k, round, Stroke::new(1.0, ghost), egui::StrokeKind::Inside);
            }
        }
    }

    /// Each clip's box on the filmstrip and its slots' tiles: no frame crosses a seam.
    pub(in crate::app) fn tile_lanes(&self, tv: &TrimView) -> Vec<(Rect, Vec<Option<usize>>)> {
        let Some(r) = self.reel_now() else {
            return vec![(tv.r, (0..THUMBS).map(Some).collect())];
        };
        let step = (tv.v1 - tv.v0) / THUMBS as f64;
        let owner: Vec<usize> = (0..THUMBS)
            .map(|i| r.at(tv.v0 + (i as f64 + 0.5) * step))
            .collect();
        (0..r.clips.len())
            .map(|c| {
                let (a, b) = r.bounds(c);
                let xs = tv.x(a).max(tv.r.min.x)..=tv.x(b).min(tv.r.max.x);
                let mine = |j: usize| (owner.get(j) == Some(&c)).then_some(j);
                let tiles = (0..THUMBS)
                    .map(|i| {
                        mine(i)
                            .or_else(|| mine(i.wrapping_sub(1)))
                            .or_else(|| mine(i + 1))
                    })
                    .collect();
                (Rect::from_x_y_ranges(xs, tv.r.y_range()), tiles)
            })
            .collect()
    }
}
