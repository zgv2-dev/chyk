//! The Join header mode: its button and the bar (add, order, remove, fit, output frame).

use super::lane::fit_label;
use super::place::place_fields;
use super::*;
use crate::ffmpeg::Fit;

const FITS: [Fit; 4] = [Fit::Fit, Fit::Fill, Fit::Stretch, Fit::Place([0, 0])];

/// What the bar was asked to do this frame.
enum Act {
    Add,
    Shift(isize),
    Remove,
    Fit(Fit),
    Place([i32; 2]),
    Frame,
}

fn fit_icon(fit: Fit) -> &'static str {
    match fit {
        Fit::Fit => icon::CORNERS_IN,
        Fit::Fill => icon::CORNERS_OUT,
        Fit::Stretch => icon::ARROWS_OUT_LINE_HORIZONTAL,
        Fit::Place(_) => icon::ARROWS_OUT_CARDINAL,
    }
}

/// A frame rate as short as it reads: `30`, `29.97`.
pub(super) fn fps_label(f: f64) -> String {
    let s = format!("{f:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl App {
    /// J: with one video, pick videos to join; with a join, open or close its bar.
    pub(in crate::app) fn toggle_join(&mut self, ctx: &egui::Context) {
        if self.reel.is_none() {
            return self.add_dialog(ctx);
        }
        if self.panel == Some(Panel::Crop) {
            self.toggle_crop(ctx);
        }
        self.panel = (self.panel != Some(Panel::Join)).then_some(Panel::Join);
    }

    pub(in crate::app) fn join_mode_button(
        &mut self,
        ui: &mut Ui,
        look: Look,
        (icons, caps): (bool, &[String]),
    ) {
        let open = self.panel == Some(Panel::Join);
        let label = if icons { "" } else { t!("mode.join") };
        let resp = button_caps(ui, icon::STACK, label, open, look, caps);
        if icons {
            resp.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, t!("mode.join"))
            });
        }
        applied_dot(ui, resp.rect, self.reel.is_some() && !open);
        if tip_cmds(resp, t!("cmd.join"), &[Cmd::Join]).clicked() {
            self.toggle_join(&ui.ctx().clone());
        }
    }

    pub(in crate::app) fn join_bar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(r) = self.reel_now() else {
            self.panel = None;
            return;
        };
        let mut act = None;
        let pop_open = self.pop == Some(Pop::Frame);
        let mut frame_rect = Rect::NOTHING;
        // Fit modes as icons when the labels don't fit; measured like the Strip bar.
        let avail = ui.available_width();
        let shape = matches!(r.clips[r.sel].fit, Fit::Place(_));
        let key = ui.id().with("join_bar_full");
        let full: Option<(bool, f32)> = ui.data(|d| d.get_temp(key));
        let compact = full.is_some_and(|(s, w)| s == shape && w > avail);
        let w = bar_frame(ui, 34.0, |ui| {
            let add = icon_button_keys(
                ui,
                icon::PLUS_CIRCLE,
                t!("join.add"),
                false,
                ICON_SMALL,
                &[],
            );
            if add.clicked() {
                act = Some(Act::Add);
            }
            vline(ui, 20.0);
            for (glyph, tip, by, cmd) in [
                (
                    icon::ARROW_LINE_LEFT,
                    t!("join.earlier"),
                    -1,
                    Cmd::ClipEarlier,
                ),
                (icon::ARROW_LINE_RIGHT, t!("join.later"), 1, Cmd::ClipLater),
            ] {
                if icon_button_keys(ui, glyph, tip, false, ICON_SMALL, &[cmd]).clicked() {
                    act = Some(Act::Shift(by));
                }
            }
            if icon_button_keys(ui, icon::TRASH, t!("join.remove"), false, ICON_SMALL, &[])
                .clicked()
            {
                act = Some(Act::Remove);
            }
            vline(ui, 20.0);
            ui.add_space(2.0);
            let sel = &r.clips[r.sel];
            for (fit, hint) in FITS.into_iter().zip([
                "join.fit_hint",
                "join.fill_hint",
                "join.stretch_hint",
                "join.place_hint",
            ]) {
                let on = sel.fit.same_mode(fit);
                let b = match compact {
                    true => button(ui, fit_icon(fit), "", on, CHIP),
                    false => button(ui, "", fit_label(fit), on, CHIP),
                };
                let tip = match compact {
                    true => format!("{}: {}", fit_label(fit), i18n::tr(hint)),
                    false => i18n::tr(hint).to_string(),
                };
                if b.on_hover_text(tip).clicked() {
                    act = Some(Act::Fit(fit));
                }
            }
            if let Fit::Place(at) = sel.fit
                && let Some(at) = place_fields(ui, at)
            {
                act = Some(Act::Place(at));
            }
            vline(ui, 20.0);
            // Two decimals at most; then the size alone, then just the icon, to stay clear of Export.
            let fps = fps_label((r.fps * 100.0).round() / 100.0);
            let size = format!("{}×{}", r.frame[0], r.frame[1]);
            // Full width first, so a tight bar turns the fit labels into icons before this.
            let room = if compact {
                ui.available_width() - 48.0
            } else {
                f32::INFINITY
            };
            let label = [format!("{size} · {fps}"), size]
                .into_iter()
                .find(|l| {
                    let w = ui.painter().layout_no_wrap(l.clone(), mono(12.0), TEXT);
                    w.size().x <= room
                })
                .unwrap_or_default();
            let b = button(ui, icon::FRAME_CORNERS, &label, pop_open, CHIP);
            frame_rect = b.rect;
            if b.on_hover_text(t!("join.frame")).clicked() {
                act = Some(Act::Frame);
            }
        });
        if !compact {
            ui.data_mut(|d| d.insert_temp(key, (shape, w)));
            if w > avail + 0.5 {
                ctx.request_discard("join bar too wide");
            }
        }
        self.anchors.frame = frame_rect;
        match act {
            Some(Act::Add) => self.add_dialog(ctx),
            Some(Act::Shift(by)) => self.shift_clip(by),
            Some(Act::Remove) => self.remove_clip(r.sel),
            Some(Act::Fit(f)) => self.set_fit(f),
            Some(Act::Place(at)) => self.place_at(at),
            Some(Act::Frame) => self.toggle_pop(Pop::Frame),
            None => {}
        }
    }
}
