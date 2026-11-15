//! Putting the settings to work: what changes right away when one changes, the defaults a new
//! video starts with, and what the settings window shows about updates.

use super::settings_window::UpdateInfo;
use super::*;
use crate::settings::{Sensitivity, Settings};
use crate::update::{Checked, State};

impl App {
    /// Use `new` from now on and save it. Returns the error when the file couldn't be written.
    pub(super) fn apply_settings(&mut self, new: Settings, ctx: &egui::Context) -> Option<String> {
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
        if old.sensitivity != s.sensitivity {
            // Scans made with the old threshold no longer describe what Strip would cut.
            self.scans.clear();
            self.cancel_scan();
            self.strip.scan = None;
            // During an export the scan waits; it starts when the export ends (see poll).
            if self.strip.enabled && self.video.is_some() && self.job.is_none() {
                self.ensure_scan(ctx);
            }
        }
        self.settings.save().err().map(|e| e.to_string())
    }

    /// Open the settings window, or bring it to the front when it's already open.
    pub(super) fn show_settings(&mut self, ctx: &egui::Context) {
        if self.settings_open {
            ctx.send_viewport_cmd_to(
                super::settings_window::viewport(),
                egui::ViewportCommand::Focus,
            );
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
            .last_folder = dir;
        let _ = self.settings.save();
    }

    /// A newly loaded video starts from the export and Strip defaults.
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
