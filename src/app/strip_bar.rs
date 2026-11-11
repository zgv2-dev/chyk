use super::*;

impl App {
    /// Strip bar: ignored areas, padding, then the scan state and its action.
    /// `compact`: icon-only buttons, short bars, no total, for narrow windows.
    pub(super) fn strip_controls(&mut self, ui: &mut Ui, ctx: &egui::Context, compact: bool) {
        let label = |l: &'static str| if compact { "" } else { l };
        let bar_w = if compact { 30.0 } else { 90.0 };
        let add = button(
            ui,
            icon::EYE_SLASH,
            label(t!("strip.add_zone")),
            false,
            GHOST,
        );
        if tip_keys(add, t!("tip.add_zone"), Cmd::AddZone).clicked() {
            self.add_zone();
        }
        if !self.strip.zones.is_empty() {
            let tip = format!(
                "{} · {}",
                t!("strip.edit_zones"),
                tn!("strip.zones", self.strip.zones.len())
            );
            if icon_button_keys(
                ui,
                icon::PENCIL_SIMPLE,
                &tip,
                self.strip.edit,
                ICON_SMALL,
                &[Cmd::EditZones],
            )
            .clicked()
            {
                self.toggle_zone_edit();
            }
            if icon_button_look(ui, icon::TRASH, t!("strip.clear"), false, ICON_SMALL).clicked() {
                self.clear_zones();
            }
        }
        ui.add_space(6.0);
        vline(ui, 20.0);
        ui.add_space(6.0);
        ui.label(
            RichText::new(icon::ARROWS_HORIZONTAL)
                .font(mono(15.0))
                .color(MUTED),
        )
        .on_hover_text(t!("tip.pad"));
        ui.add_space(4.0);
        let step = |pad: f64, d: f64| ((pad + d) * 100.0).round() / 100.0;
        if icon_button_look(ui, icon::MINUS, t!("tip.pad"), false, ICON_SMALL).clicked() {
            self.strip.pad = step(self.strip.pad, -0.05).max(0.0);
        }
        ui.scope(|ui| {
            let w = &mut ui.visuals_mut().widgets;
            w.inactive.bg_fill = CARD;
            w.inactive.weak_bg_fill = CARD;
            w.inactive.bg_stroke = Stroke::new(1.0, LINE);
            w.inactive.corner_radius = 6.into();
            w.hovered.corner_radius = 6.into();
            w.active.corner_radius = 6.into();
            // Text must fit inside the 26 pt box, or the frame's top edge gets clipped.
            ui.style_mut().override_font_id = Some(mono(11.0));
            ui.spacing_mut().button_padding = vec2(8.0, 2.0);
            ui.spacing_mut().interact_size.y = 26.0;
            ui.add_sized(
                vec2(if compact { 54.0 } else { 60.0 }, 26.0),
                egui::DragValue::new(&mut self.strip.pad)
                    .range(0.0..=30.0)
                    .speed(0.01)
                    .fixed_decimals(2)
                    .suffix(format!(" {}", t!("unit.s"))),
            )
            .on_hover_text(t!("tip.pad"));
        });
        if icon_button_look(ui, icon::PLUS, t!("tip.pad"), false, ICON_SMALL).clicked() {
            self.strip.pad = step(self.strip.pad, 0.05).min(30.0);
        }
        ui.add_space(6.0);
        vline(ui, 20.0);
        ui.add_space(6.0);

        if self.strip.job.is_some() {
            let (r, _) = ui.allocate_exact_size(vec2(bar_w, 26.0), Sense::hover());
            let bar = Rect::from_center_size(r.center(), vec2(bar_w, 5.0));
            let p = ui.painter();
            p.rect_filled(bar, 3.0, CARD_HI);
            // Same pace as the timeline's scan line.
            let (a, b) = self.strip.scan_range;
            let frac = ((self.strip.front.shown - a) / (b - a).max(1e-6)).clamp(0.0, 1.0) as f32;
            progress_fill(p, bar, frac, 3.0, GREEN);
            ui.add_space(6.0);
            let pct = format!("{:.0}", frac * 100.0);
            let text = if compact {
                format!("{pct}%")
            } else {
                t!("strip.scanning", pct = pct).to_string()
            };
            ui.label(RichText::new(text).font(mono(11.0)).color(GREEN));
            if icon_button_look(ui, icon::X, t!("tip.cancel_scan"), false, ICON_SMALL).clicked() {
                self.cancel_scan();
                // Don't extend again by itself until the trim or padding changes.
                self.strip.held = Some(self.scan_need());
            }
            return;
        }
        let rescan = self.strip.scan.is_some();
        if let Some(scan) = &self.strip.scan {
            let keep = strip::keep(&scan.active, self.strip.pad, self.trim.0, self.trim.1);
            let kept: f64 = keep.iter().map(|(a, b)| b - a).sum();
            let total = (self.trim.1 - self.trim.0).max(0.0);
            ui.label(RichText::new(icon::SCISSORS).font(mono(15.0)).color(RED));
            ui.add_space(4.0);
            if kept <= 0.0 {
                ui.label(
                    RichText::new(t!("strip.nothing"))
                        .font(mono(11.0))
                        .color(RED),
                );
            } else {
                let (r, resp) = ui.allocate_exact_size(vec2(bar_w, 26.0), Sense::hover());
                let bar = Rect::from_center_size(r.center(), vec2(bar_w, 5.0));
                let p = ui.painter();
                p.rect_filled(bar, 3.0, RED.gamma_multiply(0.35));
                let frac = (kept / total.max(1e-9)) as f32;
                progress_fill(p, bar, frac, 3.0, GREEN);
                let pct = format!("{:.0}", kept / total.max(1e-9) * 100.0);
                resp.on_hover_text(t!("strip.keep", pct = pct, time = fmt_time(kept)));
                ui.add_space(4.0);
                ui.label(RichText::new(fmt_time(kept)).font(mono(11.0)).color(TEXT));
                if !compact {
                    ui.label(
                        RichText::new(format!("/ {}", fmt_time(total)))
                            .font(mono(11.0))
                            .color(MUTED),
                    );
                }
            }
            ui.add_space(6.0);
        }
        let (glyph, label) = if rescan {
            (icon::ARROWS_CLOCKWISE, t!("strip.rescan"))
        } else {
            (icon::MAGNIFYING_GLASS, t!("strip.scan"))
        };
        let tip = if compact { label } else { t!("tip.scan") };
        let scan = accent(ui, glyph, if compact { "" } else { label });
        if tip_keys(scan, tip, Cmd::Rescan).clicked() {
            self.start_scan(ctx);
        }
    }
}
