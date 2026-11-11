use super::*;

/// A filmstrip's range in milliseconds and its crop in 1/10000ths of the frame. Rounded, so the
/// same view reached again (zoom in, then out) finds its strip despite float noise.
pub(super) type ThumbKey = (i64, i64, Option<[i32; 4]>);

/// Filmstrips kept per file, about 330 KB of textures each.
const STRIPS: usize = 32;

pub(super) fn thumb_key((a, b): (f64, f64), crop: Option<Rect>) -> ThumbKey {
    let ms = |t: f64| (t * 1000.0).round() as i64;
    let crop = crop.map(|c| [c.min.x, c.min.y, c.max.x, c.max.y].map(|v| (v * 1e4).round() as i32));
    (ms(a), ms(b), crop)
}

impl App {
    /// Show the visible timeline range's filmstrip: from the strips seen before, or one ffmpeg
    /// run for all tiles (a partly loaded strip shows what it has meanwhile).
    pub(super) fn load_thumbs(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        let (path, (a, b)) = (v.path.clone(), self.timeline.view(v.dur));
        let key = thumb_key((a, b), self.thumb_crop);
        let tl = &mut self.timeline;
        if let Some(old) = tl.strip_key.take() {
            tl.strips.insert(old, std::mem::take(&mut self.thumbs));
        }
        // shortcut: forgets every strip once there are too many; evict the oldest if it matters.
        if tl.strips.len() > STRIPS {
            tl.strips.clear();
        }
        self.thumbs = tl.strips.remove(&key).unwrap_or_else(|| vec![None; THUMBS]);
        tl.strip_key = Some(key);
        tl.thumb_range = (a, b);
        tl.thumb_after = None;
        self.thumbs_rx = None;
        if self.thumbs.iter().any(Option::is_none) {
            let (tx, rx) = channel();
            let crop = self.thumb_crop;
            ffmpeg::thumbnails(path, (a, b), THUMBS, THUMB, crop, tx, ctx.clone());
            self.thumbs_rx = Some(rx);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_key_ignores_float_noise_but_not_range_or_crop() {
        let a = thumb_key((1.0, 4.0), None);
        assert_eq!(a, thumb_key((1.0 + 1e-9, 4.0 - 1e-9), None));
        assert_ne!(a, thumb_key((1.0, 4.001), None));
        let c = Rect::from_min_max(pos2(0.1, 0.2), pos2(0.9, 0.8));
        assert_eq!(
            thumb_key((1.0, 4.0), Some(c)),
            (1000, 4000, Some([1000, 2000, 9000, 8000]))
        );
    }
}
