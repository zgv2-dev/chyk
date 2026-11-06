use super::*;

impl App {
    pub(super) fn open(&mut self, path: PathBuf) {
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
        self.loading = None;
        self.crop = None;
        self.panel = None;
        self.drag = None;
        self.thumbs.clear();
        self.thumbs_rx = None;
        self.timeline.thumb_after = None;
        self.meta = None;
        self.pop = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Glint".into()));
    }

    pub(super) fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new().pick_file() {
            self.request_open(p);
        }
    }

    /// Open `path`, or, if a video is already open, ask first so its edits aren't lost.
    pub(super) fn request_open(&mut self, path: PathBuf) {
        if self.video.is_some() || self.loading.is_some() {
            self.pop = None;
            self.confirm = Some(Confirm::Open(path));
        } else {
            self.open(path);
        }
    }

    /// Start another Glint for `path`. Inside an AppImage the running binary lives in a temporary
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
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} — Glint")));
        let ext = path.extension().unwrap_or_default();
        self.format = ffmpeg::FORMATS
            .iter()
            .position(|f| ext.eq_ignore_ascii_case(f))
            .unwrap_or(0);

        self.meta = None;
        self.video = Some(Video {
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
        self.saved = Some(self.edits());
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
            self.pop = None;
            self.confirm = Some(Confirm::CloseVideo);
        } else {
            self.close(ctx);
        }
    }
}
