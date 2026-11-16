//! A lossless join's seams, tried in the background once its clips and trims settle, so the
//! status bar and export popover say whether it will copy before Save, not after.

use super::*;

/// How long the clips and trims stay unchanged before their seams are tried.
const SETTLE: Duration = Duration::from_millis(600);

/// The seams last asked about: their [`ffmpeg::seam_key`], since when, and the trial's result.
#[derive(Default)]
pub(in crate::app) struct SeamCheck {
    key: String,
    since: Option<Instant>,
    ok: Option<bool>,
}

impl App {
    /// Whether the export at resolution `size` can stream-copy (mirrors `ffmpeg::run`). It may
    /// still fall back.
    pub(in crate::app) fn stream_copy(&self, size: usize) -> bool {
        self.copy_allowed(size) && !self.seams_bad()
    }

    /// A finished trial found that this join's seams don't copy cleanly.
    pub(in crate::app) fn seams_bad(&self) -> bool {
        self.reel.is_some() && self.timeline.seams.ok == Some(false)
    }

    /// Starts a trial of the join's seams once they have been still for [`SETTLE`].
    pub(in crate::app) fn watch_seams(&mut self, ctx: &egui::Context) {
        let ext = ffmpeg::FORMATS.get(self.format).copied().unwrap_or("mp4");
        let job = (self.reel.is_some() && self.copy_allowed(self.size))
            .then(|| self.export_job(PathBuf::from(format!("seams.{ext}"))))
            .flatten();
        let Some((job, key)) = job.and_then(|j| ffmpeg::seam_key(&j).map(|k| (j, k))) else {
            self.timeline.seams = SeamCheck::default();
            return;
        };
        let s = &mut self.timeline.seams;
        if s.key != key {
            *s = SeamCheck {
                key,
                since: Some(Instant::now()),
                ok: None,
            };
        }
        if s.ok.is_some() {
            return;
        }
        s.ok = ffmpeg::seams_ok(&s.key);
        let waited = s.since.map_or(SETTLE, |t| t.elapsed());
        if s.ok.is_none() && waited >= SETTLE {
            ffmpeg::try_seams(job, s.key.clone(), ctx.clone());
        } else if s.ok.is_none() {
            ctx.request_repaint_after(SETTLE - waited);
        }
    }
}
