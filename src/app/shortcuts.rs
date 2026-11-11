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
        if self.sheet.is_some() {
            let closed = ctx.input_mut(|i| {
                let n = i.events.len();
                i.events.retain(|e| {
                    !matches!(
                        e,
                        egui::Event::Key {
                            key: Key::Escape | Key::Questionmark,
                            pressed: true,
                            ..
                        } | egui::Event::Key {
                            key: Key::Slash,
                            pressed: true,
                            modifiers: Modifiers { shift: true, .. },
                            ..
                        }
                    )
                });
                n != i.events.len()
            });
            if closed {
                self.sheet = None;
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
        self.export_keys(ctx);
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
