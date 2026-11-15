//! Start screen footer on Windows: a checkbox for "Edit with Chyk" in Explorer's right-click menu.

use super::*;
use crate::shell_menu;

/// Height the footer takes at the bottom of the start screen (none where there is no menu).
pub(super) const FOOT_H: f32 = if shell_menu::SUPPORTED { 28.0 } else { 0.0 };

impl App {
    pub(super) fn shell_toggle(&mut self, ui: &mut Ui, full: Rect) {
        if !shell_menu::SUPPORTED {
            return;
        }
        let on = self.shell_menu;
        let label = t!("shell.toggle");
        let p = ui.painter();
        let glyph = if on { icon::CHECK_SQUARE } else { icon::SQUARE };
        let g = p.layout_no_wrap(glyph.into(), mono(13.0), Color32::PLACEHOLDER);
        let l = p.layout_no_wrap(label.into(), mono(11.5), Color32::PLACEHOLDER);
        let w = g.size().x + 7.0 + l.size().x;
        let center = pos2(full.center().x, full.max.y - FOOT_H / 2.0);
        let r = Rect::from_center_size(center, vec2(w + 12.0, 22.0));
        let resp = ui.interact(r, ui.id().with("shell_menu"), Sense::click());
        let color = match (resp.hovered(), on) {
            (true, _) => TEXT,
            (false, true) => GREEN_DIM,
            (false, false) => MUTED,
        };
        let x = center.x - w / 2.0;
        p.galley(pos2(x, center.y - g.size().y / 2.0), g.clone(), color);
        let lx = x + g.size().x + 7.0;
        p.galley(pos2(lx, center.y - l.size().y / 2.0), l, color);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.has_focus() {
            focus_ring(ui.painter(), r, 4.0);
        }
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label)
        });
        if resp.clicked() {
            self.toggle_shell_menu();
        }
    }

    fn toggle_shell_menu(&mut self) {
        let on = !self.shell_menu;
        self.status = Some(match shell_menu::set(on) {
            Ok(()) => {
                self.shell_menu = on;
                let msg = if on {
                    t!("shell.added")
                } else {
                    t!("shell.removed")
                };
                (msg.to_string(), false)
            }
            Err(e) => (t!("shell.failed", error = e), true),
        });
    }
}
