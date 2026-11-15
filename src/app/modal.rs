use super::*;

#[cfg(test)]
mod tests;

/// What a dialog button does. Which button each is depends on the question (see `modal_key`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Choice {
    Export,
    Discard,
    NewWindow,
    Replace,
    /// Add the files to the open video as a join.
    Join,
}

/// The choice a key answers: ⏎ is the primary button, ⌘⌫ the second.
pub(super) fn modal_key(confirm: &Confirm, enter: bool, cmd_backspace: bool) -> Option<Choice> {
    let (primary, second) = match confirm {
        Confirm::Open(_) => (Choice::Join, Choice::Replace),
        Confirm::Quit | Confirm::CloseVideo => (Choice::Export, Choice::Discard),
    };
    if enter {
        Some(primary)
    } else {
        cmd_backspace.then_some(second)
    }
}

/// The video Join adds to: one still opening (with no join open) rather than the one on screen,
/// as `add_clips` does.
pub(super) fn join_target<'a>(
    loading: Option<&'a std::path::Path>,
    joined: bool,
    video: Option<&'a std::path::Path>,
) -> Option<&'a std::path::Path> {
    loading.filter(|_| !joined).or(video)
}

/// ⏎ answers the primary button only when no button has the keyboard; a focused one takes ⏎ itself.
pub(super) fn enter_is_primary(focused: bool) -> bool {
    !focused
}

impl App {
    /// Put a question up. It takes the keyboard, so close what would sit over it or keep it.
    pub(super) fn ask(&mut self, confirm: Confirm) {
        self.pop = None;
        self.sheet = None;
        self.confirm = Some(confirm);
    }

    /// The center modal: replace-or-new-window for a second file, or unexported edits on close.
    pub(super) fn open_modal(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let (glyph, title, hint, second, first) = match &confirm {
            Confirm::Open(paths) => {
                let video = self.video.as_ref().map(|v| v.path.as_path());
                let name = join_target(self.loading.as_deref(), self.reel.is_some(), video)
                    .and_then(|p| p.file_name())
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                (
                    icon::STACK_PLUS,
                    tn!("join.add_title", paths.len()).to_string(),
                    t!("join.add_hint", name = name).to_string(),
                    (icon::SWAP, t!("open.replace")),
                    (icon::STACK_PLUS, t!("join.append")),
                )
            }
            Confirm::Quit | Confirm::CloseVideo => (
                icon::WARNING,
                t!("unsaved.title").to_string(),
                t!("unsaved.list").to_string(),
                if confirm == Confirm::Quit {
                    (icon::SIGN_OUT, t!("unsaved.quit"))
                } else {
                    (icon::X_CIRCLE, t!("unsaved.close"))
                },
                (icon::EXPORT, t!("export.button")),
            ),
        };
        let is_open = matches!(confirm, Confirm::Open(_));
        let chips = self
            .saved
            .as_ref()
            .map_or(vec![], |s| changed(s, &self.edits(), self.preset));
        let mut choice = None;
        // A modal layer: Tab stays among the dialog's buttons.
        egui::Modal::new("open_modal".into())
            .backdrop_color(Color32::from_black_alpha(150))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(12.0)
                    .inner_margin(20)
                    .shadow(shadow()),
            )
            .show(ctx, |ui| {
                ui.set_width(if is_open { 560.0 } else { 380.0 });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.label(RichText::new(glyph).font(mono(20.0)).color(GREEN));
                    ui.add(
                        egui::Label::new(RichText::new(title).font(bold(13.0)).color(TEXT))
                            .truncate(),
                    );
                });
                ui.add_space(6.0);
                ui.label(RichText::new(hint).font(mono(11.5)).color(MUTED));
                if !is_open {
                    ui.add_space(8.0);
                    chips_row(ui, &chips, self.crop.is_none());
                }
                ui.add_space(14.0);
                // One right-to-left row so all three buttons share a center line.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let w = ui
                        .painter()
                        .layout_no_wrap(first.1.into(), bold(12.5), INK)
                        .size()
                        .x
                        + 56.0;
                    let mac = cfg!(target_os = "macos");
                    let enter = caps(Modifiers::NONE, Key::Enter, mac);
                    let discard = caps(Modifiers::COMMAND, Key::Backspace, mac);
                    let esc = caps(Modifiers::NONE, Key::Escape, mac);
                    let w = w + caps_width(ui.painter(), &enter) + 8.0;
                    if primary_caps(ui, first.0, first.1, vec2(w, 34.0), false, &enter).clicked() {
                        choice = Some(if is_open {
                            Choice::Join
                        } else {
                            Choice::Export
                        });
                    }
                    if accent_caps(ui, second.0, second.1, &discard).clicked() {
                        choice = Some(if is_open {
                            Choice::Replace
                        } else {
                            Choice::Discard
                        });
                    }
                    if is_open
                        && button(ui, icon::APP_WINDOW, t!("open.new_window"), false, GHOST)
                            .clicked()
                    {
                        choice = Some(Choice::NewWindow);
                    }
                    let cancel = ui
                        .painter()
                        .layout_no_wrap(t!("common.cancel").into(), mono(GHOST.text), TEXT)
                        .size()
                        .x
                        + caps_width(ui.painter(), &esc)
                        + 8.0
                        + GHOST.pad * 2.0;
                    ui.add_space((ui.available_width() - cancel).max(0.0));
                    if button_caps(ui, "", t!("common.cancel"), false, GHOST, &esc).clicked() {
                        self.confirm = None;
                    }
                });
            });
        if let Some(choice) = choice {
            self.resolve(confirm, choice, ctx);
        }
    }

    pub(super) fn resolve(&mut self, confirm: Confirm, choice: Choice, ctx: &egui::Context) {
        self.confirm = None;
        match (confirm, choice) {
            (Confirm::Open(paths), Choice::Replace) => self.open_paths(paths),
            (Confirm::Open(paths), Choice::NewWindow) => self.open_in_new_window(paths),
            (Confirm::Open(paths), Choice::Join) => self.add_clips(paths),
            (Confirm::Quit, Choice::Discard) => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (Confirm::CloseVideo, Choice::Discard) => self.close(ctx),
            (Confirm::Quit | Confirm::CloseVideo, Choice::Export) => self.pop = Some(Pop::Export),
            _ => {}
        }
    }
}
