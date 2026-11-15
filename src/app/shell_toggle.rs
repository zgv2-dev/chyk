//! Start screen footer on Windows: a checkbox for "Edit with Chyk" in Explorer's right-click menu.

use super::*;
use crate::shell_menu::{self, Win11};

/// Height the footer takes at the bottom of the start screen (none where there is no menu).
pub(super) const FOOT_H: f32 = if shell_menu::SUPPORTED { 28.0 } else { 0.0 };

impl App {
    pub(super) fn shell_toggle(&mut self, ui: &mut Ui, full: Rect) {
        if !shell_menu::SUPPORTED {
            return;
        }
        self.poll_shell_menu();
        let busy = self.shell_menu.job.is_some();
        let on = self.shell_menu.on;
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
            _ if busy => MUTED,
            (true, _) => TEXT,
            (false, true) => GREEN_DIM,
            (false, false) => MUTED,
        };
        let x = center.x - w / 2.0;
        p.galley(pos2(x, center.y - g.size().y / 2.0), g.clone(), color);
        let lx = x + g.size().x + 7.0;
        p.galley(pos2(lx, center.y - l.size().y / 2.0), l, color);
        if resp.hovered() && !busy {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.has_focus() {
            focus_ring(ui.painter(), r, 4.0);
        }
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label)
        });
        if resp.clicked() {
            self.toggle_shell_menu(ui.ctx());
        }
    }

    fn toggle_shell_menu(&mut self, ctx: &egui::Context) {
        if self.shell_menu.job.is_some() {
            return;
        }
        // Another Chyk window may have changed it since this one started: ask the registry.
        let on = !shell_menu::is_on();
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        // Registering the Windows 11 package runs PowerShell for a second or two.
        std::thread::spawn(move || {
            let _ = tx.send(shell_menu::set(on));
            crate::wake(&ctx);
        });
        self.shell_menu.job = Some((on, rx));
        self.status = Some((t!("shell.working").to_string(), false));
    }

    /// The toggle's result, once its thread is done.
    fn poll_shell_menu(&mut self) {
        let Some((on, rx)) = &self.shell_menu.job else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        let on = *on;
        self.shell_menu.job = None;
        self.status = Some(match result {
            Err(e) => (t!("shell.failed", error = e), true),
            Ok(win11) => {
                self.shell_menu.on = on;
                match win11 {
                    _ if !on => (t!("shell.removed").to_string(), false),
                    Win11::NeedsDevMode => (t!("shell.needs_dev_mode").to_string(), false),
                    Win11::Failed(e) => (t!("shell.win11_failed", error = e), true),
                    _ => (t!("shell.added").to_string(), false),
                }
            }
        });
    }
}

/// The toggle's state: on, and the registration running in the background.
pub(super) struct ShellMenu {
    pub(super) on: bool,
    job: Option<(bool, Receiver<Result<Win11, String>>)>,
}

impl ShellMenu {
    pub(super) fn new() -> Self {
        Self {
            on: shell_menu::is_on(),
            job: None,
        }
    }
}
