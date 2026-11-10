use super::*;

impl App {
    pub(super) fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(SPEED_MIN, SPEED_MAX);
        let _ = self.player.mpv.set_property("speed", self.speed);
    }

    /// Approximate output duration after trim, strip and retiming. `None` means the strip scan is missing.
    pub(super) fn output_length(&self) -> Option<f64> {
        let trim = (self.trim.1 - self.trim.0).max(0.0);
        let source = if self.strip.enabled {
            let scan = self.strip.scan.as_ref()?;
            let keep = strip::keep(&scan.active, self.strip.pad, self.trim.0, self.trim.1);
            keep.iter().map(|&(a, b)| (b - a).max(0.0)).sum()
        } else {
            trim
        };
        Some(source / self.speed.max(0.001))
    }

    /// Export frame size after the crop, before any scaling.
    pub(super) fn cropped(&self) -> Option<[u32; 2]> {
        let v = self.video.as_ref()?;
        let [w, h] = v.size;
        Some(self.crop.map_or(v.size, |c| {
            let [cw, ch, ..] = crop::to_pixels(c, w, h);
            [cw, ch]
        }))
    }

    /// Export frame size at resolution `size` (index into [`ffmpeg::SIZES`]). `None` when it
    /// keeps the cropped size.
    pub(super) fn scale(&self, size: usize) -> Option<[u32; 2]> {
        let base = self.cropped()?;
        let short = ffmpeg::SIZES[size].1?;
        Some(ffmpeg::scaled(base, short)).filter(|&s| s != base)
    }

    /// Whether the export at resolution `size` can stream-copy (mirrors `ffmpeg::run`). It may
    /// still fall back.
    pub(super) fn stream_copy(&self, size: usize) -> bool {
        let crop = match (self.crop, &self.video) {
            (Some(c), Some(v)) => {
                let [w, h] = v.size;
                crop::to_pixels(c, w, h) != [w, h, 0, 0]
            }
            _ => false,
        };
        !crop
            && self.scale(size).is_none()
            && !self.precise
            && self.quality == 0
            && !self.strip.enabled
            && (self.speed - 1.0).abs() < 1e-4
    }

    pub(super) fn nudge_speed(&mut self, up: bool) {
        let current = self.speed;
        let next = if up {
            SPEEDS
                .iter()
                .copied()
                .find(|&s| s > current + 1e-6)
                .unwrap_or(SPEED_MAX)
        } else {
            SPEEDS
                .iter()
                .rev()
                .copied()
                .find(|&s| s < current - 1e-6)
                .unwrap_or(SPEED_MIN)
        };
        self.set_speed(next);
    }

    /// Normalized width/height lock for the current crop preset.
    pub(super) fn lock(&self) -> Option<f32> {
        let aspect = self.video.as_ref()?.aspect;
        crop::PRESETS[self.preset]
            .1
            .map(|r| if r == 0.0 { 1.0 } else { crop::lock(r, aspect) })
    }

    /// Apply a preset. Free keeps the current rectangle and only removes the aspect lock.
    pub(super) fn set_crop_preset(&mut self, preset: usize) {
        self.preset = preset;
        let lock = self.lock();
        if let Some(current) = self.crop {
            self.crop = Some(crop_for_preset(preset, current, lock));
        }
    }

    pub(super) fn pos(&self) -> f64 {
        self.player.props.pos
    }

    pub(super) fn paused(&self) -> bool {
        self.player.props.paused
    }

    pub(super) fn toggle_play(&mut self) {
        let paused = self.paused();
        // At end of file mpv ignores unpausing, and the last frame can start well before
        // the out point on low-frame-rate clips, so restart from IN there too.
        let outside = !(self.trim.0..self.trim.1 - 0.05).contains(&self.pos());
        if paused && (outside || self.player.props.eof) {
            self.player.seek(self.trim.0);
        }
        let _ = self.player.mpv.set_property("pause", !paused);
    }

    pub(super) fn set_in(&mut self, t: f64) {
        // `max`/`min` rather than `clamp`: a clip shorter than MIN_LEN (or of unknown length)
        // would give clamp an inverted range, which panics.
        self.trim.0 = t.min(self.trim.1 - MIN_LEN).max(0.0);
        self.player.seek(self.trim.0);
    }

    pub(super) fn set_out(&mut self, t: f64) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        self.trim.1 = t.max(self.trim.0 + MIN_LEN).min(dur);
        self.player.seek(self.trim.1);
    }

    pub(super) fn skip(&mut self, by: f64) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        self.player.seek((self.pos() + by).clamp(0.0, dur));
    }

    /// Open the Crop panel (whole frame, selection box), or close it, which applies the crop:
    /// the preview then shows only the cropped part.
    pub(super) fn toggle_crop(&mut self, ctx: &egui::Context) {
        self.drag = None;
        if self.panel == Some(Panel::Crop) {
            self.panel = None;
            // A box over the whole frame crops nothing: don't keep it as an applied crop.
            if self.crop.is_some_and(crop::is_full) {
                self.crop = None;
            }
            self.crop_applied(ctx);
            return;
        }
        self.end_zone_edit();
        self.panel = Some(Panel::Crop);
        if self.crop.is_none() {
            self.crop = Some(self.lock().map_or_else(crop::full, crop::fit));
        }
    }

    /// Close the open panel, keeping its effect.
    pub(super) fn close_panel(&mut self, ctx: &egui::Context) {
        match self.panel {
            Some(Panel::Crop) => self.toggle_crop(ctx),
            Some(Panel::Strip) => self.toggle_strip(ctx),
            None => {}
        }
    }

    /// Remove the crop (the crop bar's own button).
    pub(super) fn remove_crop(&mut self, ctx: &egui::Context) {
        self.crop = None;
        if self.panel == Some(Panel::Crop) {
            self.panel = None;
        }
        self.crop_applied(ctx);
    }

    /// The crop changed: a strip scan of another crop no longer fits. With Strip on, scan again.
    pub(super) fn crop_applied(&mut self, ctx: &egui::Context) {
        // The filmstrip shows what will be exported.
        if self.thumb_crop != self.crop {
            self.thumb_crop = self.crop;
            self.load_thumbs(ctx);
        }
        let stale = (self.strip.scan.is_some() || self.strip.job.is_some())
            && self.strip.scan_crop != self.crop;
        if stale && self.strip.enabled {
            self.ensure_scan(ctx);
        } else if stale {
            self.cancel_scan();
            self.strip.scan = None;
        }
    }
}
