use super::*;

/// What sits under `p` in the ignore-zone editor: a zone corner, a zone body, or nothing.
pub(super) fn zone_hit(zones: &[Rect], vr: Rect, src: Rect, p: Pos2) -> Option<(usize, Drag)> {
    for (i, z) in zones.iter().enumerate().rev() {
        let r = Rect::from_min_max(
            zoom::source_to_screen(src, vr, z.min),
            zoom::source_to_screen(src, vr, z.max),
        );
        for (hx, hy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let corner = pos2(
                if hx < 0 { r.min.x } else { r.max.x },
                if hy < 0 { r.min.y } else { r.max.y },
            );
            if corner.distance(p) < 16.0 {
                return Some((i, Drag::Crop(hx, hy)));
            }
        }
        if r.contains(p) {
            return Some((i, Drag::Move));
        }
    }
    None
}

impl App {
    /// Red rectangles whose changes the scanner ignores. With `editing`, draw handles and hints.
    pub(super) fn draw_zones(&self, ui: &Ui, vr: Rect, src: Rect, editing: bool) {
        if self.panel != Some(Panel::Strip) {
            return;
        }
        let p = ui.painter_at(vr);
        for (i, z) in self.strip.zones.iter().enumerate() {
            let r = Rect::from_min_max(
                zoom::source_to_screen(src, vr, z.min),
                zoom::source_to_screen(src, vr, z.max),
            );
            let selected = editing && self.strip.selected == Some(i);
            let fill = if selected { 0.22 } else { 0.12 };
            p.rect_filled(r, 0.0, RED.gamma_multiply(fill));
            let stroke = if selected { 2.0 } else { 1.0 };
            p.rect_stroke(r, 0.0, Stroke::new(stroke, RED), egui::StrokeKind::Inside);
            if selected {
                for c in [
                    r.left_top(),
                    r.right_top(),
                    r.left_bottom(),
                    r.right_bottom(),
                ] {
                    p.rect_filled(Rect::from_center_size(c, vec2(8.0, 8.0)), 0.0, RED);
                }
            }
        }
    }

    /// Drag/resize ignore zones over the video.
    /// Drag/resize ignore zones over the video; drag empty space to pan when zoomed.
    pub(super) fn zone_stage(&mut self, ui: &mut Ui, vr: Rect, src: Rect, resp: Response) {
        let origin = ui.input(|i| i.pointer.press_origin());
        if resp.drag_started() {
            let shift = ui.input(|i| i.modifiers.shift);
            let hit = (!shift)
                .then(|| origin.and_then(|p| zone_hit(&self.strip.zones, vr, src, p)))
                .flatten();
            self.strip.drag = hit;
            self.drag = if hit.is_none() && self.video_zoom > 1.001 {
                Some(Drag::Pan)
            } else {
                None
            };
            self.strip.zone_start = None;
            self.strip.grab = Vec2::ZERO;
            if hit.is_none() {
                self.strip.selected = None;
            }
            if let Some((i, drag)) = hit {
                self.strip.selected = Some(i);
                self.strip.zone_start = Some(self.strip.zones[i]);
                if let (Drag::Crop(hx, hy), Some(o)) = (drag, origin) {
                    let z = self.strip.zones[i];
                    let r = Rect::from_min_max(
                        zoom::source_to_screen(src, vr, z.min),
                        zoom::source_to_screen(src, vr, z.max),
                    );
                    let pick = |h: i8, lo: f32, hi: f32, p: f32| match h {
                        -1 => lo,
                        1 => hi,
                        _ => p,
                    };
                    let corner = pos2(
                        pick(hx, r.min.x, r.max.x, o.x),
                        pick(hy, r.min.y, r.max.y, o.y),
                    );
                    self.strip.grab = corner - o;
                }
            }
        }
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            if self.drag == Some(Drag::Pan) {
                self.pan_video(resp.drag_delta(), vr);
            } else if let Some((i, drag)) = self.strip.drag {
                let np = zoom::screen_to_source(src, vr, p + self.strip.grab);
                if let Some(z) = self.strip.zones.get_mut(i) {
                    *z = match drag {
                        Drag::Crop(hx, hy) => crop::drag(*z, hx, hy, np, None),
                        Drag::Move => crop::shift(*z, resp.drag_delta() / vr.size() * src.size()),
                        _ => *z,
                    };
                }
            }
        }
        if resp.drag_stopped() {
            if let (Some((i, _)), Some(start)) = (self.strip.drag, self.strip.zone_start)
                && self.strip.zones.get(i) != Some(&start)
            {
                self.zones_changed();
            }
            self.strip.drag = None;
            self.strip.zone_start = None;
            self.drag = None;
        }
        if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            self.strip.selected = zone_hit(&self.strip.zones, vr, src, p).map(|(i, _)| i);
        }
        let hovered = self.strip.drag.or_else(|| {
            resp.hover_pos()
                .and_then(|p| zone_hit(&self.strip.zones, vr, src, p))
        });
        let shift = ui.input(|i| i.modifiers.shift);
        if self.drag == Some(Drag::Pan) {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if shift && self.video_zoom > 1.001 {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        } else {
            match hovered {
                Some((_, Drag::Crop(-1, -1) | Drag::Crop(1, 1))) => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe)
                }
                Some((_, Drag::Crop(-1, 1) | Drag::Crop(1, -1))) => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw)
                }
                Some((_, Drag::Move)) if self.strip.drag.is_some() => {
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing)
                }
                Some((_, Drag::Move)) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
                _ if self.video_zoom > 1.001 && resp.hovered() => {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab)
                }
                _ => {}
            }
        }
        self.draw_zones(ui, vr, src, true);
    }
}
