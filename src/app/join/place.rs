//! Place: a clip at its own size anywhere in the output frame, set by X/Y or by dragging it.

use super::field::number_box;
use super::*;
use crate::ffmpeg::Fit;

/// How close (screen points) a dragged clip comes to a frame edge or the center to snap there.
const SNAP: f32 = 8.0;

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
        let ([fw, fh], size) = (r.frame.map(|v| v as f32), r.clips[r.sel].info.size);
        // Frame pixels to screen: the output frame's box, and the clip's inside it.
        let to_screen = |x: f32, y: f32| {
            let u = vec2(x / fw, y / fh) - src.min.to_vec2();
            vr.min
                + vec2(
                    u.x / src.width() * vr.width(),
                    u.y / src.height() * vr.height(),
                )
        };
        let [cx, cy] = at.map(|v| v as f32);
        let [w, h] = size.map(|v| v as f32);
        let p = ui.painter_at(ui.clip_rect());
        let frame = Rect::from_two_pos(to_screen(0.0, 0.0), to_screen(fw, fh));
        let clip = Rect::from_two_pos(to_screen(cx, cy), to_screen(cx + w, cy + h));
        p.rect_stroke(
            frame,
            0.0,
            Stroke::new(1.0, MUTED),
            egui::StrokeKind::Outside,
        );
        let edge = Stroke::new(1.0, GREEN.gamma_multiply(0.7));
        p.rect_stroke(clip, 0.0, edge, egui::StrokeKind::Inside);
        // Frame pixels per screen point.
        let k = vec2(
            fw * src.width() / vr.width(),
            fh * src.height() / vr.height(),
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
            // Within 8 pt of the frame's edges or center, the clip snaps there.
            let snap = |v: f32, side: f32, f: f32, k: f32| {
                [0.0, (f - side) / 2.0, f - side]
                    .into_iter()
                    .find(|c| (v - c).abs() <= SNAP * k)
                    .unwrap_or(v)
            };
            let to = [snap(to.x, w, fw, k.x), snap(to.y, h, fh, k.y)];
            self.place_at(to.map(|v| v.round() as i32));
        }
        if resp.drag_stopped() {
            self.drag = None;
        }
        true
    }
}
