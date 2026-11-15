//! Playing a join: only the kept parts, in order, each clip shown with its fit.

use super::*;
use crate::ffmpeg::Fit;

impl App {
    /// Timeline range the trim box may cover: the selected clip, or the whole video.
    pub(in crate::app) fn trim_bounds(&self) -> (f64, f64) {
        match &self.reel {
            Some(r) => r.bounds(r.sel),
            None => (0.0, self.video.as_ref().map_or(0.0, |v| v.dur)),
        }
    }

    /// Once per pass while a join is open: show the playing clip's fit, and while playing skip
    /// what is cut, stopping after the last clip's kept part.
    pub(in crate::app) fn reel_tick(&mut self) {
        if self.loading.is_some() {
            return;
        }
        let Some(r) = self.reel_now() else { return };
        let pos = self.pos();
        let i = r.at(pos);
        if r.shown != Some(i) {
            if let Some(m) = &mut self.reel {
                m.shown = Some(i);
            }
            let c = &r.clips[i];
            self.show_fit(c.fit, c.info.size, r.frame);
        }
        if self.paused() {
            return;
        }
        match r.play_target(pos) {
            Ok(Some(t)) => self.player.seek(t),
            Ok(None) => {}
            Err(()) => {
                let _ = self.player.mpv.set_property("pause", true);
            }
        }
    }

    /// mpv draws into the output frame's shape; this is how a clip of `size` fills `frame`.
    /// A placed clip is zoomed back to its own size and panned to its spot.
    pub(in crate::app) fn show_fit(&self, fit: Fit, size: [u32; 2], frame: [u32; 2]) {
        let (keep, panscan) = match fit {
            Fit::Fit | Fit::Place(_) => (true, 0.0),
            Fit::Fill => (true, 1.0),
            Fit::Stretch => (false, 0.0),
        };
        let (zoom, pan) = match fit {
            Fit::Place(at) => place_view(at, size, frame),
            _ => (0.0, [0.0, 0.0]),
        };
        let mpv = &self.player.mpv;
        let _ = mpv.set_property("keepaspect", keep);
        let _ = mpv.set_property("panscan", panscan);
        let _ = mpv.set_property("video-zoom", zoom);
        let _ = mpv.set_property("video-pan-x", pan[0]);
        let _ = mpv.set_property("video-pan-y", pan[1]);
    }

    /// Set how the selected clip fills the frame.
    /// A placed clip starts centered; picking Place again keeps it where it is.
    pub(in crate::app) fn set_fit(&mut self, fit: Fit) {
        let Some(mut r) = self.reel_now() else { return };
        let sel = r.sel;
        let c = &mut r.clips[sel];
        if c.fit.same_mode(fit) {
            return;
        }
        c.fit = match fit {
            Fit::Place(_) => {
                let mid = |f: u32, s: u32| (f as i32 - s as i32) / 2;
                Fit::Place([
                    mid(r.frame[0], c.info.size[0]),
                    mid(r.frame[1], c.info.size[1]),
                ])
            }
            f => f,
        };
        r.shown = None;
        self.reel = Some(r);
    }

    /// Set the output frame; the crop is kept in the frame's proportions.
    pub(in crate::app) fn set_frame(&mut self, frame: [u32; 2], fps: f64, ctx: &egui::Context) {
        let Some(mut r) = self.reel_now() else { return };
        r.frame = reel::even(frame);
        r.fps = fps;
        if let Some(v) = &mut self.video {
            v.size = r.frame;
            v.aspect = r.frame[0] as f32 / r.frame[1] as f32;
        }
        r.shown = None;
        self.reel = Some(r);
        ctx.request_repaint();
    }

    /// Where Play starts when stopped at `pos`: a join restarts from its first kept part once
    /// past the last one; a single video from IN when outside the selection.
    pub(in crate::app) fn play_restart(&self, pos: f64) -> Option<f64> {
        match self.reel_now() {
            Some(r) => {
                let end = r.kept_range(r.clips.len() - 1).1;
                (pos >= end - 0.05 || self.player.props.eof).then(|| r.kept_range(0).0)
            }
            None => {
                let outside = !(self.trim.0..self.trim.1 - 0.05).contains(&pos);
                (outside || self.player.props.eof).then_some(self.trim.0)
            }
        }
    }
}

/// mpv `video-zoom` (log2) and `video-pan` (in shown video sizes) that put a clip of `size` at
/// `at` in `frame` at its own size: mpv first fits it whole and centered.
pub(super) fn place_view(at: [i32; 2], size: [u32; 2], frame: [u32; 2]) -> (f64, [f64; 2]) {
    let [w, h] = size.map(|v| f64::from(v.max(1)));
    let [fw, fh] = frame.map(f64::from);
    let fit = (fw / w).min(fh / h);
    let pan = |a: i32, s: f64, f: f64| (f64::from(a) + s / 2.0 - f / 2.0) / s;
    ((1.0 / fit).log2(), [pan(at[0], w, fw), pan(at[1], h, fh)])
}

#[cfg(test)]
mod tests {
    /// 1280×720 at (42, 74) in 1920×1080: mpv fits it 1.5× up, so zoom back by log2(1/1.5),
    /// then pan its center from the frame's.
    #[test]
    fn place_view_zooms_to_own_size_and_pans_to_its_spot() {
        let (z, [x, y]) = super::place_view([42, 74], [1280, 720], [1920, 1080]);
        assert!((z - (1.0f64 / 1.5).log2()).abs() < 1e-9);
        assert!((x - (42.0 + 640.0 - 960.0) / 1280.0).abs() < 1e-9);
        assert!((y - (74.0 + 360.0 - 540.0) / 720.0).abs() < 1e-9);
    }
}
