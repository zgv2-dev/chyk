//! Putting the settings to work: what changes right away when one changes, the defaults a new
//! video starts with, and what the settings window shows about updates.

use super::settings_window::UpdateInfo;
use super::*;
use crate::settings::{Sensitivity, Settings};
use crate::update::{Checked, State};

impl App {
    /// Use the settings window's `edited` copy from now on and save it, keeping what another
    /// window saved meanwhile. Returns the error when the file couldn't be written.
    pub(super) fn apply_settings(
        &mut self,
        edited: Settings,
        ctx: &egui::Context,
    ) -> Option<String> {
        let new = Settings::merge(&self.settings, &edited, &Settings::load());
        self.adopt_settings(new, ctx);
        self.settings.save().err().map(|e| e.to_string())
    }

    /// Use `new` from now on, without saving: also for what another Chyk window saved.
    pub(super) fn adopt_settings(&mut self, new: Settings, ctx: &egui::Context) {
        let old = std::mem::replace(&mut self.settings, new);
        let s = &self.settings;
        if old.lang != s.lang {
            i18n::set(s.lang.lang());
            // Ukrainian uses its own font; every window redraws in the new language.
            fonts(ctx);
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            ctx.request_repaint_of(super::log_window::viewport());
        }
        if old.hwdec != s.hwdec {
            let _ = self
                .player
                .mpv
                .set_property("hwdec", crate::player::hwdec_option(s.hwdec));
        }
    }

    /// Open the settings window, or bring it to the front when it's already open, here or in
    /// another Chyk window.
    pub(super) fn show_settings(&mut self, ctx: &egui::Context) {
        if self.settings_open {
            ctx.send_viewport_cmd_to(
                super::settings_window::viewport(),
                egui::ViewportCommand::Focus,
            );
        } else if self.settings_share.ask() {
            return;
        } else {
            self.settings_share.claim();
        }
        self.settings_open = true;
    }

    /// Keep the folder of an export for "Save as… starts in: Last used folder".
    pub(super) fn remember_folder(&mut self, dst: &std::path::Path) {
        let dir = dst.parent().map(PathBuf::from);
        if dir.is_none() || dir == self.settings.last_folder {
            return;
        }
        self.settings.last_folder.clone_from(&dir);
        // An open settings window edits its own copy; keep it from writing back the old folder.
        super::settings_window::lock(&self.settings_view)
            .settings
            .last_folder
            .clone_from(&dir);
        // Onto the file as it is now: another Chyk window may have saved changes since.
        let mut disk = Settings::load();
        disk.last_folder = dir;
        let _ = disk.save();
    }

    /// A newly loaded video starts from the export, Strip and preview defaults; an open one
    /// keeps what it has when they change.
    pub(super) fn apply_defaults(&mut self, ext: &std::ffi::OsStr) {
        let s = &self.settings;
        let same = ffmpeg::FORMATS
            .iter()
            .position(|f| ext.eq_ignore_ascii_case(f));
        self.format = s.format.or(same).unwrap_or(0);
        self.quality = s.quality;
        self.size = s.size;
        self.precise = s.precise;
        self.mute = s.mute;
        self.strip.pad = s.pad;
        self.strip.sensitivity = s.sensitivity;
        self.set_preview_mute(s.mute_preview);
    }

    pub(super) fn update_info(&self) -> UpdateInfo {
        let u = &self.update;
        if !u.supported() {
            return UpdateInfo::Unsupported;
        }
        match (&u.state, u.checked) {
            (
                State::Available { tag, .. }
                | State::Downloading { tag, .. }
                | State::Ready { tag, .. }
                | State::Updating { tag, .. },
                _,
            ) => UpdateInfo::Available(tag.clone()),
            (_, Checked::NotYet) => UpdateInfo::NotYet,
            (_, Checked::Running) => UpdateInfo::Checking,
            (_, Checked::Offline) => UpdateInfo::Offline,
            (_, Checked::Done) => UpdateInfo::Latest,
        }
    }

    /// libmpv's version, e.g. `0.40.0` from mpv's `mpv v0.40.0`.
    pub(super) fn mpv_version(&self) -> Option<String> {
        let v = self.player.get::<String>("mpv-version")?;
        Some(
            v.trim_start_matches("mpv ")
                .trim_start_matches('v')
                .to_string(),
        )
    }
}

/// Strip's thresholds for a sensitivity: how far a pixel's gray value must move to count as
/// changed, and how many changed pixels make a frame active.
pub(super) fn strip_thresholds(s: Sensitivity) -> strip::Thresholds {
    let (diff, min_pixels) = match s {
        Sensitivity::Low => (16, 12),
        Sensitivity::Normal => (10, 6),
        Sensitivity::High => (6, 3),
    };
    strip::Thresholds { diff, min_pixels }
}

/// `ffmpeg -version`'s number, read in the background; it wakes the settings window when known.
pub(super) fn ffmpeg_version(view: super::settings_window::SharedSettings, ctx: egui::Context) {
    std::thread::spawn(move || {
        let mut cmd = ffmpeg::command("ffmpeg");
        cmd.arg("-version");
        // Logged like the other probes.
        let entry = ffmpeg::log::begin(ffmpeg::log::Kind::Probe);
        let out = ffmpeg::log::output(entry, &mut cmd).ok();
        let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
        // "ffmpeg version 7.1.1 Copyright (c) …" or "ffmpeg version n7.1-…".
        let ver = text
            .as_deref()
            .and_then(|t| t.split_whitespace().nth(2))
            .map(str::to_string);
        super::settings_window::lock(&view).ffmpeg = Some(ver.unwrap_or_else(|| "—".into()));
        ctx.request_repaint_of(super::settings_window::viewport());
    });
}
