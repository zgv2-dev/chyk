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
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.open_dialog();
        }
        if self.video.is_none() {
            return;
        }
        // Same as the Export button: open the settings; pressed again, save.
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            if self.pop == Some(Pop::Export) {
                self.pop = None;
                self.export(ctx);
            } else {
                self.pop = Some(Pop::Export);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::I)) {
            self.close_panel(ctx);
            self.toggle_pop(Pop::Info);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::W)) {
            self.request_close(ctx);
            return;
        }
        // Typing in a time field: leave these keys to the text edit.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        let key = |k| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        if key(Key::Space) {
            self.toggle_play();
        }
        if key(Key::ArrowLeft) {
            self.player.cmd("frame-back-step", &[]);
        }
        if key(Key::ArrowRight) {
            self.player.cmd("frame-step", &[]);
        }
        if key(Key::I) {
            self.set_in(self.pos());
        }
        if key(Key::O) {
            self.set_out(self.pos());
        }
        if key(Key::C) {
            self.toggle_crop(ctx);
        }
        if key(Key::Z) && self.panel == Some(Panel::Strip) {
            self.toggle_zone_edit();
        }
        if key(Key::Plus)
            || key(Key::Equals)
            || ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Plus))
        {
            self.zoom_video(1.25, None);
        }
        if key(Key::Minus) {
            self.zoom_video(0.8, None);
        }
        if key(Key::Num0) {
            self.reset_video_zoom();
        }
        if key(Key::CloseBracket) {
            self.zoom_timeline(1.5, dur);
        }
        if key(Key::OpenBracket) {
            self.zoom_timeline(1.0 / 1.5, dur);
        }
        if key(Key::Comma) || ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Comma)) {
            self.nudge_speed(false);
        }
        if key(Key::Period) || ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Period)) {
            self.nudge_speed(true);
        }
        if self.strip.edit && (key(Key::Delete) || key(Key::Backspace)) {
            self.remove_selected_zone();
        }
        if key(Key::Escape) {
            if self.pop.is_some() {
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
