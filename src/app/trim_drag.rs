use super::*;

impl App {
    /// Press, drag and release on the trim bar: move a handle, scrub, or slide the selection.
    pub(super) fn trim_drag(&mut self, ui: &Ui, resp: &Response, tv: &TrimView) {
        let TrimView {
            r,
            strip,
            dur,
            xi,
            xo,
            xi_vis,
            xo_vis,
            ..
        } = *tv;
        let t_at = |px: f32| tv.t_at(px);
        if resp.drag_started() || resp.clicked() {
            // Where the press began; see `stage`.
            let o = ui
                .input(|i| i.pointer.press_origin())
                .or(resp.interact_pointer_pos())
                .unwrap_or(r.min);
            let d = tv.hit(o);
            self.drag = Some(d);
            self.grab = vec2(
                match d {
                    Drag::In | Drag::Slide if xi_vis => xi - o.x,
                    Drag::Out if xo_vis => xo - o.x,
                    _ => 0.0,
                },
                0.0,
            );
            let _ = self.player.mpv.set_property("pause", true);
        }
        if (resp.dragged() || resp.clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let t = t_at(p.x + self.grab.x).clamp(0.0, dur);
            match self.drag {
                Some(Drag::In) => self.set_in(t),
                Some(Drag::Out) => self.set_out(t),
                Some(Drag::Scrub) => self.player.seek(t.clamp(self.trim.0, self.trim.1)),
                Some(Drag::Slide) => {
                    self.trim = slide(self.trim, t, dur);
                    self.player.seek(self.trim.0);
                }
                _ => {}
            }
        }
        // Holding a handle still zooms in around it for frame-level trimming; letting go zooms back.
        if resp.dragged()
            && matches!(self.drag, Some(Drag::In | Drag::Out))
            && let Some(p) = resp.interact_pointer_pos()
        {
            let now = Instant::now();
            match self.timeline.hold {
                Some((_, x)) if (p.x - x).abs() <= 2.0 => {}
                _ => self.timeline.hold = Some((now, p.x)),
            }
            let since = self.timeline.hold.map_or(now, |h| h.0);
            if self.timeline.restore.is_none() {
                if now.duration_since(since) >= HOLD {
                    let (t, xe) = if self.drag == Some(Drag::In) {
                        (self.trim.0, xi)
                    } else {
                        (self.trim.1, xo)
                    };
                    let f = ((xe - strip.min.x) / strip.width()).clamp(0.0, 1.0) as f64;
                    self.timeline.restore = Some((self.timeline.zoom, self.timeline.center));
                    self.zoom_timeline_at(HOLD_ZOOM, t, f, dur);
                } else {
                    ui.ctx()
                        .request_repaint_after(HOLD - now.duration_since(since));
                }
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            self.drag = None;
            self.timeline.hold = None;
            if let Some((z, c)) = self.timeline.restore.take() {
                self.set_timeline(z, c, dur);
            }
        }
    }

    /// Wheel over the strip zooms around the pointer; horizontal scroll pans a zoomed view.
    pub(super) fn trim_wheel(&mut self, ui: &mut Ui, resp: &Response, tv: &TrimView) {
        let TrimView {
            r,
            strip,
            dur,
            v0,
            v1,
            span,
            ..
        } = *tv;
        if resp.hovered() && dur > 0.0 {
            // Consume the smoothed wheel delta so one notch does not keep zooming as it decays.
            let (pan, scroll) = ui.input_mut(|i| {
                let d = i.smooth_scroll_delta;
                i.smooth_scroll_delta = Vec2::ZERO;
                (d.x, d.y)
            });
            // A trackpad swipe carries a little of the other axis; only the larger one counts,
            // so a sideways swipe pans without zooming.
            if pan.abs() > scroll.abs() {
                let c = (v0 + v1) / 2.0 - pan as f64 / strip.width() as f64 * span;
                self.pan_timeline(c, dur);
            } else if scroll.abs() > 0.01 {
                let p = resp.hover_pos().unwrap_or(r.center());
                let f = ((p.x - strip.min.x) / strip.width()).clamp(0.0, 1.0) as f64;
                let anchor = v0 + f * span;
                let factor = (scroll as f64 * 0.005).exp().clamp(0.5, 2.0);
                self.zoom_timeline_at(factor, anchor, f, dur);
            }
        }
    }
}
