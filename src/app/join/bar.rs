//! The Join header mode: its button and the bar (add, order, remove, fit, output frame).

use super::lane::fit_label;
use super::*;
use crate::ffmpeg::Fit;

const FITS: [Fit; 3] = [Fit::Fit, Fit::Fill, Fit::Stretch];

/// What the bar was asked to do this frame.
enum Act {
    Add,
    Shift(isize),
    Remove,
    Fit(Fit),
    Frame,
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

    pub(in crate::app) fn join_mode_button(&mut self, ui: &mut Ui, look: Look, caps: &[String]) {
        let open = self.panel == Some(Panel::Join);
        let resp = button_caps(ui, icon::STACK, t!("mode.join"), open, look, caps);
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
        bar_frame(ui, 34.0, |ui| {
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
            for (fit, hint) in
                FITS.into_iter()
                    .zip(["join.fit_hint", "join.fill_hint", "join.stretch_hint"])
            {
                let b = button(ui, "", fit_label(fit), sel.fit == fit, CHIP);
                if b.on_hover_text(i18n::tr(hint)).clicked() {
                    act = Some(Act::Fit(fit));
                }
            }
            vline(ui, 20.0);
            let label = format!("{}×{} · {}", r.frame[0], r.frame[1], fps_label(r.fps));
            let b = button(ui, icon::FRAME_CORNERS, &label, pop_open, CHIP);
            frame_rect = b.rect;
            if b.on_hover_text(t!("join.frame")).clicked() {
                act = Some(Act::Frame);
            }
        });
        self.anchors.frame = frame_rect;
        match act {
            Some(Act::Add) => self.add_dialog(ctx),
            Some(Act::Shift(by)) => self.shift_clip(by),
            Some(Act::Remove) => self.remove_clip(r.sel),
            Some(Act::Fit(f)) => self.set_fit(f),
            Some(Act::Frame) => self.toggle_pop(Pop::Frame),
            None => {}
        }
    }
}
