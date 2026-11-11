use super::*;

impl App {
    /// The video, fitted to the panel, with the crop overlay on top.
    pub(super) fn stage(&mut self, ui: &mut Ui) {
        let (aspect, source) = self
            .video
            .as_ref()
            .map_or((16.0 / 9.0, [0, 0]), |v| (v.aspect, v.size));
        // An applied crop (panel closed) frames the preview to just the cropped part; while the
        // Crop panel is open the whole frame shows, with the selection box.
        let editing_crop = self.panel == Some(Panel::Crop);
        let view = if editing_crop { None } else { self.crop };
        let shown = view.map_or(aspect, |c| aspect * c.width() / c.height().max(1e-6));
        let avail = ui.max_rect().shrink(12.0);
        let size = if avail.width() / avail.height() > shown {
            vec2(avail.height() * shown, avail.height())
        } else {
            vec2(avail.width(), avail.width() / shown)
        };
        let vr = Rect::from_center_size(avail.center(), size);
        // Zoom keys and buttons run elsewhere; enforce the crop bound before drawing.
        if editing_crop {
            self.contain_crop();
        }
        let src = self.shown_src();
        self.player.paint(ui, vr, src, source, aspect);

        let resp = ui.interact(
            vr.expand(20.0),
            ui.id().with("stage"),
            Sense::click_and_drag(),
        );
        // Pinch or Ctrl/⌘+wheel, and plain wheel or two-finger scroll: up zooms in.
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        let zoom_delta = ui.input(|i| i.zoom_delta()) * (scroll * 0.0015).exp();
        if resp.hovered() && (zoom_delta - 1.0).abs() > 1e-3 {
            let p = resp.hover_pos().unwrap_or(vr.center());
            self.zoom_video(zoom_delta, Some((p, vr)));
            if editing_crop {
                self.contain_crop();
            }
        }
        if self.strip.edit {
            self.zone_stage(ui, vr, src, resp);
            return;
        }
        let (true, Some(c)) = (editing_crop, self.crop) else {
            if resp.drag_started() {
                self.drag = (self.video_zoom > 1.001).then_some(Drag::Pan);
            }
            if resp.dragged() && self.drag == Some(Drag::Pan) {
                self.pan_video(resp.drag_delta(), vr);
            }
            if resp.drag_stopped() {
                self.drag = None;
            }
            if resp.clicked() {
                self.toggle_play();
            }
            if self.drag == Some(Drag::Pan) {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if self.video_zoom > 1.001 && resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
            self.draw_zones(ui, vr, src, false);
            return;
        };
        self.crop_stage(ui, vr, src, resp, c);
    }

    /// Video zoom: out, level (click to fit), in, and a fit button unless `compact`.
    pub(super) fn video_zoom_controls(&mut self, ui: &mut Ui, compact: bool) {
        bar_frame(ui, DECK_H, |ui| {
            if icon_button_keys(
                ui,
                icon::MINUS,
                t!("tip.zoom_out"),
                false,
                ICON_SMALL,
                &[Cmd::ZoomOut],
            )
            .clicked()
            {
                self.zoom_video(0.8, None);
            }
            let zoomed = self.video_zoom > 1.001;
            let (r, resp) = ui.allocate_exact_size(vec2(44.0, 26.0), Sense::click());
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                zoom_label(self.video_zoom),
                mono(11.0),
                if zoomed || resp.hovered() {
                    GREEN
                } else {
                    MUTED
                },
            );
            let tip = format!("{} · {}", t!("tip.zoom"), t!("tip.zoom_fit"));
            if tip_keys(resp, &tip, Cmd::ZoomFit).clicked() {
                self.reset_video_zoom();
            }
            if icon_button_keys(
                ui,
                icon::PLUS,
                t!("tip.zoom_in"),
                false,
                ICON_SMALL,
                &[Cmd::ZoomIn],
            )
            .clicked()
            {
                self.zoom_video(1.25, None);
            }
            if !compact
                && icon_button_keys(
                    ui,
                    icon::CORNERS_IN,
                    t!("tip.zoom_fit"),
                    false,
                    ICON_SMALL,
                    &[Cmd::ZoomFit],
                )
                .clicked()
            {
                self.reset_video_zoom();
            }
        });
    }
}
