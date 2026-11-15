//! Place: a clip at its own size anywhere in the output frame, set by X/Y or by dragging it.

use super::field::number_box;
use super::*;
use crate::ffmpeg::Fit;

/// X and Y boxes for the bar; the typed spot once committed.
pub(super) fn place_fields(ui: &mut Ui, at: [i32; 2]) -> Option<[i32; 2]> {
    let mut out = None;
    ui.add_space(6.0);
    for (i, axis) in ["X", "Y"].into_iter().enumerate() {
        ui.label(RichText::new(axis).font(mono(11.0)).color(MUTED));
        let (r, _) = ui.allocate_exact_size(vec2(58.0, 26.0), Sense::hover());
        let id = format!("join_place_{axis}");
        if let Some(v) = number_box(ui, r, &id, Some(at[i].to_string()), axis, false) {
            let mut to = out.unwrap_or(at);
            to[i] = v.round() as i32;
            out = Some(to);
        }
        ui.add_space(4.0);
    }
    out
}

impl App {
    /// Move the selected placed clip to `at`, kept at least 2 px inside the frame.
    pub(in crate::app) fn place_at(&mut self, at: [i32; 2]) {
        let Some(mut r) = self.reel_now() else { return };
        let sel = r.sel;
        let [w, h] = r.clips[sel].info.size.map(|v| v as i32);
        let [fw, fh] = r.frame.map(|v| v as i32);
        let at = [at[0].clamp(2 - w, fw - 2), at[1].clamp(2 - h, fh - 2)];
        r.clips[sel].fit = Fit::Place(at);
        r.shown = None;
        self.reel = Some(r);
    }

    /// With the Join bar open and the selected placed clip on screen, dragging the stage moves
    /// it. Returns whether this drag is a placement.
    pub(in crate::app) fn place_drag(
        &mut self,
        ui: &Ui,
        resp: &Response,
        vr: Rect,
        src: Rect,
    ) -> bool {
        let Some(r) = &self.reel else { return false };
        let Fit::Place(at) = r.clips[r.sel].fit else {
            return false;
        };
        let live = self.panel == Some(Panel::Join) && r.shown == Some(r.sel);
        if !live && self.drag != Some(Drag::Place) {
            return false;
        }
        // Frame pixels per screen point.
        let k = vec2(
            r.frame[0] as f32 * src.width() / vr.width(),
            r.frame[1] as f32 * src.height() / vr.height(),
        );
        let id = ui.id().with("join_place_drag");
        if resp.drag_started() {
            self.drag = Some(Drag::Place);
            ui.data_mut(|d| d.insert_temp(id, vec2(at[0] as f32, at[1] as f32)));
        }
        if self.drag != Some(Drag::Place) {
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Move);
            }
            return false;
        }
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        if resp.dragged() {
            let to =
                ui.data(|d| d.get_temp::<Vec2>(id)).unwrap_or_default() + resp.drag_delta() * k;
            ui.data_mut(|d| d.insert_temp(id, to));
            self.place_at([to.x.round() as i32, to.y.round() as i32]);
        }
        if resp.drag_stopped() {
            self.drag = None;
        }
        true
    }
}
