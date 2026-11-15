use super::*;

impl App {
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        match self.update.poll() {
            Some(crate::update::Event::Failed(e)) => {
                self.status = Some((t!("status.update_failed", error = e), true));
            }
            Some(crate::update::Event::Close) => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => {}
        }
        // A scan asked for during the export waited for it to end; like Rescan, it replaces any
        // result, so it goes before extend_scan could grow the old one.
        if self.strip.rescan && self.job.is_none() && self.strip.job.is_none() {
            self.strip.rescan = false;
            if self.strip.enabled {
                self.start_scan(ctx);
            }
        }
        self.extend_scan(ctx);
        self.watch_seams(ctx);
        self.poll_frame();
        let (loaded, failed) = self.player.poll();
        if loaded {
            self.loaded(ctx);
        }
        // Rotation and aspect are only known once the first frame is decoded.
        // A join keeps its output frame whatever clip mpv is showing.
        let geometry = (self.video.is_some() && self.reel.is_none()).then(|| self.geometry());
        if let (Some(v), Some(g)) = (&mut self.video, geometry) {
            (v.size, v.aspect) = g;
        }
        if let Some(e) = failed
            && let Some(p) = self.loading.take()
        {
            // mpv already dropped the previous file; showing it would leave a dead editor.
            if self.video.take().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("Chyk".into()));
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
                        // Progress only falls when ffmpeg starts another pass over the video.
                        if p + 0.05 < self.progress {
                            self.export_glide = Glide::default();
                            if let Some((at, _)) = &mut self.export_started {
                                *at = Instant::now();
                            }
                        }
                        self.progress = p;
                        self.export_glide.reported(p as f64);
                    }
                    ffmpeg::Msg::Done(Ok(bytes)) => {
                        if self.settings.sound
                            && !ctx.input(|i| i.viewport().focused.unwrap_or(true))
                        {
                            notify::done_sound();
                        }
                        self.saved = self.exporting.take();
                        let msg = self.saved_message(bytes, job.compress);
                        self.last_export = self.export_dst.take();
                        self.saved_toast = Some(msg.clone());
                        self.status = Some((msg, false));
                        self.job = None;
                        break;
                    }
                    // Keeps the destination for the toast's Re-encode button.
                    ffmpeg::Msg::CopyFailed => {
                        self.status = Some((t!("export.copy_failed").into(), true));
                        self.job = None;
                        break;
                    }
                    ffmpeg::Msg::Done(Err(e)) => {
                        self.status = Some((t!("status.export_failed", error = e), true));
                        self.export_dst = None;
                        self.job = None;
                        break;
                    }
                }
            }
        }
        if let Some(job) = &self.strip.job {
            let (mut done, mut hold) = (false, false);
            // A join's clip scan reports in its clip's seconds; the timeline adds its start.
            let off = match (&self.strip.clip, &self.reel) {
                (Some((_, i)), Some(r)) if *i < r.clips.len() => r.start(*i),
                _ => 0.0,
            };
            for msg in job.rx.try_iter() {
                match msg {
                    strip::Msg::Progress(p) => self.strip.progress = p,
                    strip::Msg::Partial {
                        from,
                        intervals,
                        scanned,
                    } => {
                        self.strip.partial.truncate(from);
                        self.strip
                            .partial
                            .extend(intervals.into_iter().map(|(a, b)| (a + off, b + off)));
                        self.strip.scanned = scanned + off;
                        self.strip.front.reported(scanned + off);
                    }
                    strip::Msg::Done(Ok(piece)) if self.strip.clip.is_some() => {
                        // A clip's piece joins its earlier scan; the next clip starts from poll.
                        if let Some((key, _)) = self.strip.clip.take() {
                            let scan = match self.clip_scans.remove(&key) {
                                Some(s) => s.merge(piece),
                                None => piece,
                            };
                            self.clip_scans.insert(key, scan);
                        }
                        self.strip.progress = 1.0;
                        done = true;
                        break;
                    }
                    strip::Msg::Done(Ok(piece)) => {
                        // A piece next to an earlier scan joins it.
                        let scan = match self.strip.scan.take() {
                            Some(s) => s.merge(piece),
                            None => piece,
                        };
                        if scan.active.is_empty() {
                            self.status = Some((t!("status.no_activity").into(), false));
                        }
                        if let Some(v) = &self.video {
                            self.scans.insert(
                                v.path.clone(),
                                (
                                    self.strip.zones.clone(),
                                    self.strip.scan_crop,
                                    self.strip.sensitivity,
                                    scan.clone(),
                                ),
                            );
                        }
                        self.strip.scan = Some(scan);
                        self.strip.progress = 1.0;
                        done = true;
                        break;
                    }
                    strip::Msg::Done(Err(e)) => {
                        self.status = Some((t!("status.scan_failed", error = e), true));
                        // A failed extension would fail again: retry only once the trim changes.
                        hold = true;
                        done = true;
                        break;
                    }
                }
            }
            if hold {
                self.hold_scan();
            }
            if done {
                self.strip.job = None;
                self.strip.clip = None;
                self.strip.partial.clear();
            }
        }
        // Stop at the out point, like Quick Look's trim preview (unless turned off in the settings).
        // A join always runs its own tick: it skips the cut parts between clips.
        if self.reel.is_some() {
            self.reel_tick();
        } else if self.settings.stop_at_out
            && self.video.is_some()
            && !self.paused()
            && self.pos() >= self.trim.1
        {
            let _ = self.player.mpv.set_property("pause", true);
        }
    }
}
