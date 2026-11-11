use super::*;

impl App {
    pub(super) fn toast(&mut self, ui: &mut Ui) {
        let Some((msg, err)) = self.status.clone() else {
            self.status_since = None;
            return;
        };
        // Confirmations fade out after a few seconds; errors stay until dismissed.
        let now = Instant::now();
        if self.status_since.as_ref().is_none_or(|(m, _)| *m != msg) {
            self.status_since = Some((msg.clone(), now));
        }
        let age = self
            .status_since
            .as_ref()
            .map_or(Duration::ZERO, |(_, t)| now - *t);
        let mut opacity = 1.0;
        if !err {
            if age >= STATUS_LIFE {
                self.status = None;
                self.status_since = None;
                return;
            }
            let left = STATUS_LIFE - age;
            opacity = (left.as_secs_f32() / STATUS_FADE.as_secs_f32()).min(1.0);
            ui.ctx().request_repaint_after(if left > STATUS_FADE {
                left - STATUS_FADE
            } else {
                Duration::from_millis(16)
            });
        }
        let central = ui.max_rect();
        // The saved toast offers the file manager; the others are just their message.
        let saved = (!err && self.saved_toast.as_deref() == Some(&msg))
            .then(|| self.last_export.clone())
            .flatten();
        let area = egui::Area::new("status".into())
            .fixed_pos(pos2(central.max.x - 16.0, central.max.y - 12.0))
            .pivot(Align2::RIGHT_BOTTOM)
            .order(egui::Order::Foreground);
        area.show(ui.ctx(), |ui| {
            ui.set_opacity(opacity);
            let (glyph, color) = if err {
                (icon::WARNING, RED)
            } else {
                (icon::CHECK, GREEN)
            };
            // Never wider than the window; long messages wrap.
            ui.set_max_width(520.0f32.min(central.width() - 32.0));
            egui::Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, color.gamma_multiply(0.7)))
                .corner_radius(11.0)
                .inner_margin(egui::Margin {
                    left: 14,
                    right: 8,
                    top: 10,
                    bottom: 10,
                })
                .shadow(shadow())
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        ui.label(RichText::new(glyph).color(color).font(mono(22.0)));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.add(
                                egui::Label::new(RichText::new(&msg).font(mono(12.0)).color(TEXT))
                                    .wrap(),
                            );
                            if let Some(name) = saved.as_ref().and_then(|p| p.file_name()) {
                                let name = name.to_string_lossy();
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(name).font(mono(11.0)).color(MUTED),
                                    )
                                    .truncate(),
                                );
                            }
                        });
                        if saved.is_some() {
                            let label = if cfg!(target_os = "macos") {
                                t!("toast.reveal_mac")
                            } else if cfg!(windows) {
                                t!("toast.reveal_win")
                            } else {
                                t!("toast.reveal_linux")
                            };
                            let caps = cmd_caps(Cmd::RevealExport);
                            if button_caps(ui, "", label, false, CHIP, &caps).clicked() {
                                self.run(Cmd::RevealExport, ui.ctx());
                            }
                        }
                        if icon_button_look(ui, icon::X, t!("common.dismiss"), false, ICON_SMALL)
                            .clicked()
                        {
                            self.status = None;
                        }
                    });
                });
        });
    }
}
