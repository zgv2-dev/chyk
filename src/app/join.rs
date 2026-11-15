//! Join: opening several videos as one, adding, selecting, moving and removing clips. The
//! selected clip's trim lives in `App::trim` (timeline seconds) while it is selected.

use super::reel::{Clip, Reel, largest};
use super::*;

mod bar;
mod field;
mod frame;
mod lane;
mod load;
mod place;
mod play;

impl App {
    /// The join with the trim box's edits put back into its selected clip.
    /// While the join (re)loads the trim box still holds the old timeline, so it is left out.
    pub(super) fn reel_now(&self) -> Option<Reel> {
        let mut r = self.reel.clone()?;
        if self.loading.is_some() {
            return Some(r);
        }
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
        self.status = None;
        let clips = self.probe_clips(&paths);
        // Opening clears the status line; what couldn't be read is still worth saying.
        let note = self.status.take();
        if clips.len() < 2 {
            if let Some(c) = clips.into_iter().next() {
                self.open(c.path);
                self.status = note;
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
        self.status = note;
    }

    /// Add `paths` after the open video, which becomes clip 1 and keeps its frame.
    pub(super) fn add_clips(&mut self, paths: Vec<PathBuf>) {
        // A video still opening is the one to join to, not the one on screen.
        if self.reel.is_none()
            && let Some(p) = self.loading.clone()
        {
            // Its pending restore (a join cut down to it) goes to its clip, not a later load.
            let session = self.restore.take();
            let trim = session.as_ref().and_then(|s| s.trim_of(&p));
            let abs = std::path::absolute(&p).unwrap_or_else(|_| p.clone());
            self.open_paths([vec![p], paths].concat());
            match &mut self.reel {
                // Only its own clip, which may have been skipped as unreadable.
                Some(r) => {
                    if let (Some((a, b)), Some(c)) =
                        (trim, r.clips.iter_mut().find(|c| c.path == abs))
                    {
                        let d = c.info.dur;
                        c.trim = (a.clamp(0.0, d), b.clamp(0.0, d));
                    }
                }
                // No join after all: if it is what opens alone, it still restores.
                None => {
                    let same = self
                        .loading
                        .as_ref()
                        .is_some_and(|l| std::path::absolute(l).is_ok_and(|l| l == abs));
                    // A session for another video (replaced before it loaded) is dropped.
                    if same && trim.is_some() {
                        self.restore = session;
                    }
                }
            }
            return;
        }
        // A join still loading is added to as it is.
        let mut reel = match self.reel_now() {
            Some(r) => r,
            None => {
                let Some(v) = &self.video else {
                    return self.open_paths(paths);
                };
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
