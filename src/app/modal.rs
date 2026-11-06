use super::*;

impl App {
    /// "Open another video": replace the current one, or open it in a new window.
    /// The center modal: replace-or-new-window for a second file, or unexported edits on close.
    pub(super) fn open_modal(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let screen = ctx.content_rect();
        egui::Area::new("modal_scrim".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(r, 0.0, Color32::from_black_alpha(150));
            });
        let (glyph, title, hint, second, first) = match &confirm {
            Confirm::Open(path) => {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into(),
                );
                (
                    icon::FILM_SLATE,
                    t!("open.another", name = name).to_string(),
                    t!("open.another_hint"),
                    (icon::SWAP, t!("open.replace")),
                    (icon::APP_WINDOW, t!("open.new_window")),
                )
            }
            Confirm::Quit | Confirm::CloseVideo => (
                icon::WARNING,
                t!("unsaved.title").to_string(),
                t!("unsaved.hint"),
                if confirm == Confirm::Quit {
                    (icon::SIGN_OUT, t!("unsaved.quit"))
                } else {
                    (icon::X_CIRCLE, t!("unsaved.close"))
                },
                (icon::EXPORT, t!("export.button")),
            ),
        };
        let mut choice = None;
        let shown = egui::Area::new("open_modal".into())
            .fixed_pos(screen.center())
            .pivot(Align2::CENTER_CENTER)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(12.0)
                    .inner_margin(20)
                    .shadow(shadow())
                    .show(ui, |ui| {
                        ui.set_width(380.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            ui.label(RichText::new(glyph).font(mono(20.0)).color(GREEN));
                            ui.add(
                                egui::Label::new(RichText::new(title).font(bold(13.0)).color(TEXT))
                                    .truncate(),
                            );
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new(hint).font(mono(11.5)).color(MUTED));
                        ui.add_space(14.0);
                        // One right-to-left row so all three buttons share a center line.
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let w = ui
                                .painter()
                                .layout_no_wrap(first.1.into(), bold(12.5), INK)
                                .size()
                                .x
                                + 56.0;
                            if primary(ui, first.0, first.1, vec2(w, 34.0), false).clicked() {
                                choice = Some(2);
                            }
                            if accent(ui, second.0, second.1).clicked() {
                                choice = Some(1);
                            }
                            let cancel = ui
                                .painter()
                                .layout_no_wrap(t!("common.cancel").into(), mono(GHOST.text), TEXT)
                                .size()
                                .x
                                + GHOST.pad * 2.0;
                            ui.add_space((ui.available_width() - cancel).max(0.0));
                            if button(ui, "", t!("common.cancel"), false, GHOST).clicked() {
                                choice = Some(0);
                            }
                        });
                    });
            });
        ctx.move_to_top(shown.response.layer_id);
        let Some(choice) = choice else { return };
        self.confirm = None;
        match (confirm, choice) {
            (Confirm::Open(path), 1) => self.open(path),
            (Confirm::Open(path), 2) => self.open_in_new_window(path),
            (Confirm::Quit, 1) => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (Confirm::CloseVideo, 1) => self.close(ctx),
            (Confirm::Quit | Confirm::CloseVideo, 2) => self.pop = Some(Pop::Export),
            _ => {}
        }
    }
}
