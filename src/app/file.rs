use super::*;

impl App {
    pub(super) fn open(&mut self, path: PathBuf) {
        self.history = Default::default();
        let (enabled, pad) = (self.strip.enabled, self.strip.pad);
        self.cancel_scan();
        self.strip = Strip {
            enabled,
            pad,
            ..Default::default()
        };
        self.timeline = Timeline {
            zoom: 1.0,
            ..Default::default()
        };
        self.video_zoom = 1.0;
        self.video_center = pos2(0.5, 0.5);
        self.speed = 1.0;
        self.drag = None;
        self.grab = Vec2::ZERO;
        self.thumbs = vec![None; THUMBS];
        self.thumbs_rx = None;
        self.meta = None;
        self.pop = None;
        let _ = self.player.mpv.set_property("speed", 1.0);
        self.player.cmd("loadfile", &[&path.to_string_lossy()]);
        self.loading = Some(path);
        self.status = None;
    }

    /// Back to the start screen. A running export keeps going and still reports when done.
    pub(super) fn close(&mut self, ctx: &egui::Context) {
        let (enabled, pad) = (self.strip.enabled, self.strip.pad);
        self.cancel_scan();
        self.strip = Strip {
            enabled,
            pad,
            ..Default::default()
        };
        self.player.cmd("stop", &[]);
        self.video = None;
        self.history = Default::default();
        self.loading = None;
        self.crop = None;
        self.panel = None;
        self.drag = None;
        self.thumbs.clear();
        self.recent_tex.clear();
        self.recent_checked = None;
        self.thumbs_rx = None;
        self.timeline.thumb_after = None;
        self.meta = None;
        self.pop = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Chyk".into()));
    }

    pub(super) fn open_dialog(&mut self, ctx: &egui::Context) {
        let dialog = rfd::FileDialog::new().add_filter(t!("open.videos"), videos::VIDEO_EXTS);
        let picked = dialog.pick_file();
        keys_released(ctx);
        if let Some(p) = picked {
            self.request_open(p);
        }
    }

    /// Open `path`, or, if a video is already open, ask first so its edits aren't lost.
    pub(super) fn request_open(&mut self, path: PathBuf) {
        if self.video.is_some() || self.loading.is_some() {
            self.ask(Confirm::Open(path));
        } else {
            self.open(path);
        }
    }

    /// Start another Chyk for `path`. Inside an AppImage the running binary lives in a temporary
    /// mount, so relaunch the AppImage file itself.
    pub(super) fn open_in_new_window(&mut self, path: PathBuf) {
        let exe = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .or_else(|| std::env::current_exe().ok());
        let started = exe.map(|exe| std::process::Command::new(exe).arg(&path).spawn());
        if !matches!(started, Some(Ok(_))) {
            self.status = Some((t!("open.new_window_failed").into(), true));
        }
    }

    /// Frame size and display aspect after rotation.
    pub(super) fn geometry(&self) -> ([u32; 2], f32) {
        let p = &self.player.props;
        let (w, h) = (p.width.max(1) as u32, p.height.max(1) as u32);
        let aspect = p.aspect.unwrap_or(w as f64 / h as f64) as f32;
        if p.rotate % 180 == 90 {
            ([h, w], 1.0 / aspect)
        } else {
            ([w, h], aspect)
        }
    }

    pub(super) fn loaded(&mut self, ctx: &egui::Context) {
        let Some(path) = self.loading.take() else {
            return;
        };
        // The recent list outlives the working directory, so keep the full path.
        let path = std::path::absolute(&path).unwrap_or(path);
        let dur = self.player.get::<f64>("duration").unwrap_or(0.0);
        let (size, aspect) = self.geometry();
        let codec = self
            .player
            .get::<String>("current-tracks/video/codec")
            .unwrap_or_default();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} — Chyk")));
        let ext = path.extension().unwrap_or_default();
        self.format = ffmpeg::FORMATS
            .iter()
            .position(|f| ext.eq_ignore_ascii_case(f))
            .unwrap_or(0);

        self.meta = None;
        let bytes = std::fs::metadata(&path).map_or(0, |m| m.len());
        self.video = Some(Video {
            bytes,
            path,
            dur,
            size,
            aspect,
            codec,
        });
        self.trim = (0.0, dur);
        self.deck_need = 0.0;
        self.crop = None;
        self.panel = self.panel.filter(|p| *p == Panel::Strip);
        self.timeline = Timeline {
            zoom: 1.0,
            center: dur / 2.0,
            ..Default::default()
        };
        self.video_zoom = 1.0;
        self.video_center = pos2(0.5, 0.5);
        self.thumb_crop = None;
        self.load_thumbs(ctx);
        if self.strip.enabled {
            self.ensure_scan(ctx);
        }
        self.record_recent();
        self.saved = Some(self.edits());
        if let Some(s) = self.restore.take() {
            self.apply_session(s, ctx);
        }
        self.history.reset(self.snapshot());
    }

    pub(super) fn edits(&self) -> Edits {
        let strip = self
            .strip
            .enabled
            .then(|| (self.strip.zones.clone(), self.strip.pad));
        (self.trim, self.crop, strip, self.speed)
    }

    /// Edits made since loading or the last export, which closing would lose.
    pub(super) fn unsaved(&self) -> bool {
        self.video.is_some() && self.saved.as_ref() != Some(&self.edits())
    }

    /// Close the video, asking first if that would lose unexported edits.
    pub(super) fn request_close(&mut self, ctx: &egui::Context) {
        if self.unsaved() {
            self.ask(Confirm::CloseVideo);
        } else {
            self.close(ctx);
        }
    }
}
