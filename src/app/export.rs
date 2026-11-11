use super::*;

impl App {
    pub(super) fn export(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        if self.job.is_some() {
            return;
        }
        if self.strip.enabled {
            let covered = self
                .strip
                .scan
                .as_ref()
                .is_some_and(|s| covers(s.range, self.scan_need()));
            if self.strip.job.is_some() {
                self.status = Some((t!("status.wait_scan").into(), true));
                return;
            }
            if !covered {
                self.status = Some((t!("status.scan_first").into(), true));
                return;
            }
        }
        let keep = if self.strip.enabled {
            self.kept()
        } else {
            None
        };
        if keep.as_ref().is_some_and(Vec::is_empty) {
            self.status = Some((t!("status.nothing_left").into(), true));
            return;
        }
        let ext = ffmpeg::FORMATS[self.format];
        let stem = v.path.file_stem().unwrap_or_default().to_string_lossy();
        let mut tag = if self.strip.enabled {
            "stripped".to_string()
        } else if self.quality == ffmpeg::COMPRESS {
            "compressed".to_string()
        } else {
            "edited".to_string()
        };
        if (self.speed - 1.0).abs() > 1e-4 {
            tag.push('-');
            tag.push_str(&speed_label(self.speed));
        }
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(format!("{stem}-{tag}.{ext}"))
            .add_filter(ext.to_uppercase(), &[ext]);
        if let Some(dir) = v.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        let picked = dialog.save_file();
        keys_released(ctx);
        let Some(mut dst) = picked else {
            return;
        };
        if dst.extension().is_none() {
            dst.set_extension(ext);
        }
        if std::fs::canonicalize(&dst).ok() == std::fs::canonicalize(&v.path).ok() {
            self.status = Some((t!("status.same_file").into(), true));
            return;
        }
        let name = dst
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into());
        let Some(job) = self.export_job(dst.clone()) else {
            return;
        };
        self.export_dst = Some(dst);
        self.job = Some(ffmpeg::export(job, ctx.clone()));
        self.export_glide = Glide::default();
        self.exporting = Some(self.edits());
        self.progress = 0.0;
        self.export_started = Some((Instant::now(), name));
        self.status = None;
        let _ = self.player.mpv.set_property("pause", true);
    }

    /// The export of the current settings to `dst`. `None` without a video, or with Strip on
    /// before its scan.
    pub(super) fn export_job(&self, dst: PathBuf) -> Option<ffmpeg::Job> {
        let v = self.video.as_ref()?;
        let keep = if self.strip.enabled {
            Some(self.kept()?)
        } else {
            None
        };
        let [fw, fh] = v.size;
        let crop = self
            .crop
            .map(|c| crop::to_pixels(c, fw, fh))
            .filter(|&c| c != [fw, fh, 0, 0]);
        Some(ffmpeg::Job {
            src: v.path.clone(),
            dst,
            start: self.trim.0,
            end: self.trim.1,
            crop,
            scale: self.scale(self.size),
            precise: self.precise,
            codec: v.codec.clone(),
            quality: self.quality,
            mute: self.mute,
            speed: self.speed,
            keep,
        })
    }

    /// Plain "Saved", or for compression how much smaller the file got. The percentage is
    /// only shown for a whole-file export: a trimmed clip is smaller than the source anyway.
    pub(super) fn saved_message(&self, bytes: u64, compress: bool) -> String {
        if !compress || bytes == 0 {
            return t!("status.saved").into();
        }
        let Some(v) = &self.video else {
            return t!("status.saved").into();
        };
        let src = std::fs::metadata(&v.path).map_or(0, |m| m.len());
        let whole = self.trim.0 <= 0.01 && self.trim.1 >= v.dur - 0.01;
        if whole && src > bytes {
            let pct = (src - bytes) as f64 / src as f64 * 100.0;
            t!(
                "status.saved_smaller",
                pct = format!("{pct:.0}"),
                size = fmt_size(bytes)
            )
        } else {
            t!("status.saved_size", size = fmt_size(bytes))
        }
    }
}
