use super::status_bar::{clock, time_left};
use super::*;

/// Drag moves the window; double-click maximizes or restores it, like a native titlebar.
pub(super) fn title_bar(resp: &Response) {
    if resp.drag_started() {
        resp.ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        chrome::press(&resp.ctx, chrome::Button::Maximize);
    }
}

/// Minimize/maximize/close, in the order given (reverse it inside a right-to-left layout).
pub(super) fn window_buttons<'a>(ui: &mut Ui, buttons: impl Iterator<Item = &'a chrome::Button>) {
    let max = chrome::maximized(ui.ctx());
    for &b in buttons {
        let (glyph, label) = chrome::look(b, max);
        if icon_button(ui, glyph, label, false).clicked() {
            chrome::press(ui.ctx(), b);
        }
    }
}

impl App {
    /// Header: window controls, mark and file, info/close, modes, and the Export action.
    pub(super) fn header(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        // The whole header is the titlebar: drag moves, double-click maximizes. Registered
        // before the controls, so they sit on top and keep their own clicks.
        let bar = ui.max_rect().expand2(vec2(16.0, 6.0));
        title_bar(&ui.interact(bar, ui.id().with("titlebar"), Sense::click_and_drag()));
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), HEADER_ROW),
            Layout::left_to_right(Align::Center),
            |ui| {
                // macOS full-size content view: leave room for the native traffic lights.
                if cfg!(target_os = "macos") {
                    ui.add_space(69.0);
                }
                if !self.buttons.left.is_empty() {
                    window_buttons(ui, self.buttons.left.iter());
                    ui.add_space(10.0);
                }
                let right = self.buttons.right.clone();
                let title = ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    logo(ui);
                    if self.video.is_none() {
                        ui.label(RichText::new("C H Y K").font(bold(15.0)).color(GREEN));
                    }
                });
                // The mark doubles as the titlebar: drag moves, double-click maximizes.
                let drag = ui.interact(
                    title.response.rect,
                    ui.id().with("title_drag"),
                    Sense::click_and_drag(),
                );
                title_bar(&drag);
                if let Some(v) = &self.video {
                    drag.on_hover_text(v.path.display().to_string());
                    // Item spacing adds 8: 12 pt between the mark's ink and the group, as before it.
                    ui.add_space(5.0);
                    self.mode_group(ui);
                    ui.add_space(10.0);
                    vline(ui, 24.0);
                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !right.is_empty() {
                            window_buttons(ui, right.iter().rev());
                            ui.add_space(6.0);
                            vline(ui, 24.0);
                            ui.add_space(6.0);
                        }
                        self.export_action(ui);
                        ui.add_space(8.0);
                        // Caps give way first: count the room they hold as free for the update button.
                        let caps_room = if self.header_caps {
                            self.caps_extra(ui.painter())
                        } else {
                            0.0
                        };
                        self.update_button(ui, &ctx, self.center_need + 12.0 - caps_room);
                        ui.add_space(12.0);
                        // What's left between Crop/Strip and Export; never drawn past it.
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
                            self.header_center(ui, &ctx);
                            self.center_need = ui.min_rect().width();
                            let slack = ui.max_rect().width() - self.center_need;
                            self.update_header_caps(ui, slack);
                        });
                    });
                } else {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !right.is_empty() {
                            window_buttons(ui, right.iter().rev());
                            ui.add_space(10.0);
                        }
                        self.update_button(ui, &ctx, 0.0);
                    });
                }
            },
        );
    }

    /// Export button, or the running export's progress with a cancel button.
    pub(super) fn export_action(&mut self, ui: &mut Ui) {
        if self.job.is_some() {
            if icon_button(ui, icon::X, t!("export.cancel"), false).clicked()
                && let Some(job) = self.job.take()
            {
                job.cancel();
            }
            let frac = self.export_glide.tick().min(1.0) as f32;
            let left = self
                .export_started
                .as_ref()
                .and_then(|(at, _)| time_left(at.elapsed(), frac))
                .map(|d| t!("status.left", time = clock(d)));
            let text = format!("{}  {:.0}%", icon::EXPORT, frac * 100.0);
            let text = left.map_or(text.clone(), |l| format!("{text}  {l}"));
            let tw = ui
                .painter()
                .layout_no_wrap(text.clone(), mono(12.0), GREEN)
                .size()
                .x;
            let (r, _) = ui.allocate_exact_size(vec2((tw + 28.0).max(150.0), 34.0), Sense::hover());
            let p = ui.painter();
            p.rect(
                r,
                8.0,
                CARD,
                Stroke::new(1.0, LINE),
                egui::StrokeKind::Inside,
            );
            progress_fill(p, r.shrink(3.0), frac, 6.0, GREEN.gamma_multiply(0.22));
            ui.ctx().request_repaint();
            p.text(r.center(), Align2::CENTER_CENTER, text, mono(12.0), GREEN);
            return;
        }
        let label = t!("export.button");
        let w = ui
            .painter()
            .layout_no_wrap(label.into(), bold(12.5), INK)
            .size()
            .x
            + 60.0;
        let caps = if self.header_caps {
            cmd_caps(Cmd::Export)
        } else {
            Vec::new()
        };
        let w = w + if caps.is_empty() {
            0.0
        } else {
            caps_width(ui.painter(), &caps) + 8.0
        };
        let resp = primary_caps(
            ui,
            icon::EXPORT,
            label,
            vec2(w, HEADER_ROW.min(34.0)),
            self.pop == Some(Pop::Export),
            &caps,
        );
        let resp = tip_keys(resp, t!("tip.export"), Cmd::Export);
        self.anchors.export = resp.rect;
        if resp.clicked() {
            self.toggle_pop(Pop::Export);
        }
    }
}
