//! The settings in their own OS window, laid out like Windows Terminal's: categories on the
//! left, the chosen page on the right. There is no Save button: changes apply at once.

use super::settings_items::Page;
use super::*;
use crate::settings::Settings;
use std::sync::{Arc, Mutex, PoisonError};

/// The window's title; hidden on macOS, where it also finds the window to place its buttons.
const TITLE: &str = "Chyk — Settings";

pub(super) fn viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("chyk-settings")
}

/// What the About and General pages say about updates.
#[derive(Clone, Default, PartialEq)]
pub(super) enum UpdateInfo {
    /// A build that doesn't update itself.
    #[default]
    Unsupported,
    /// Not checked this session (the launch check is off).
    NotYet,
    Checking,
    Offline,
    Latest,
    Available(String),
}

/// What the settings window shows. It draws on its own passes (a deferred viewport), so the main
/// window shares this with it.
#[derive(Default)]
pub(super) struct SettingsView {
    /// The window's copy, edited in place; `changed` asks the main window to apply and save it.
    pub(super) settings: Settings,
    pub(super) changed: bool,
    pub(super) page: Page,
    pub(super) query: String,
    /// The window asked to close; the main window turns `settings_open` off.
    closed: bool,
    /// Set up on the window's first pass.
    opened: bool,
    pub(super) buttons: chrome::Layout,
    pub(super) update: UpdateInfo,
    /// Check now was clicked; `asked` stays on until the answer is in.
    pub(super) check_now: bool,
    pub(super) asked: bool,
    /// The answer to Check now was "up to date": a trophy in the button's place until the page
    /// changes or the window reopens, and confetti from `cheer` (start time, origin) for a moment.
    pub(super) won: bool,
    pub(super) cheer: Option<(f64, Pos2)>,
    /// ffmpeg's and libmpv's versions for About, once known.
    pub(super) ffmpeg: Option<String>,
    pub(super) mpv: Option<String>,
    /// Writing the settings file failed, with the error.
    pub(super) error: Option<String>,
    /// The right-click menu entry; `None` where there is none.
    pub(super) shell: Option<super::shell_toggle::ShellRow>,
    /// Its switch was clicked.
    pub(super) shell_toggle: bool,
}

pub(super) type SharedSettings = Arc<Mutex<SettingsView>>;

pub(super) fn lock(view: &SharedSettings) -> std::sync::MutexGuard<'_, SettingsView> {
    view.lock().unwrap_or_else(PoisonError::into_inner)
}

impl App {
    /// Take what the settings window asked for. Runs in `logic`, so edits apply and save even
    /// while the main window is minimized (eframe skips `ui` then).
    pub(super) fn settings_sync(&mut self, ctx: &egui::Context) {
        // A clone of the handle, so `self` stays free for the calls below.
        let shared = self.settings_view.clone();
        let mut view = lock(&shared);
        if std::mem::take(&mut view.closed) {
            self.settings_open = false;
            self.settings_share.release();
        }
        // Saved by another Chyk window: take it on here, and in the open window's copy.
        if self.settings_share.take_changed() {
            let disk = Settings::load();
            if disk != self.settings && !view.changed {
                view.settings = disk.clone();
                drop(view);
                self.adopt_settings(disk, ctx);
                ctx.request_repaint_of(viewport());
                view = lock(&shared);
            }
        }
        // A change made on the window's last pass is applied even if it closed on that pass.
        if std::mem::take(&mut view.changed) {
            let (new, failed) = (view.settings.clone(), view.error.is_some());
            drop(view);
            let error = self.apply_settings(new, failed, ctx);
            view = lock(&shared);
            view.error = error;
            // What another window saved meanwhile shows here too.
            view.settings.clone_from(&self.settings);
        }
        if self.settings_share.take_raise() {
            drop(view);
            self.show_settings(ctx);
            view = lock(&shared);
        }
        if !self.settings_open {
            return;
        }
        if std::mem::take(&mut view.check_now) {
            self.update.check_now();
        }
        if std::mem::take(&mut view.shell_toggle) {
            self.toggle_shell_menu(ctx);
        }
        self.poll_shell_menu();
        let (info, shell) = (self.update_info(), self.shell_row());
        if view.update != info || view.shell != shell {
            (view.update, view.shell) = (info, shell);
            ctx.request_repaint_of(viewport());
        }
    }

    /// The main window's `ui` side: keep the settings window alive (a deferred viewport has to be
    /// shown from `ui`).
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        let mut view = lock(&self.settings_view);
        if !self.settings_open {
            view.opened = false;
            return;
        }
        if !view.opened {
            view.settings = self.settings.clone();
            view.buttons = self.buttons.clone();
            view.query.clear();
            (view.won, view.cheer) = (false, None);
            view.mpv = self.mpv_version();
            // Shown as … until the probe answers; set now so later passes don't start another.
            if view.ffmpeg.is_none() {
                view.ffmpeg = Some("…".into());
                super::settings_apply::ffmpeg_version(self.settings_view.clone(), ctx.clone());
            }
        }
        drop(view);
        let builder = egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([900.0, 640.0])
            .with_min_inner_size([640.0, 420.0])
            // Like the main window: on macOS the traffic lights sit inside the header.
            .with_fullsize_content_view(cfg!(target_os = "macos"))
            .with_titlebar_shown(!cfg!(target_os = "macos"))
            .with_title_shown(!cfg!(target_os = "macos"))
            // Elsewhere Chyk draws the window buttons and resize edges, as in the main window.
            .with_decorations(!chrome::CUSTOM);
        let shared = self.settings_view.clone();
        ctx.show_viewport_deferred(viewport(), builder, move |ui, _| {
            draw(ui, &mut lock(&shared));
        });
    }
}

/// One pass of the settings window.
fn draw(ui: &mut Ui, view: &mut SettingsView) {
    #[cfg(windows)]
    chrome::round_corners();
    follow_ui_size(ui.ctx(), vec2(640.0, 420.0));
    #[cfg(target_os = "macos")]
    chrome::place_traffic_lights_titled(TITLE, f64::from(super::settings_nav::HEAD_H));
    if !view.opened {
        view.opened = true;
        // Dark title bar like the main window's, whatever the system theme.
        let dark = egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark);
        ui.ctx().send_viewport_cmd(dark);
        ui.memory_mut(|m| m.request_focus(super::settings_nav::search_id()));
    }
    let close = ui.input_mut(|i| {
        i.viewport().close_requested()
            | i.consume_key(Modifiers::COMMAND, Key::W)
            | i.consume_key(Modifiers::COMMAND, Key::Comma)
    });
    // Esc clears the search first, then closes.
    let esc = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    if esc && !view.query.is_empty() {
        view.query.clear();
    } else if esc || close {
        view.closed = true;
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    let before = view.settings.clone();
    super::settings_nav::head(ui, view);
    super::settings_nav::sidebar(ui, view);
    super::settings_page::page(ui, view);
    if view.settings != before {
        view.changed = true;
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    chrome::resize_edges(ui.ctx());
}
