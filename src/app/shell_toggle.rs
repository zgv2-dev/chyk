//! The "Edit with Chyk" right-click menu entry, switched from the settings window (General →
//! Opening videos). Registering runs in the background; the row shows the result.

use super::*;
use crate::shell_menu::{self, Win11};

/// What the settings row shows: whether the entry is there, a change running, the last result.
#[derive(Clone, Default, PartialEq)]
pub(super) struct ShellRow {
    pub(super) on: bool,
    pub(super) busy: bool,
    /// The last change's message, and whether it is an error.
    pub(super) note: Option<(String, bool)>,
}

impl App {
    pub(super) fn toggle_shell_menu(&mut self, ctx: &egui::Context) {
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
        self.shell_menu.note = None;
    }

    /// The toggle's result, once its thread is done.
    pub(super) fn poll_shell_menu(&mut self) {
        let Some((on, rx)) = &self.shell_menu.job else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        let on = *on;
        self.shell_menu.job = None;
        self.shell_menu.note = Some(match result {
            Err(e) => (t!("shell.failed", error = e), true),
            Ok(win11) => {
                self.shell_menu.on = on;
                match win11 {
                    Win11::Failed(e) if !on => (t!("shell.failed", error = e), true),
                    _ if !on => (t!("shell.removed").to_string(), false),
                    Win11::NeedsDevMode => (t!("shell.needs_dev_mode").to_string(), false),
                    Win11::Failed(e) => (t!("shell.win11_failed", error = e), true),
                    _ => (t!("shell.added").to_string(), false),
                }
            }
        });
    }

    /// The row's state; `None` where there is no menu to add to.
    pub(super) fn shell_row(&self) -> Option<ShellRow> {
        shell_menu::SUPPORTED.then(|| ShellRow {
            on: self.shell_menu.on,
            busy: self.shell_menu.job.is_some(),
            note: self.shell_menu.note.clone(),
        })
    }
}

/// The toggle's state: on, the registration running in the background, and its result.
pub(super) struct ShellMenu {
    pub(super) on: bool,
    job: Option<(bool, Receiver<Result<Win11, String>>)>,
    note: Option<(String, bool)>,
}

impl ShellMenu {
    pub(super) fn new() -> Self {
        Self {
            on: shell_menu::is_on(),
            job: None,
            note: None,
        }
    }
}
