//! Starting an export to a chosen file, and the re-encode a failed lossless join offers.

use super::*;

impl App {
    /// Exports the current edits to `dst`. `reencode` turns off the lossless copy.
    pub(in crate::app) fn begin_export(
        &mut self,
        dst: PathBuf,
        reencode: bool,
        ctx: &egui::Context,
    ) {
        // Never over a source: the video, or any clip of a join (checked again on a retry,
        // since clips may have been added since).
        let Some(v) = &self.video else { return };
        let mut sources = vec![v.path.clone()];
        if let Some(r) = &self.reel {
            sources.extend(r.clips.iter().map(|c| c.path.clone()));
        }
        let out = std::fs::canonicalize(&dst).ok();
        if out.is_some() && sources.iter().any(|p| std::fs::canonicalize(p).ok() == out) {
            self.status = Some((t!("status.same_file").into(), true));
            return;
        }
        let name = dst
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into());
        // The encoder probe may have finished while the dialog was open.
        if self.encoder_blocked(ctx) {
            return;
        }
        let Some(mut job) = self.export_job(dst.clone()) else {
            return;
        };
        // A precise trim re-encodes, and only makes the cuts more exact. Seams known not to copy
        // cleanly re-encode without asking: the status bar already said so.
        job.precise |= reencode || self.seams_bad();
        self.remember_folder(&dst);
        self.export_dst = Some(dst);
        self.job = Some(ffmpeg::export(job, ctx.clone()));
        self.export_glide = Glide::default();
        self.exporting = Some(self.edits());
        self.progress = 0.0;
        self.export_started = Some((Instant::now(), name));
        self.status = None;
        let _ = self.player.mpv.set_property("pause", true);
    }

    /// Whether the toast `msg` is a failed lossless join that a re-encode can retry.
    pub(in crate::app) fn can_retry(&self, msg: &str, ctx: &egui::Context) -> bool {
        msg == t!("export.copy_failed")
            && self.job.is_none()
            && self.export_dst.is_some()
            && ffmpeg::can_encode(ctx) != Some(false)
    }

    /// Exports the failed join again to the same file, re-encoded.
    pub(in crate::app) fn retry_reencoded(&mut self, ctx: &egui::Context) {
        if let Some(dst) = self.export_dst.take() {
            self.begin_export(dst, true, ctx);
        }
    }
}
