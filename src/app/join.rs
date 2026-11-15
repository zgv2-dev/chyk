//! Join: opening several videos as one, adding, selecting, moving and removing clips. The
//! selected clip's trim lives in `App::trim` (timeline seconds) while it is selected.

use super::reel::{Clip, Reel, largest};
use super::*;

mod bar;
mod lane;
mod load;
mod play;

impl App {
    /// The join with the trim box's edits put back into its selected clip.
    pub(super) fn reel_now(&self) -> Option<Reel> {
        let mut r = self.reel.clone()?;
        let s = r.start(r.sel);
        let d = r.clips[r.sel].info.dur;
        r.clips[r.sel].trim = (
            (self.trim.0 - s).clamp(0.0, d),
            (self.trim.1 - s).clamp(0.0, d),
        );
        Some(r)
    }

    /// Clips for `paths`, skipping (and reporting) what ffmpeg can't read.
    fn probe_clips(&mut self, paths: &[PathBuf]) -> Vec<Clip> {
        let mut out = Vec::new();
        for p in paths {
            let path = std::path::absolute(p).unwrap_or_else(|_| p.clone());
            match ffmpeg::probe_clip(&path) {
                Some(info) => out.push(Clip::new(path, info)),
                None => {
                    let e = t!("join.unreadable");
                    self.status = Some((t!("open.failed", path = p.display(), error = e), true));
                }
            }
        }
        out
    }

    /// Open `paths`: one as usual, several as a join whose frame takes their largest width,
    /// height and frame rate.
    pub(super) fn open_paths(&mut self, mut paths: Vec<PathBuf>) {
        if paths.len() < 2 {
            if let Some(p) = paths.pop() {
                self.open(p);
            }
            return;
        }
        let clips = self.probe_clips(&paths);
        if clips.len() < 2 {
            if let Some(c) = clips.into_iter().next() {
                self.open(c.path);
            }
            return;
        }
        let (frame, fps) = largest(&clips);
        let reel = Reel {
            clips,
            sel: 0,
            frame,
            fps,
            shown: None,
        };
        self.open_as(reel.clips[0].path.clone(), &reel.edl());
        self.saved = None;
        self.reel = Some(reel);
    }

    /// Add `paths` after the open video, which becomes clip 1 and keeps its frame.
    pub(super) fn add_clips(&mut self, paths: Vec<PathBuf>) {
        // A video still opening is the one to join to, not the one on screen.
        if self.reel.is_none()
            && let Some(p) = self.loading.clone()
        {
            return self.open_paths([vec![p], paths].concat());
        }
        let Some(v) = &self.video else {
            return self.open_paths(paths);
        };
        let mut reel = match self.reel_now() {
            Some(r) => r,
            None => {
                let info = ffmpeg::probe_clip(&v.path).unwrap_or_else(|| ffmpeg::ClipInfo {
                    dur: v.dur,
                    size: v.size,
                    fps: 30.0,
                    codec: v.codec.clone(),
                });
                let (frame, fps) = largest(std::slice::from_ref(&Clip::new(
                    v.path.clone(),
                    info.clone(),
                )));
                let mut first = Clip::new(v.path.clone(), info);
                first.trim = self.trim;
                Reel {
                    clips: vec![first],
                    sel: 0,
                    frame,
                    fps,
                    shown: None,
                }
            }
        };
        let added = self.probe_clips(&paths);
        if added.is_empty() {
            return;
        }
        reel.sel = reel.clips.len();
        reel.clips.extend(added);
        self.trim = reel.kept_range(reel.sel);
        self.reel = Some(reel);
        self.reload_reel();
    }

    /// Pick videos to add (J with a single video, or the Join bar's +).
    pub(super) fn add_dialog(&mut self, ctx: &egui::Context) {
        let dialog = rfd::FileDialog::new().add_filter(t!("open.videos"), videos::VIDEO_EXTS);
        let picked = dialog.pick_files();
        keys_released(ctx);
        if let Some(p) = picked.filter(|p| !p.is_empty()) {
            self.add_clips(p);
        }
    }
}
