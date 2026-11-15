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
            self.show_fit(r.clips[i].fit);
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

    /// mpv draws into the output frame's shape; this is how the playing clip fills it.
    pub(in crate::app) fn show_fit(&self, fit: Fit) {
        let (keep, panscan) = match fit {
            Fit::Fit => (true, 0.0),
            Fit::Fill => (true, 1.0),
            Fit::Stretch => (false, 0.0),
        };
        let _ = self.player.mpv.set_property("keepaspect", keep);
        let _ = self.player.mpv.set_property("panscan", panscan);
    }

    /// Set how the selected clip fills the frame.
    pub(in crate::app) fn set_fit(&mut self, fit: Fit) {
        let Some(mut r) = self.reel_now() else { return };
        let sel = r.sel;
        r.clips[sel].fit = fit;
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
