use super::*;

/// The trim bar's layout for one frame: the visible range and where the selection's edges and
/// handles sit.
#[derive(Clone, Copy)]
pub(super) struct TrimView {
    /// The whole bar.
    pub(super) r: Rect,
    /// The filmstrip between the handles' outer edges.
    pub(super) strip: Rect,
    pub(super) dur: f64,
    /// Visible time range and its length.
    pub(super) v0: f64,
    pub(super) v1: f64,
    pub(super) span: f64,
    /// Screen x of IN and OUT, and whether each is in view.
    pub(super) xi: f32,
    pub(super) xo: f32,
    pub(super) xi_vis: bool,
    pub(super) xo_vis: bool,
    /// Handles pinned to a view edge because their point is off view.
    pub(super) in_pin: bool,
    pub(super) out_pin: bool,
    pub(super) in_hr: Rect,
    pub(super) out_hr: Rect,
    pub(super) in_show: bool,
    pub(super) out_show: bool,
}

impl TrimView {
    pub(super) fn new(r: Rect, dur: f64, (v0, v1): (f64, f64), trim: (f64, f64)) -> Self {
        // Handles sit outside the kept range, with a little breathing room from the window edge.
        let strip = r.shrink2(vec2(HANDLE + 8.0, 0.0));
        let span = (v1 - v0).max(1e-6);
        let x = |t: f64| strip.min.x + ((t - v0) / span) as f32 * strip.width();
        let (xi, xo) = (x(trim.0), x(trim.1));
        let xi_vis = strip.x_range().contains(xi);
        let xo_vis = strip.x_range().contains(xo);

        // When the timeline is zoomed, one or both trim edges can sit outside the view.
        // Pin those handles to the nearest view edge so they never become unreachable.
        let same_side_off = !xi_vis
            && !xo_vis
            && ((xi < strip.min.x && xo < strip.min.x) || (xi > strip.max.x && xo > strip.max.x));
        let in_pin = !xi_vis && !same_side_off;
        let out_pin = !xo_vis && !same_side_off;
        let pin_rect = |x: f32| {
            let (lo, hi) = if x < strip.min.x {
                (strip.min.x, strip.min.x + HANDLE)
            } else {
                (strip.max.x - HANDLE, strip.max.x)
            };
            Rect::from_x_y_ranges(lo..=hi, r.y_range())
        };
        let in_hr = if xi_vis {
            Rect::from_x_y_ranges(xi - HANDLE..=xi, r.y_range())
        } else if in_pin {
            pin_rect(xi)
        } else {
            Rect::NOTHING
        };
        let out_hr = if xo_vis {
            Rect::from_x_y_ranges(xo..=xo + HANDLE, r.y_range())
        } else if out_pin {
            pin_rect(xo)
        } else {
            Rect::NOTHING
        };
        let in_show = in_hr.is_positive();
        let out_show = out_hr.is_positive();
        Self {
            r,
            strip,
            dur,
            v0,
            v1,
            span,
            xi,
            xo,
            xi_vis,
            xo_vis,
            in_pin,
            out_pin,
            in_hr,
            out_hr,
            in_show,
            out_show,
        }
    }

    /// Screen x of time `t`.
    pub(super) fn x(&self, t: f64) -> f32 {
        self.strip.min.x + ((t - self.v0) / self.span) as f32 * self.strip.width()
    }

    /// Time at screen x `px`, clamped to the view.
    pub(super) fn t_at(&self, px: f32) -> f64 {
        let f = ((px - self.strip.min.x) / self.strip.width()).clamp(0.0, 1.0) as f64;
        (self.v0 + f * self.span).clamp(0.0, self.dur)
    }

    /// What sits under `p`: a visible handle, the selection's top band, or the strip.
    pub(super) fn hit(&self, p: Pos2) -> Drag {
        let Self {
            r,
            xi,
            xo,
            xi_vis,
            xo_vis,
            in_hr,
            out_hr,
            in_show,
            out_show,
            ..
        } = *self;
        let (di, dout) = (
            (p.x - in_hr.center().x).abs(),
            (p.x - out_hr.center().x).abs(),
        );
        if in_show && di < HANDLE * 2.0 && di <= dout {
            Drag::In
        } else if out_show && dout < HANDLE * 2.0 {
            Drag::Out
        } else if xi_vis
            && xo_vis
            && (p.y < r.min.y + GRIP || p.y > r.max.y - GRIP)
            && (xi..=xo).contains(&p.x)
        {
            Drag::Slide
        } else {
            Drag::Scrub
        }
    }
}

impl App {
    /// Filmstrip with Quick Look–style trim handles and a playhead.
    pub(super) fn trim_bar(&mut self, ui: &mut Ui) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur).max(0.001);
        let view = self.timeline.view(dur);
        let (r, resp) = ui.allocate_exact_size(
            vec2(ui.available_width().max(60.0), TRIM_H),
            Sense::click_and_drag(),
        );
        let tv = TrimView::new(r, dur, view, self.trim);
        self.timeline_scroll(ui, tv.strip, dur);
        self.trim_wheel(ui, &resp, &tv);
        self.trim_drag(ui, &resp, &tv);
        match self.drag.or_else(|| resp.hover_pos().map(|p| tv.hit(p))) {
            Some(Drag::In | Drag::Out) => ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal),
            Some(Drag::Slide) if self.drag.is_some() => {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing)
            }
            Some(Drag::Slide) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
            _ => {}
        }
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, t!("a11y.trim"))
        });
        self.paint_trim(ui, &tv);
        if let Some(at) = self.timeline.thumb_after {
            ui.ctx()
                .request_repaint_after(at.saturating_duration_since(Instant::now()));
        }
        let hint = if tv.in_pin || tv.out_pin {
            t!("tip.trim_pinned")
        } else {
            t!("tip.trim")
        };
        let _ = resp.on_hover_text(hint);
    }
}
