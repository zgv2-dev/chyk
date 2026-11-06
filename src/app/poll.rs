use super::*;

impl App {
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let (loaded, failed) = self.player.poll();
        if loaded {
            self.loaded(ctx);
        }
        // Rotation and aspect are only known once the first frame is decoded.
        let geometry = self.video.is_some().then(|| self.geometry());
        if let (Some(v), Some(g)) = (&mut self.video, geometry) {
            (v.size, v.aspect) = g;
        }
        if let Some(e) = failed
            && let Some(p) = self.loading.take()
        {
            // mpv already dropped the previous file; showing it would leave a dead editor.
            if self.video.take().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("Glint".into()));
            }
            self.status = Some((t!("open.failed", path = p.display(), error = e), true));
        }
        if let Some(rx) = &self.thumbs_rx {
            for (i, img) in rx.try_iter() {
                self.thumbs[i] =
                    Some(ctx.load_texture(format!("thumb{i}"), img, Default::default()));
            }
        }
        if let Some(at) = self.timeline.thumb_after {
            let now = Instant::now();
            if now >= at {
                self.load_thumbs(ctx);
            } else {
                ctx.request_repaint_after(at.saturating_duration_since(now));
            }
        }
        if let Some(job) = &self.job {
            for msg in job.rx.try_iter() {
                match msg {
                    ffmpeg::Msg::Progress(p) => {
                        self.progress = p;
                        self.export_glide.reported(p as f64);
                    }
                    ffmpeg::Msg::Done(Ok(bytes)) => {
                        if !ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
                            notify::done_sound();
                        }
                        self.saved = self.exporting.take();
                        let msg = self.saved_message(bytes, job.compress);
                        self.status = Some((msg, false));
                        self.job = None;
                        break;
                    }
                    ffmpeg::Msg::Done(Err(e)) => {
                        self.status = Some((t!("status.export_failed", error = e), true));
                        self.job = None;
                        break;
                    }
                }
            }
        }
        if let Some(job) = &self.strip.job {
            let mut done = false;
            for msg in job.rx.try_iter() {
                match msg {
                    strip::Msg::Progress(p) => self.strip.progress = p,
                    strip::Msg::Partial {
                        from,
                        intervals,
                        scanned,
                    } => {
                        self.strip.partial.truncate(from);
                        self.strip.partial.extend(intervals);
                        self.strip.scanned = scanned;
                        self.strip.front.reported(scanned);
                    }
                    strip::Msg::Done(Ok(scan)) => {
                        if scan.active.is_empty() {
                            self.status = Some((t!("status.no_activity").into(), false));
                        }
                        if let Some(v) = &self.video {
                            self.scans.insert(
                                v.path.clone(),
                                (self.strip.zones.clone(), self.strip.scan_crop, scan.clone()),
                            );
                        }
                        self.strip.scan = Some(scan);
                        self.strip.progress = 1.0;
                        done = true;
                        break;
                    }
                    strip::Msg::Done(Err(e)) => {
                        self.status = Some((t!("status.scan_failed", error = e), true));
                        done = true;
                        break;
                    }
                }
            }
            if done {
                self.strip.job = None;
                self.strip.partial.clear();
            }
        }
        // Stop at the out point, like Quick Look's trim preview.
        if self.video.is_some() && !self.paused() && self.pos() >= self.trim.1 {
            let _ = self.player.mpv.set_property("pause", true);
        }
    }
}
