//! A join on the timeline: a lane of clip tabs over the ruler (select, remove), and on the
//! filmstrip the seams, each clip's cut parts dimmed and the other clips' kept parts outlined.

use super::*;

const LANE_H: f32 = 22.0;
/// Amber for a clip that is scaled or barred into the frame.
const WARN: Color32 = Color32::from_rgb(255, 200, 97);

/// The fit's name.
pub(in crate::app) fn fit_label(fit: ffmpeg::Fit) -> &'static str {
    match fit {
        ffmpeg::Fit::Fit => t!("join.fit"),
        ffmpeg::Fit::Fill => t!("join.fill"),
        ffmpeg::Fit::Stretch => t!("join.stretch"),
        ffmpeg::Fit::Place(_) => t!("join.place"),
    }
}

impl App {
    /// Clip tabs spanning each clip's place on the timeline.
    pub(in crate::app) fn clip_lane(&mut self, ui: &mut Ui, w: f32, dur: f64, view: (f64, f64)) {
        let Some(r) = self.reel_now() else { return };
        let (rect, _) = ui.allocate_exact_size(vec2(w, LANE_H), Sense::hover());
        let tv = TrimView::new(rect, dur, view, self.trim);
        let (mut pick, mut drop) = (None, None);
        for (i, c) in r.clips.iter().enumerate() {
            let (a, b) = r.bounds(i);
            let (x0, x1) = (tv.x(a).max(tv.strip.min.x), tv.x(b).min(tv.strip.max.x));
            if x1 - x0 < 6.0 {
                continue;
            }
            let tab = Rect::from_x_y_ranges(x0 + 1.0..=x1 - 1.0, rect.y_range());
            let resp = ui.interact(tab, ui.id().with(("clip_tab", i)), Sense::CLICK);
            let on = i == r.sel;
            let p = ui.painter_at(tab);
            let round = egui::CornerRadius {
                nw: 6,
                ne: 6,
                sw: 0,
                se: 0,
            };
            let (fill, edge) = if on {
                (Color32::from_rgba_unmultiplied(77, 255, 155, 20), GREEN_DIM)
            } else {
                (if resp.hovered() { CARD_HI } else { CARD }, LINE)
            };
            p.rect(
                tab,
                round,
                fill,
                Stroke::new(1.0, edge),
                egui::StrokeKind::Inside,
            );
            let cy = tab.center().y;
            let badge = Rect::from_center_size(pos2(tab.min.x + 13.0, cy), vec2(16.0, 15.0));
            p.rect_filled(badge, 4.0, if on { GREEN } else { CARD_HI });
            let ink = if on { INK } else { MUTED };
            p.text(
                badge.center(),
                Align2::CENTER_CENTER,
                (i + 1).to_string(),
                mono(10.0),
                ink,
            );
            // Right to left: remove (on hover), kept length, the fit hint; the name takes the rest.
            let mut right = tab.max.x - 6.0;
            if resp.hovered() || on {
                let x = Rect::from_center_size(pos2(right - 7.0, cy), vec2(14.0, 14.0));
                let hx = ui.interact(x, ui.id().with(("clip_x", i)), Sense::CLICK);
                let col = if hx.hovered() { RED } else { MUTED };
                p.text(x.center(), Align2::CENTER_CENTER, icon::X, mono(11.0), col);
                if hx.on_hover_text(t!("join.remove")).clicked() {
                    drop = Some(i);
                }
                right -= 18.0;
            }
            let len = fmt_time(c.trim.1 - c.trim.0);
            let lr = p.text(
                pos2(right, cy),
                Align2::RIGHT_CENTER,
                len,
                mono(10.5),
                MUTED,
            );
            right = lr.min.x - 8.0;
            if c.info.size != r.frame {
                let [cw, ch] = c.info.size;
                let hint = format!("{} {cw}×{ch} · {}", icon::WARNING_CIRCLE, fit_label(c.fit));
                let hw = ui
                    .painter()
                    .layout_no_wrap(hint.clone(), mono(10.0), WARN)
                    .size()
                    .x;
                if right - hw > badge.max.x + 40.0 {
                    p.text(
                        pos2(right, cy),
                        Align2::RIGHT_CENTER,
                        hint,
                        mono(10.0),
                        WARN,
                    );
                    right -= hw + 8.0;
                }
            }
            let name = c.path.file_name().unwrap_or_default().to_string_lossy();
            let clip = Rect::from_x_y_ranges(
                badge.max.x + 6.0..=right.max(badge.max.x + 6.0),
                tab.y_range(),
            );
            ui.painter_at(clip).text(
                pos2(clip.min.x, cy),
                Align2::LEFT_CENTER,
                name,
                mono(11.0),
                if on { GREEN } else { TEXT },
            );
            let tip = format!(
                "{}\n{} · {}×{} · {:.3} fps · {}",
                c.path.display(),
                codec_label(&c.info.codec),
                c.info.size[0],
                c.info.size[1],
                c.info.fps,
                fmt_time(c.info.dur)
            );
            if resp.on_hover_text(tip).clicked() {
                pick = Some(i);
            }
        }
        if let Some(i) = drop {
            self.remove_clip(i);
        } else if let Some(i) = pick {
            self.select_clip(i);
        }
    }

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
            if i != r.sel && x(kb) > x(ka) {
                let k = band(ka, kb).intersect(strip.expand2(vec2(0.0, 1.0)));
                let ghost = Color32::from_rgba_unmultiplied(77, 255, 155, 110);
                p.rect_stroke(k, 5.0, Stroke::new(1.0, ghost), egui::StrokeKind::Inside);
            }
            if i > 0 && strip.x_range().contains(tv.x(r.start(i))) {
                p.vline(tv.x(r.start(i)), rr.y_range(), Stroke::new(3.0, PANEL));
            }
        }
    }

    /// Each clip's box on the filmstrip, with the tile each slot shows in it. A slot sampled
    /// in another clip borrows its neighbour from this one, so no frame crosses a seam.
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
