use super::*;

impl App {
    pub(super) fn status_pill(&mut self, ui: &mut Ui) {
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
        let area = egui::Area::new("status".into())
            // Bottom center, above the zoom bar's row: the top belongs to the crop and strip bars.
            .fixed_pos(pos2(central.center().x, central.max.y - 64.0))
            .pivot(Align2::CENTER_BOTTOM)
            .order(egui::Order::Foreground);
        area.show(ui.ctx(), |ui| {
            ui.set_opacity(opacity);
            let (glyph, color) = if err {
                (icon::WARNING, RED)
            } else {
                (icon::CHECK, GREEN)
            };
            // Never wider than the window; long messages wrap.
            ui.set_max_width(480.0f32.min(central.width() - 32.0));
            egui::Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, color.gamma_multiply(0.7)))
                .corner_radius(8.0)
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 4,
                    top: 3,
                    bottom: 3,
                })
                .shadow(shadow())
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(glyph).color(color).font(mono(15.0)));
                        // Wraps before the dismiss button; short messages stay compact.
                        let width = ui.available_width() - ICON_SMALL.height - 8.0;
                        ui.scope(|ui| {
                            ui.set_max_width(width.max(40.0));
                            ui.add(
                                egui::Label::new(RichText::new(msg).font(mono(12.0)).color(TEXT))
                                    .wrap(),
                            );
                        });
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
