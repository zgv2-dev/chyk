use super::*;

impl App {
    pub fn new(
        cc: &eframe::CreationContext,
        file: Option<PathBuf>,
        restore: Option<Session>,
    ) -> Result<Self, String> {
        fonts(&cc.egui_ctx);
        theme(&cc.egui_ctx);
        #[cfg(target_os = "macos")]
        chrome::use_srgb(cc);
        #[cfg(all(unix, not(target_os = "macos")))]
        crate::dnd::start(cc);
        let mut app = Self {
            player: Player::new(cc)?,
            loading: None,
            video: None,
            trim: (0.0, 0.0),
            crop: None,
            preset: 0,
            precise: false,
            format: 0,
            quality: 0,
            size: 0,
            sampler: Sampler::default(),
            mute: false,
            speed: 1.0,
            strip: Strip {
                pad: 0.5,
                ..Default::default()
            },
            timeline: Timeline {
                zoom: 1.0,
                ..Default::default()
            },
            video_zoom: 1.0,
            video_center: pos2(0.5, 0.5),
            drag: None,
            grab: Vec2::ZERO,
            thumbs: Vec::new(),
            thumbs_rx: None,
            thumb_crop: None,
            job: None,
            progress: 0.0,
            status: None,
            status_since: None,
            export_dst: None,
            last_export: None,
            saved_toast: None,
            buttons: chrome::Layout::detect(),
            pop: None,
            export_glide: Glide::default(),
            panel: None,
            strip_fit: (u8::MAX, 0.0),
            deck_need: 0.0,
            confirm: None,
            sheet: None,
            saved: None,
            exporting: None,
            quitting: false,
            anchors: Anchors::default(),
            meta: None,
            scans: HashMap::new(),
            update: crate::update::Updater::start({
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            }),
            center_need: 0.0,
            header_caps: false,
            restore,
            history: Default::default(),
            log_open: false,
            export_started: None,
            recent: crate::recent::load(),
            recent_sel: 0,
            recent_tex: HashMap::new(),
        };
        if let Some(f) = file {
            app.open(f);
        }
        Ok(app)
    }
}

impl eframe::App for App {
    /// Runs before every `ui`, and alone while the window is covered or minimized (eframe skips
    /// `ui` then), so an export keeps updating the Dock and ends with the sound.
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.poll(ctx);
        notify::progress(frame, self.job.as_ref().map(|_| self.progress));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "macos")]
        chrome::place_traffic_lights(_frame, 52.0);
        #[cfg(target_os = "macos")]
        chrome::route_quit_to_close();
        // Closing the window or quitting with unexported edits: ask first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.unsaved() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pop = None;
            self.confirm = Some(Confirm::Quit);
        }
        self.shortcuts(&ctx);

        let header = egui::Panel::top("header")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ui, |ui| self.header(ui));
        let r = header.response.rect;
        self.anchors.header_bottom = r.bottom();
        ui.painter()
            .hline(r.x_range(), r.bottom(), Stroke::new(1.0, LINE));

        self.status_bar(ui);
        if self.video.is_some() {
            let deck = egui::Panel::bottom("deck")
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin {
                    left: 16,
                    right: 16,
                    top: 10,
                    bottom: 14,
                }))
                .show(ui, |ui| self.deck(ui));
            let r = deck.response.rect;
            ui.painter()
                .hline(r.x_range(), r.top(), Stroke::new(1.0, LINE));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ui, |ui| {
                if self.video.is_some() {
                    canvas(ui, ui.max_rect());
                    self.stage(ui);
                } else {
                    self.empty(ui);
                }
                self.toast(ui);
            });
        self.popover(&ctx);
        self.sheet_ui(&ctx);
        self.open_modal(&ctx);
        chrome::resize_edges(&ctx);
        self.settle_history(&ctx);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        crate::dnd::feed(raw);
    }

    /// A finished update download replaces this copy as Chyk quits; the user agreed by downloading.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.update.on_exit();
    }
}
