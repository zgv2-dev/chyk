use super::*;

impl App {
    pub(super) fn toggle_pop(&mut self, pop: Pop) {
        self.pop = (self.pop != Some(pop)).then_some(pop);
    }

    /// The open popover over a dimmed window, with its arrow on the button that opened it.
    pub(super) fn popover(&mut self, ctx: &egui::Context) {
        let Some(pop) = self.pop else { return };
        if self.video.is_none() {
            self.pop = None;
            return;
        }
        let screen = ctx.content_rect();
        let scrim = egui::Area::new("scrim".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(r, 0.0, Color32::from_black_alpha(115));
                resp
            })
            .inner;
        if scrim.clicked() {
            self.pop = None;
            return;
        }
        let (anchor, width) = match pop {
            Pop::Export => (self.anchors.export, 360.0),
            Pop::Info => (self.anchors.info, 420.0),
        };
        let left = match pop {
            Pop::Export => anchor.max.x - width,
            Pop::Info => anchor.center().x - 120.0,
        }
        .clamp(
            screen.min.x + 12.0,
            (screen.max.x - width - 12.0).max(screen.min.x + 12.0),
        );
        let top = self.anchors.header_bottom + 6.0;
        let shown = egui::Area::new(egui::Id::new("popover"))
            .fixed_pos(pos2(left, top))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(12.0)
                    .inner_margin(18)
                    .shadow(shadow())
                    .show(ui, |ui| {
                        ui.set_width(width - 36.0);
                        // Room left above the window's bottom edge, inside the 18 pt padding.
                        let max_h = (screen.max.y - top - 12.0 - 36.0).max(120.0);
                        match pop {
                            Pop::Export => self.export_pop(ui, ctx, max_h),
                            // The info popover scrolls below its title row (~50 pt).
                            Pop::Info => self.info_pop(ui, ctx, max_h - 50.0),
                        }
                    });
            });
        ctx.move_to_top(shown.response.layer_id);
        let r = shown.response.rect;
        let cx = anchor.center().x.clamp(r.min.x + 18.0, r.max.x - 18.0);
        let p = ctx.layer_painter(shown.response.layer_id);
        let (a, tip, b) = (
            // The base sits below the frame's 1 px top border, so no line shows across it.
            pos2(cx - 7.0, r.min.y + 1.5),
            pos2(cx, r.min.y - 6.5),
            pos2(cx + 7.0, r.min.y + 1.5),
        );
        p.add(egui::Shape::convex_polygon(
            vec![a, tip, b],
            PANEL,
            Stroke::NONE,
        ));
        p.line_segment([a, tip], Stroke::new(1.0, LINE));
        p.line_segment([tip, b], Stroke::new(1.0, LINE));
    }

    pub(super) fn pop_head(
        &mut self,
        ui: &mut Ui,
        glyph: &str,
        title: &str,
        extra: impl FnOnce(&mut Ui),
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 9.0;
            ui.label(RichText::new(glyph).font(mono(18.0)).color(GREEN));
            ui.add(egui::Label::new(RichText::new(title).font(bold(14.0)).color(TEXT)).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if icon_button(ui, icon::X, t!("common.close"), false).clicked() {
                    self.pop = None;
                }
                extra(ui);
            });
        });
    }
}
