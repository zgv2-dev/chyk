use super::*;

impl App {
    /// True, with the reason in the status bar, when this export needs an encoder ffmpeg lacks.
    fn encoder_blocked(&mut self, ctx: &egui::Context) -> bool {
        let blocked = self.copy_only(ctx) && !self.stream_copy(self.size);
        if blocked {
            let msg = format!(
                "{} {}",
                t!("export.no_encoder"),
                super::pop_export::encoder_hint()
            );
            self.status = Some((msg, true));
        }
        blocked
    }

    pub(super) fn export(&mut self, ctx: &egui::Context) {
        if self.job.is_none() && self.encoder_blocked(ctx) {
            return;
        }
        let Some(v) = &self.video else { return };
        if self.job.is_some() {
            return;
        }
        if self.strip.enabled {
            let covered = match self.reel_now() {
                Some(r) => self.reel_covered(&r),
                None => self
                    .strip
                    .scan
                    .as_ref()
                    .is_some_and(|s| covers(s.range, self.scan_need())),
            };
            if self.strip.job.is_some() {
                self.status = Some((t!("status.wait_scan").into(), true));
                return;
            }
            if !covered {
                self.status = Some((t!("status.scan_first").into(), true));
                return;
            }
        }
        let nothing = self.strip.enabled && self.strip_summary().is_some_and(|(k, _)| k <= 0.0);
        if nothing {
            self.status = Some((t!("status.nothing_left").into(), true));
            return;
        }
        let ext = ffmpeg::FORMATS[self.format];
        let stem = v.path.file_stem().unwrap_or_default().to_string_lossy();
        // Join, Strip and Compress name themselves; a plain edit takes the File name ending setting.
        let mut tag = if self.reel.is_some() {
            "-joined".to_string()
        } else if self.strip.enabled {
            "-stripped".to_string()
        } else if self.quality == ffmpeg::COMPRESS {
            "-compressed".to_string()
        } else {
            self.settings.suffix.clone()
        };
        if (self.speed - 1.0).abs() > 1e-4 {
            tag.push('-');
            tag.push_str(&speed_label(self.speed));
        }
        let name = format!("{stem}{tag}.{ext}");
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(name)
            .add_filter(ext.to_uppercase(), &[ext]);
        if let Some(dir) = self.save_dir(&v.path) {
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
        // Never over a source: the video, or any clip of a join.
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
        let Some(job) = self.export_job(dst.clone()) else {
            return;
        };
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

    /// Where Save as… starts: the video's folder, or the last export's when the setting says so.
    fn save_dir(&self, video: &std::path::Path) -> Option<PathBuf> {
        let here = video.parent().map(PathBuf::from);
        match self.settings.save_to {
            crate::settings::SaveTo::VideoFolder => here,
            crate::settings::SaveTo::LastFolder => self
                .settings
                .last_folder
                .clone()
                .filter(|d| d.is_dir())
                .or(here),
        }
    }

    /// The export of the current settings to `dst`. `None` without a video, or with Strip on
    /// before its scan.
    pub(super) fn export_job(&self, dst: PathBuf) -> Option<ffmpeg::Job> {
        let v = self.video.as_ref()?;
        let reel = self.reel_now();
        // A join keeps per clip; its whole-file `keep` is unused.
        let (keep, keeps) = match (&reel, self.strip.enabled) {
            (_, false) => (None, None),
            (None, true) => (Some(self.kept()?), None),
            (Some(r), true) => (None, Some(self.reel_keeps(r)?)),
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
            join: reel.map(|r| ffmpeg::Join {
                parts: r
                    .clips
                    .iter()
                    .enumerate()
                    .map(|(i, c)| ffmpeg::Part {
                        src: c.path.clone(),
                        start: c.trim.0,
                        end: c.trim.1,
                        fit: c.fit,
                        keep: keeps.as_ref().map(|k| k[i].clone()),
                        audio: false,
                    })
                    .collect(),
                frame: r.frame,
                // No `fps` filter when every clip already runs at the frame's rate.
                fps: r
                    .clips
                    .iter()
                    .any(|c| (c.info.fps - r.fps).abs() >= 0.01)
                    .then_some(r.fps),
            }),
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
        let whole = self.reel.is_none() && self.trim.0 <= 0.01 && self.trim.1 >= v.dur - 0.01;
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
