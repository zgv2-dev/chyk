use super::*;

impl App {
    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            if videos::is_video(&p) {
                self.request_open(p);
            } else {
                let name = p.file_name().unwrap_or(p.as_os_str()).to_string_lossy();
                self.status = Some((t!("status.not_video", name = name), true));
            }
        }
        // A question in the modal takes the keyboard until it is answered.
        if self.confirm.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.confirm = None;
            }
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        // A focused control in an open popover keeps Space and the arrows.
        let focused = self.pop.is_some() && ctx.memory(|m| m.focused().is_some());
        for cmd in pressed(ctx, typing, focused) {
            if self.enabled(cmd) {
                self.run(cmd, ctx);
            }
        }
        if self.video.is_none() || typing {
            return;
        }
        let key = |k| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        if key(Key::Escape) {
            if egui::Popup::is_any_open(ctx) {
                // A list open inside the popover closes first.
                egui::Popup::close_all(ctx);
            } else if self.pop.is_some() {
                self.pop = None;
            } else if self.strip.edit {
                self.strip.edit = false;
                self.strip.selected = None;
            } else {
                self.close_panel(ctx);
            }
        }
    }
}
