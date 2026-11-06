use super::*;

impl App {
    /// Filmstrip dimmed outside the selection, strip cuts, the trim box and the playhead.
    pub(super) fn paint_trim(&mut self, ui: &Ui, tv: &TrimView) {
        let TrimView {
            r,
            strip,
            v0,
            v1,
            xi,
            xo,
            in_pin,
            out_pin,
            in_hr,
            out_hr,
            in_show,
            out_show,
            ..
        } = *tv;
        let aspect = THUMB[0] as f32 / THUMB[1] as f32;
        let p = ui.painter_at(r);
        p.rect_filled(r, 5.0, CARD);

        let (ta, tb) = self.timeline.thumb_range;
        let range_ready = (ta - v0).abs() < 1e-6 && (tb - v1).abs() < 1e-6;
        if range_ready {
            let slot = strip.width() / THUMBS as f32;
            for (i, tex) in self.thumbs.iter().enumerate() {
                let Some(tex) = tex else { continue };
                let sr = Rect::from_min_size(
                    pos2(strip.min.x + i as f32 * slot, r.min.y),
                    vec2(slot, r.height()),
                );
                // Center-crop the thumbnail to the slot's aspect.
                let s = (sr.aspect_ratio() / aspect).min(1.0);
                let t = (aspect / sr.aspect_ratio()).min(1.0);
                let uv = Rect::from_center_size(pos2(0.5, 0.5), vec2(s, t));
                p.image(tex.id(), sr, uv, THUMB_TINT);
            }
        }
        if !range_ready || self.thumbs.iter().all(Option::is_none) {
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                t!("timeline.sampling"),
                mono(10.0),
                MUTED,
            );
        }

        let sx0 = xi.clamp(r.min.x, r.max.x);
        let sx1 = xo.clamp(r.min.x, r.max.x);

        // Dim the parts of the visible range outside the selection.
        if self.trim.0 > v0 {
            let xe = xi.clamp(r.min.x, r.max.x);
            p.rect_filled(Rect::from_min_max(r.min, pos2(xe, r.max.y)), 0.0, DIM);
        }
        if self.trim.1 < v1 {
            let xs = xo.clamp(r.min.x, r.max.x);
            p.rect_filled(Rect::from_min_max(pos2(xs, r.min.y), r.max), 0.0, DIM);
        }
        self.paint_strip_cuts(ui, &p, tv);
        // Quick Look–style trim box: solid bars along the top and bottom of the selection,
        // joined to rounded handles with grip lines. Off-view edges get hollow pinned handles.
        if sx1 > sx0 {
            let bar_x = (sx0.max(strip.min.x))..=(sx1.min(strip.max.x));
            for y in [r.min.y..=r.min.y + FRAME, r.max.y - FRAME..=r.max.y] {
                p.rect_filled(Rect::from_x_y_ranges(bar_x.clone(), y), 0.0, GREEN);
            }
        }
        for (hr, inner, visible, pinned) in [
            (
                in_hr,
                egui::CornerRadius {
                    nw: 7,
                    sw: 7,
                    ne: 0,
                    se: 0,
                },
                in_show,
                in_pin,
            ),
            (
                out_hr,
                egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 7,
                    se: 7,
                },
                out_show,
                out_pin,
            ),
        ] {
            if !visible {
                continue;
            }
            let grips = [hr.center().x - 2.5, hr.center().x + 2.5];
            let ys = hr.y_range().shrink(hr.height() * 0.28);
            if pinned {
                let hr = hr.shrink(1.0);
                p.rect_filled(hr, 4.0, CARD);
                p.rect_stroke(hr, 4.0, Stroke::new(1.5, GREEN), egui::StrokeKind::Inside);
                for gx in grips {
                    p.vline(gx, ys, Stroke::new(1.0, GREEN));
                }
            } else {
                p.rect_filled(hr, inner, GREEN);
                for gx in grips {
                    p.vline(gx, ys, Stroke::new(1.5, INK));
                }
            }
        }

        let ph = tv.x(self.pos());
        if strip.x_range().contains(ph) {
            // Reaches a little past the strip, so it reads above the trim box.
            let p = ui.painter();
            let ys = r.y_range().expand(5.0);
            p.vline(ph, ys, Stroke::new(5.0, Color32::from_white_alpha(40)));
            p.vline(ph, ys, Stroke::new(2.0, Color32::WHITE));
        }
    }
}
