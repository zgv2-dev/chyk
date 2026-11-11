//! The Recent list on the start screen.

use super::recent_store::{HEAD, ROW, ago, clock, now};
use super::*;

impl App {
    /// ↑/↓ move the highlight, Enter opens it; only when nothing else wants the keys.
    fn recent_keys(&mut self, ui: &Ui, n: usize) {
        let busy = ui.ctx().egui_wants_keyboard_input()
            || self.loading.is_some()
            || self.sheet.is_some()
            || self.confirm.is_some()
            || self.pop.is_some();
        if busy || n == 0 {
            return;
        }
        let key = |k| ui.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        self.recent_sel = self.recent_sel.min(n - 1);
        if key(Key::ArrowDown) {
            self.recent_sel = (self.recent_sel + 1) % n;
        }
        if key(Key::ArrowUp) {
            self.recent_sel = (self.recent_sel + n - 1) % n;
        }
        if key(Key::Enter) {
            self.open_recent(self.recent_sel);
        }
    }

    /// Draw the list at `top`, `r.width()` wide.
    pub(super) fn recent_list(&mut self, ui: &mut Ui, p: &egui::Painter, r: Rect, top: f32) {
        let shown = self.recent_shown();
        self.recent_keys(ui, shown.len());
        let loading = self.loading.is_some();
        let head = pos2(r.min.x + 6.0, top + 2.0);
        let title = p.text(
            head,
            Align2::LEFT_TOP,
            t!("recent.title").to_uppercase(),
            mono(10.0),
            MUTED,
        );
        let hint = ["↑", "↓", "⏎"].map(String::from);
        keys::paint_caps(p, pos2(title.max.x + 8.0, top - 2.0), &hint);
        let now = now();
        for (i, rec) in shown.iter().enumerate() {
            let row = Rect::from_min_size(
                pos2(r.min.x, top + HEAD + ROW * i as f32),
                vec2(r.width(), ROW),
            );
            let resp = ui.interact(row, ui.id().with(("recent", i)), Sense::click());
            if resp.hovered() && !loading {
                self.recent_sel = i;
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() && !loading {
                self.open_recent(i);
            }
            if i == self.recent_sel {
                p.rect(
                    row.shrink(1.0),
                    9.0,
                    GREEN.gamma_multiply(0.07),
                    Stroke::new(1.0, GREEN.gamma_multiply(0.22)),
                    egui::StrokeKind::Inside,
                );
            }
            let thumb = Rect::from_min_size(
                pos2(row.min.x + 10.0, row.center().y - 15.5),
                vec2(54.0, 31.0),
            );
            match self.recent_thumb(ui.ctx(), &rec.path) {
                Some(tex) => {
                    // Cover: crop the 4:3 tile to the 54:31 slot.
                    let [w, h] = tex.size();
                    let keep = ((54.0_f32 / 31.0) * h as f32 / w as f32).min(1.0);
                    let uv = Rect::from_min_max(
                        pos2(0.0, (1.0 - keep) / 2.0),
                        pos2(1.0, (1.0 + keep) / 2.0),
                    );
                    p.image(tex.id(), thumb, uv, Color32::WHITE);
                }
                None => {
                    p.rect_filled(thumb, 4.0, CARD);
                    p.text(
                        thumb.center(),
                        Align2::CENTER_CENTER,
                        icon::FILM_STRIP,
                        FontId::proportional(16.0),
                        MUTED,
                    );
                }
            }
            p.rect_stroke(
                thumb,
                4.0,
                Stroke::new(1.0, LINE),
                egui::StrokeKind::Outside,
            );
            let caps = keys::cmd_caps(Cmd::OpenRecent(i));
            let caps_x = row.max.x - 10.0 - keys::caps_width(p, &caps);
            keys::paint_caps(p, pos2(caps_x, row.center().y - 11.0), &caps);
            let when = p.text(
                pos2(caps_x - 14.0, row.center().y),
                Align2::RIGHT_CENTER,
                ago(now, rec.opened),
                FontId::proportional(11.0),
                MUTED,
            );
            let text_x = thumb.max.x + 12.0;
            let clip = Rect::from_min_max(row.min, pos2(when.min.x - 12.0, row.max.y));
            let q = p.with_clip_rect(clip);
            let name = rec.path.file_name().unwrap_or_default().to_string_lossy();
            q.text(
                pos2(text_x, row.center().y - 3.0),
                Align2::LEFT_BOTTOM,
                name,
                FontId::proportional(13.0),
                TEXT,
            );
            let [w, h] = rec.size;
            let info = [rec.codec.clone(), format!("{w}×{h}"), clock(rec.dur)];
            let info: Vec<_> = info.iter().filter(|s| !s.is_empty()).cloned().collect();
            q.text(
                pos2(text_x, row.center().y + 3.0),
                Align2::LEFT_TOP,
                info.join(" · "),
                mono(11.0),
                MUTED,
            );
        }
    }
}
