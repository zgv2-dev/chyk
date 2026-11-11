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
        if let Some(confirm) = self.confirm.clone() {
            let focused = ctx.memory(|m| m.focused().is_some());
            let (esc, enter, back) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::Escape),
                    // A focused button gets ⏎ itself.
                    enter_is_primary(focused) && i.consume_key(Modifiers::NONE, Key::Enter),
                    i.consume_key(Modifiers::COMMAND, Key::Backspace),
                )
            });
            if let Some(choice) = modal_key(&confirm, enter, back) {
                self.resolve(confirm, choice, ctx);
            } else if esc {
                self.confirm = None;
            }
            return;
        }
        // The sheet's filter takes the keyboard; ? and Esc close it, nothing behind it reacts.
        // A held ? repeats: those presses (and the `?` they type) are dropped, they don't close it.
        if self.sheet.is_some() {
            let (mut closed, mut held) = (false, false);
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    let egui::Event::Key {
                        key,
                        pressed: true,
                        repeat,
                        modifiers: m,
                        ..
                    } = e
                    else {
                        return true;
                    };
                    let hit = matches!(key, Key::Escape | Key::Questionmark)
                        || (*key == Key::Slash && m.shift);
                    held |= hit && *repeat;
                    closed |= hit && !*repeat;
                    !hit
                });
                if held {
                    i.events
                        .retain(|e| !matches!(e, egui::Event::Text(s) if s == "?"));
                }
            });
            if closed {
                self.sheet = None;
            }
            return;
        }
        // Any focused widget keeps the single keys (a popover control keeps Space and the arrows).
        let typing = ctx.egui_wants_keyboard_input();
        for cmd in pressed(ctx, typing, ctx.text_edit_focused()) {
            if self.enabled(cmd) {
                self.run(cmd, ctx);
            }
        }
        // Before the typing gate: a focused popover control counts as typing, and P/M still work.
        self.export_keys(ctx);
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
