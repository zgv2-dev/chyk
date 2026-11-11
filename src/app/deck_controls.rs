use super::*;

impl App {
    pub(super) fn speed_control(&mut self, ui: &mut Ui) {
        let speed = self.speed;
        let p = ui.painter();
        let parts = [
            (icon::GAUGE.to_string(), mono(14.0), GREEN),
            (
                speed_label(speed),
                mono(12.0),
                if (speed - 1.0).abs() > 1e-4 {
                    GREEN
                } else {
                    TEXT
                },
            ),
        ];
        let galleys: Vec<_> = parts
            .into_iter()
            .map(|(s, f, c)| (p.layout_no_wrap(s, f, c), c))
            .collect();
        let w = galleys.iter().map(|(g, _)| g.size().x).sum::<f32>() + 6.0 + 24.0;
        // The menu is as wide as the button, unless its longest entry needs more.
        let widest = SPEEDS
            .iter()
            .map(|&s| p.layout_no_wrap(speed_label(s), mono(12.0), TEXT).size().x)
            .fold(0.0, f32::max);
        let menu_w = w.max(widest + 2.0 * 8.0 + 6.0);
        let (r, resp) = ui.allocate_exact_size(vec2(w, DECK_H), Sense::click());
        let edge = if resp.hovered() {
            GREEN.gamma_multiply(0.5)
        } else {
            LINE
        };
        let p = ui.painter();
        p.rect(
            r,
            7.0,
            CARD,
            Stroke::new(1.0, edge),
            egui::StrokeKind::Inside,
        );
        let mut x = r.min.x + 12.0;
        for (g, c) in galleys {
            let gw = g.size().x;
            p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, c);
            x += gw + 6.0;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let resp = tip_cmds(resp, t!("tip.speed"), &[Cmd::Slower, Cmd::Faster]);
        let frame = egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(7)
            .inner_margin(3);
        egui::Popup::menu(&resp)
            .width(menu_w)
            .gap(4.0)
            .frame(frame)
            .show(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for &s in SPEEDS {
                    let on = (speed - s).abs() < 1e-6;
                    let (row, item) =
                        ui.allocate_exact_size(vec2(menu_w - 6.0, 22.0), Sense::click());
                    let p = ui.painter();
                    if on || item.hovered() {
                        let fill = if on {
                            GREEN.gamma_multiply(0.11)
                        } else {
                            CARD_HI
                        };
                        p.rect_filled(row, 5.0, fill);
                    }
                    p.text(
                        pos2(row.min.x + 8.0, row.center().y),
                        Align2::LEFT_CENTER,
                        speed_label(s),
                        mono(12.0),
                        if on { GREEN } else { TEXT },
                    );
                    if item.clicked() {
                        self.set_speed(s);
                        ui.close();
                    }
                }
            });
    }

    /// IN / OUT / LEN fields in one box: icon, then a draggable, typeable time.
    pub(super) fn selection_control(&mut self, ui: &mut Ui, dur: f64) {
        let (mut a, mut b) = self.trim;
        let (mut new_in, mut new_out, mut new_len) = (None, None, None);
        // The deck's right side lays out right to left; give the box its own left-to-right
        // row, sized from the last frame (egui's usual two-pass trick).
        let width = self.anchors.selection_width;
        let shown = ui.allocate_ui_with_layout(
            vec2(width, DECK_H),
            Layout::left_to_right(Align::Center),
            |ui| {
                egui::Frame::new()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(7.0)
                    .inner_margin(egui::Margin::symmetric(4, 0))
                    .show(ui, |ui| {
                        // Inside the 1 pt stroke, so the box is DECK_H overall like its neighbors.
                        let inner = DECK_H - 2.0;
                        ui.set_height(inner);
                        // A fixed-height row: `horizontal` centers items on the row height so
                        // far, so items before the separators sat higher than the rest.
                        let row = vec2(ui.available_width(), inner);
                        ui.allocate_ui_with_layout(
                            row,
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.spacing_mut().interact_size.y = 22.0;
                                let v = ui.visuals_mut();
                                v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                                v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                                v.widgets.inactive.bg_stroke = Stroke::NONE;
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let field_icon = |ui: &mut Ui, glyph: &str, tip: &str| {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(glyph).font(mono(14.0)).color(GREEN_DIM),
                                    )
                                    .on_hover_text(tip);
                                };
                                field_icon(ui, icon::SIGN_IN, t!("tip.in"));
                                if time_field(ui, &mut a, dur, "", t!("tip.in")) {
                                    new_in = Some(a);
                                }
                                vline(ui, inner);
                                field_icon(ui, icon::SIGN_OUT, t!("tip.out"));
                                if time_field(ui, &mut b, dur, "", t!("tip.out")) {
                                    new_out = Some(b);
                                }
                                vline(ui, inner);
                                field_icon(ui, icon::RULER, t!("tip.length"));
                                let mut len = b - a;
                                if time_field(ui, &mut len, dur - a, "", t!("tip.length")) {
                                    new_len = Some(len);
                                }
                            },
                        );
                    })
                    .response
                    .rect
                    .width()
            },
        );
        if (shown.inner - width).abs() > 0.5 {
            self.anchors.selection_width = shown.inner;
            ui.ctx().request_repaint();
        }
        if let Some(a) = new_in {
            self.set_in(a);
        }
        if let Some(b) = new_out {
            self.set_out(b);
        }
        if let Some(len) = new_len {
            self.set_out(a + len);
        }
    }
}
