use super::*;

impl App {
    /// Header slot right of Crop/Strip: the file when no panel is open, else the open panel's bar.
    pub(super) fn header_center(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        match self.panel {
            None => self.file_info(ui, ctx),
            Some(Panel::Crop) => {
                let mut chosen = None;
                let mut reset = false;
                bar_frame(ui, 34.0, |ui| {
                    for i in 0..crop::PRESETS.len() {
                        if button(ui, "", preset_label(i), self.preset == i, CHIP).clicked() {
                            chosen = Some(i);
                        }
                    }
                    ui.add_space(4.0);
                    vline(ui, 20.0);
                    ui.add_space(4.0);
                    reset = icon_button_look(
                        ui,
                        icon::ARROW_COUNTER_CLOCKWISE,
                        t!("crop.reset"),
                        false,
                        ICON_SMALL,
                    )
                    .clicked();
                });
                if let Some(i) = chosen {
                    self.set_crop_preset(i);
                }
                if reset {
                    self.remove_crop(ctx);
                }
            }
            Some(Panel::Strip) => {
                // Full labels when they fit, else the compact bar. The full width is measured
                // when drawn; an overflowing frame is discarded and redrawn compact.
                let kind = if self.strip.job.is_some() {
                    1
                } else {
                    u8::from(self.strip.scan.is_some()) + 2 * u8::from(!self.strip.zones.is_empty())
                };
                let avail = ui.available_width();
                let compact = self.strip_fit.0 == kind && self.strip_fit.1 > avail;
                let mut off = false;
                let w = bar_frame(ui, 34.0, |ui| {
                    self.strip_controls(ui, ctx, compact);
                    ui.add_space(4.0);
                    vline(ui, 20.0);
                    ui.add_space(4.0);
                    off = icon_button_look(ui, icon::PROHIBIT, t!("strip.off"), false, ICON_SMALL)
                        .clicked();
                });
                if !compact {
                    self.strip_fit = (kind, w);
                    if w > avail + 0.5 {
                        ctx.request_discard("strip bar too wide");
                    }
                }
                if off {
                    self.strip_off();
                }
            }
        }
    }

    /// File name and facts, with the info and close buttons.
    pub(super) fn file_info(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        let name = v.path.file_name().map_or_else(
            || t!("file.untitled").to_string(),
            |n| n.to_string_lossy().into(),
        );
        let meta = [
            (icon::FILM_STRIP, codec_label(&v.codec)),
            (icon::FRAME_CORNERS, format!("{}×{}", v.size[0], v.size[1])),
            (icon::TIMER, fmt_time(v.dur)),
        ];
        let path = v.path.display().to_string();
        let block = ui.scope(|ui| file_block(ui, &name, &meta));
        // The file block doubles as the titlebar: drag moves, double-click maximizes.
        let drag = ui.interact(
            block.response.rect,
            ui.id().with("file_drag"),
            Sense::click_and_drag(),
        );
        title_bar(&drag);
        drag.on_hover_text(path);
        ui.add_space(4.0);
        let key = ctx.format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::I));
        let head = Look {
            height: HEADER_ROW.min(34.0),
            ..ICON
        };
        let info = icon_button_look(
            ui,
            icon::INFO,
            &format!("{} ({key})", t!("tip.info")),
            self.pop == Some(Pop::Info),
            head,
        );
        self.anchors.info = info.rect;
        if info.clicked() {
            self.toggle_pop(Pop::Info);
        }
        let close = ctx.format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::W));
        if icon_button_look(
            ui,
            icon::X_CIRCLE,
            &t!("tip.close_video", key = close),
            false,
            head,
        )
        .clicked()
        {
            self.request_close(ctx);
        }
    }
}
