use super::*;

impl App {
    /// Ask for the current visible timeline range's filmstrip. One ffmpeg run fills all tiles.
    pub(super) fn load_thumbs(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        let (a, b) = self.timeline.view(v.dur);
        let (tx, rx) = channel();
        let crop = self.thumb_crop;
        ffmpeg::thumbnails(v.path.clone(), (a, b), THUMBS, THUMB, crop, tx, ctx.clone());
        self.thumbs = vec![None; THUMBS];
        self.thumbs_rx = Some(rx);
        self.timeline.thumb_range = (a, b);
        self.timeline.thumb_after = None;
    }

    /// Debounced filmstrip refresh after the timeline view changes.
    pub(super) fn schedule_thumbs(&mut self) {
        self.timeline.thumb_after = Some(Instant::now() + Duration::from_millis(140));
    }

    /// Apply a new timeline view, refreshing thumbnails only if the visible range changed.
    pub(super) fn set_timeline(&mut self, zoom_level: f32, center: f64, dur: f64) {
        let before = self.timeline.view(dur);
        self.timeline.zoom = zoom_level;
        self.timeline.center = center;
        let after = self.timeline.view(dur);
        if (before.0 - after.0).abs() > 1e-6 || (before.1 - after.1).abs() > 1e-6 {
            self.schedule_thumbs();
        }
    }

    pub(super) fn zoom_timeline(&mut self, factor: f64, dur: f64) {
        let (z, c) = zoom::timeline_zoom(dur, self.timeline.zoom, factor, self.pos(), 0.5);
        self.set_timeline(z, c, dur);
    }

    pub(super) fn zoom_timeline_at(&mut self, factor: f64, anchor: f64, fraction: f64, dur: f64) {
        let (z, c) = zoom::timeline_zoom(dur, self.timeline.zoom, factor, anchor, fraction);
        self.set_timeline(z, c, dur);
    }

    pub(super) fn fit_timeline(&mut self, dur: f64) {
        let (z, c) = zoom::timeline_fit(dur);
        self.set_timeline(z, c, dur);
    }

    pub(super) fn fit_timeline_selection(&mut self, dur: f64) {
        let (z, c) = zoom::timeline_fit_selection(dur, self.trim.0, self.trim.1);
        self.set_timeline(z, c, dur);
    }

    pub(super) fn pan_timeline(&mut self, t: f64, dur: f64) {
        self.set_timeline(self.timeline.zoom, t, dur);
    }

    /// While cropping, never zoom or pan past the crop box.
    pub(super) fn contain_crop(&mut self) {
        if let Some(c) = self.crop {
            (self.video_zoom, self.video_center) =
                zoom::contain(self.video_zoom, self.video_center, c);
        }
    }

    pub(super) fn video_src(&self) -> Rect {
        zoom::video_src(self.video_zoom, self.video_center)
    }

    /// The part of the frame on screen, in whole-frame coordinates: video zoom works inside an
    /// applied crop (Crop panel closed), or inside the whole frame while it is open.
    pub(super) fn shown_src(&self) -> Rect {
        let vsrc = self.video_src();
        match self.crop {
            Some(c) if self.panel != Some(Panel::Crop) => Rect::from_min_size(
                c.min + vsrc.min.to_vec2() * c.size(),
                vsrc.size() * c.size(),
            ),
            _ => vsrc,
        }
    }

    pub(super) fn zoom_video(&mut self, factor: f32, anchor: Option<(Pos2, Rect)>) {
        let (z, c) = zoom::video_zoom(self.video_zoom, self.video_center, factor, anchor);
        self.video_zoom = z;
        self.video_center = c;
    }

    pub(super) fn pan_video(&mut self, delta: Vec2, view: Rect) {
        self.video_center = zoom::video_pan(self.video_center, delta, view.size(), self.video_zoom);
    }

    pub(super) fn reset_video_zoom(&mut self) {
        self.video_zoom = 1.0;
        self.video_center = pos2(0.5, 0.5);
    }
}
