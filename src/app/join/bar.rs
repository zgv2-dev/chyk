//! The Join header mode: its button, the bar (add, order, remove, fit, output frame) and the
//! output frame popover.

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
fn fps_label(f: f64) -> String {
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

    /// Output frame: the largest sides, or one clip's size; and the frame rate.
    pub(in crate::app) fn frame_pop(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(r) = self.reel_now() else { return };
        self.pop_head(ui, icon::FRAME_CORNERS, t!("join.frame"), |_| {});
        ui.label(
            RichText::new(t!("join.frame_sub"))
                .font(mono(11.0))
                .color(MUTED),
        );
        ui.add_space(10.0);
        let (big, top) = reel::largest(&r.clips);
        let mut sizes = vec![(t!("join.largest").to_string(), big)];
        for (i, c) in r.clips.iter().enumerate() {
            sizes.push((format!("{}", i + 1), c.info.size));
        }
        let mut chosen = None;
        field_label(ui, icon::FRAME_CORNERS, t!("join.size"));
        for (name, s) in &sizes {
            let text = format!("{name} · {}×{}", s[0], s[1]);
            let on = reel::even(*s) == r.frame;
            let b = button(ui, "", &text, on, CHIP);
            if b.clicked() {
                chosen = Some((*s, r.fps));
            }
        }
        ui.add_space(10.0);
        field_label(ui, icon::FILM_STRIP, t!("join.fps"));
        let mut rates: Vec<f64> = r
            .clips
            .iter()
            .map(|c| c.info.fps)
            .chain([24.0, 30.0, 60.0, top])
            .collect();
        rates.sort_by(f64::total_cmp);
        rates.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        let labels: Vec<String> = rates.iter().map(|&f| fps_label(f)).collect();
        let on = rates
            .iter()
            .position(|f| (f - r.fps).abs() < 0.01)
            .unwrap_or(0);
        if let Some(i) = segmented(ui, "join_fps", &labels, on) {
            chosen = Some((r.frame, rates[i]));
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(t!("join.frame_hint"))
                .font(mono(10.5))
                .color(MUTED),
        );
        if let Some((size, fps)) = chosen {
            self.set_frame(size, fps, ctx);
        }
    }
}
