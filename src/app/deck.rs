use super::*;

impl App {
    /// Transport row over the trim bar.
    pub(super) fn deck(&mut self, ui: &mut Ui) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 40.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // Everything when it fits; else no total time and no zoom-fit button. The full
                // width is measured when drawn; an overflowing frame is discarded and redrawn.
                let row = ui.available_width();
                let compact = self.deck_need > row;
                if play_button(ui, self.paused()).clicked() {
                    self.toggle_play();
                }
                ui.add_space(4.0);
                if icon_button_keys(
                    ui,
                    icon::CLOCK_COUNTER_CLOCKWISE,
                    t!("tip.back"),
                    false,
                    DECK_ICON,
                    &[Cmd::Back5],
                )
                .clicked()
                {
                    self.skip(-5.0);
                }
                if icon_button_keys(
                    ui,
                    icon::CLOCK_CLOCKWISE,
                    t!("tip.forward"),
                    false,
                    DECK_ICON,
                    &[Cmd::Forward5],
                )
                .clicked()
                {
                    self.skip(5.0);
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(fmt_time(self.pos()))
                        .font(bold(19.0))
                        .color(Color32::WHITE),
                );
                if !compact {
                    ui.label(
                        RichText::new(format!("/ {}", fmt_time(dur)))
                            .font(mono(11.0))
                            .color(MUTED),
                    );
                }
                ui.add_space(10.0);
                self.speed_control(ui);
                ui.add_space(6.0);
                self.video_zoom_controls(ui, compact);
                let left = ui.min_rect().width();
                let right = ui
                    .with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        self.timeline_zoom_controls(ui, dur);
                        ui.add_space(10.0);
                        self.selection_control(ui, dur);
                        ui.min_rect().width()
                    })
                    .inner;
                if !compact {
                    self.deck_need = left + right + 12.0;
                    if self.deck_need > row + 0.5 {
                        ui.ctx().request_discard("transport row too wide");
                    }
                }
            },
        );
        ui.add_space(4.0);
        self.trim_bar(ui);
    }

    pub(super) fn timeline_zoom_controls(&mut self, ui: &mut Ui, dur: f64) {
        let z = self.timeline.zoom;
        let (v0, v1) = self.timeline.view(dur);
        let view = format!("{} · {} – {}", zoom_label(z), fmt_time(v0), fmt_time(v1));
        // Right to left: the rightmost control comes first.
        if icon_button_keys(
            ui,
            icon::ARROWS_OUT_LINE_HORIZONTAL,
            &t!("tip.timeline_fit", view = view),
            z <= 1.001,
            DECK_ICON,
            &[Cmd::TimelineWhole],
        )
        .clicked()
        {
            self.fit_timeline(dur);
        }
        if icon_button_keys(
            ui,
            icon::ARROWS_IN_LINE_HORIZONTAL,
            &t!("tip.timeline_selection", view = view),
            false,
            DECK_ICON,
            &[Cmd::TimelineToSelection],
        )
        .clicked()
        {
            self.fit_timeline_selection(dur);
        }
        if icon_button_keys(
            ui,
            icon::MAGNIFYING_GLASS_PLUS,
            &t!("tip.timeline_in", view = view),
            false,
            DECK_ICON,
            &[Cmd::TimelineIn],
        )
        .clicked()
        {
            self.zoom_timeline(1.5, dur);
        }
        if icon_button_keys(
            ui,
            icon::MAGNIFYING_GLASS_MINUS,
            &t!("tip.timeline_out", view = view),
            false,
            DECK_ICON,
            &[Cmd::TimelineOut],
        )
        .clicked()
        {
            self.zoom_timeline(1.0 / 1.5, dur);
        }
    }

    /// Under a zoomed timeline: which part of the video is in view, as a scrollbar thumb (with the
    /// playhead as a tick). Click or drag to move the view.
    pub(super) fn timeline_scroll(&mut self, ui: &Ui, strip: Rect, dur: f64) {
        if self.timeline.zoom <= 1.001 {
            return;
        }
        // In the deck's bottom margin, so showing it doesn't move anything.
        let track = Rect::from_x_y_ranges(strip.x_range(), strip.max.y + 4.0..=strip.max.y + 12.0);
        let resp = ui.interact(
            track,
            ui.id().with("timeline_scroll"),
            Sense::click_and_drag(),
        );
        let at = |t: f64| track.min.x + (t / dur) as f32 * track.width();
        let (v0, v1) = self.timeline.view(dur);
        // Pressing beside the thumb centers the view there; dragging then moves it by the
        // pointer's movement, so the thumb stays where it was grabbed.
        let pressed = resp.contains_pointer() && ui.input(|i| i.pointer.primary_pressed());
        if pressed
            && let Some(p) = resp.hover_pos()
            && !(at(v0)..=at(v1)).contains(&p.x)
        {
            let f = ((p.x - track.min.x) / track.width()).clamp(0.0, 1.0) as f64;
            self.pan_timeline(f * dur, dur);
        } else if resp.dragged() {
            let d = resp.drag_delta().x as f64 / track.width() as f64 * dur;
            self.pan_timeline((v0 + v1) / 2.0 + d, dur);
        }
        let (v0, v1) = self.timeline.view(dur);
        let hot = resp.hovered() || resp.dragged();
        let h = if hot { 4.0 } else { 2.0 };
        let y = track.center().y;
        let rail = |a: f32, b: f32| Rect::from_x_y_ranges(a..=b, y - h / 2.0..=y + h / 2.0);
        let p = ui.painter();
        p.rect_filled(rail(track.min.x, track.max.x), 2.0, LINE);
        let (a, b) = (at(v0), at(v1).max(at(v0) + 6.0));
        p.rect_filled(rail(a, b), 2.0, if hot { GREEN } else { GREEN_DIM });
        let x = at(self.pos());
        p.vline(x, y - 4.0..=y + 4.0, Stroke::new(1.0, TEXT));
        if hot {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.on_hover_text(format!(
            "{} · {} – {}",
            zoom_label(self.timeline.zoom),
            fmt_time(v0),
            fmt_time(v1)
        ));
    }
}
