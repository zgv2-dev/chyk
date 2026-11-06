use super::*;

impl App {
    /// The crop box over the whole frame while the Crop panel is open: drag a corner, an edge
    /// (free aspect only) or the inside; Shift-drag pans a zoomed view.
    pub(super) fn crop_stage(&mut self, ui: &mut Ui, vr: Rect, src: Rect, resp: Response, c: Rect) {
        let to_screen = |p: Pos2| zoom::source_to_screen(src, vr, p);
        let sr = Rect::from_min_max(to_screen(c.min), to_screen(c.max));
        let k = self.lock();

        // What sits under the pointer: a corner, an edge (free aspect only), the inside, or nothing.
        let hit = |p: Pos2| -> Option<Drag> {
            for (hx, hy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let corner = pos2(
                    if hx < 0 { sr.min.x } else { sr.max.x },
                    if hy < 0 { sr.min.y } else { sr.max.y },
                );
                if corner.distance(p) < 20.0 {
                    return Some(Drag::Crop(hx, hy));
                }
            }
            if k.is_none() && sr.expand(10.0).contains(p) {
                let near = |a: f32, b: f32| (a - b).abs() < 10.0;
                for (hx, hy, hit) in [
                    (-1, 0, near(p.x, sr.min.x)),
                    (1, 0, near(p.x, sr.max.x)),
                    (0, -1, near(p.y, sr.min.y)),
                    (0, 1, near(p.y, sr.max.y)),
                ] {
                    if hit {
                        return Some(Drag::Crop(hx, hy));
                    }
                }
            }
            sr.contains(p).then_some(Drag::Move)
        };

        // Hit-test where the press began: by the time egui reports a drag the pointer
        // has moved past its threshold, often off a small handle.
        let origin = ui.input(|i| i.pointer.press_origin());
        if resp.drag_started() {
            let shift = ui.input(|i| i.modifiers.shift);
            let hit = (!shift).then(|| origin.and_then(hit)).flatten();
            self.drag = hit.or_else(|| (self.video_zoom > 1.001).then_some(Drag::Pan));
            if let (Some(Drag::Crop(hx, hy)), Some(o)) = (self.drag, origin) {
                let pick = |h: i8, lo: f32, hi: f32, p: f32| match h {
                    -1 => lo,
                    1 => hi,
                    _ => p,
                };
                let handle = pos2(
                    pick(hx, sr.min.x, sr.max.x, o.x),
                    pick(hy, sr.min.y, sr.max.y, o.y),
                );
                self.grab = handle - o;
            }
        }
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            if self.drag == Some(Drag::Pan) {
                self.pan_video(resp.drag_delta(), vr);
            } else {
                let np = zoom::screen_to_source(src, vr, p + self.grab);
                self.crop = Some(match self.drag {
                    Some(Drag::Crop(hx, hy)) => crop::drag(c, hx, hy, np, k),
                    Some(Drag::Move) => crop::shift(c, resp.drag_delta() / vr.size() * src.size()),
                    _ => c,
                });
            }
        }
        if resp.drag_stopped() {
            self.drag = None;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        let hovered = self.drag.or_else(|| resp.hover_pos().and_then(hit));
        if self.drag == Some(Drag::Pan) {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if shift && self.video_zoom > 1.001 {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        } else if let Some(d) = hovered {
            ui.ctx().set_cursor_icon(match d {
                Drag::Crop(-1, -1) | Drag::Crop(1, 1) => CursorIcon::ResizeNwSe,
                Drag::Crop(-1, 1) | Drag::Crop(1, -1) => CursorIcon::ResizeNeSw,
                Drag::Crop(_, 0) => CursorIcon::ResizeHorizontal,
                Drag::Crop(..) => CursorIcon::ResizeVertical,
                Drag::Pan => {
                    if self.drag.is_some() {
                        CursorIcon::Grabbing
                    } else {
                        CursorIcon::Grab
                    }
                }
                _ if self.drag.is_some() => CursorIcon::Grabbing,
                _ => CursorIcon::Grab,
            });
        } else if self.video_zoom > 1.001 && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        draw_crop(&ui.painter_at(vr), vr, sr, k.is_none());
        self.draw_zones(ui, vr, src, false);
    }
}

/// Dim outside the crop box, then draw its thirds, corner arms and (free aspect) edge ticks.
fn draw_crop(p: &egui::Painter, vr: Rect, sr: Rect, free: bool) {
    let clip = sr.intersect(vr);
    if clip.is_positive() {
        for r in [
            Rect::from_min_max(vr.min, pos2(vr.max.x, clip.min.y)),
            Rect::from_min_max(pos2(vr.min.x, clip.max.y), vr.max),
            Rect::from_min_max(pos2(vr.min.x, clip.min.y), pos2(clip.min.x, clip.max.y)),
            Rect::from_min_max(pos2(clip.max.x, clip.min.y), pos2(vr.max.x, clip.max.y)),
        ] {
            p.rect_filled(r, 0.0, DIM);
        }
        p.rect_stroke(sr, 0.0, Stroke::new(1.0, GREEN), egui::StrokeKind::Inside);
    } else {
        // The crop is entirely outside the zoomed view; nothing is kept visibly.
        p.rect_filled(vr, 0.0, DIM);
    }
    if clip.is_positive() {
        let thin = Stroke::new(1.0, GREEN.gamma_multiply(0.28));
        for f in [1.0 / 3.0, 2.0 / 3.0] {
            p.vline(sr.lerp_inside(vec2(f, 0.0)).x, sr.y_range(), thin);
            p.hline(sr.x_range(), sr.lerp_inside(vec2(0.0, f)).y, thin);
        }
    }
    let thick = Stroke::new(3.0, GREEN);
    let arm = 18.0f32.min(sr.width() / 2.0).min(sr.height() / 2.0);
    for (hx, hy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0f32)] {
        let c = pos2(
            if hx < 0.0 { sr.min.x } else { sr.max.x },
            if hy < 0.0 { sr.min.y } else { sr.max.y },
        );
        if vr.expand(2.0).contains(c) {
            p.line_segment([c, c - vec2(hx * arm, 0.0)], thick);
            p.line_segment([c, c - vec2(0.0, hy * arm)], thick);
        }
    }
    if free && clip.is_positive() {
        let (cx, cy) = (sr.center().x, sr.center().y);
        p.hline(cx - 9.0..=cx + 9.0, sr.min.y, thick);
        p.hline(cx - 9.0..=cx + 9.0, sr.max.y, thick);
        p.vline(sr.min.x, cy - 9.0..=cy + 9.0, thick);
        p.vline(sr.max.x, cy - 9.0..=cy + 9.0, thick);
    }
}
