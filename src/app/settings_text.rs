//! Each settings row's icon, title and hint, and what Reset page puts back.

use super::settings_items::Item;
use super::settings_window::{SettingsView, UpdateInfo};
use super::*;
use crate::settings::Settings;

/// The row's title and hint, filled in.
pub(super) fn label(item: Item, view: &SettingsView) -> (String, Option<String>) {
    let s = &view.settings;
    let (_, title, hint) = item.text();
    let mut hint = hint.map(|k| i18n::tr(k).to_string());
    let title = match item {
        Item::Ffmpeg => "ffmpeg".into(),
        Item::Mpv => "libmpv".into(),
        Item::Version => t!("settings.version", version = env!("CARGO_PKG_VERSION")),
        _ => title.map_or("", i18n::tr).into(),
    };
    match item {
        Item::Version => hint = Some(update_text(&view.update)),
        Item::ShellMenu => match &view.shell {
            Some(r) if r.busy => hint = Some(t!("shell.working").into()),
            Some(super::shell_toggle::ShellRow {
                note: Some((n, _)), ..
            }) => hint = Some(n.clone()),
            _ => {}
        },
        Item::Quality => hint = Some(i18n::tr(QUALITY_HINTS[s.quality]).into()),
        Item::Suffix => {
            let name = format!("video{}.mp4", s.suffix);
            hint = Some(t!("settings.suffix_hint", example = name));
        }
        _ => {}
    }
    (title, hint)
}

const QUALITY_HINTS: [&str; 4] = [
    "quality.high_hint",
    "quality.medium_hint",
    "quality.low_hint",
    "quality.compress_hint",
];

fn update_text(info: &UpdateInfo) -> String {
    match info {
        UpdateInfo::Unsupported => t!("settings.no_updates").into(),
        UpdateInfo::NotYet => t!("settings.not_checked").into(),
        UpdateInfo::Checking => t!("settings.checking").into(),
        UpdateInfo::Offline => t!("settings.offline").into(),
        UpdateInfo::Latest => t!("settings.up_to_date").into(),
        UpdateInfo::Available(tag) => t!("settings.available", version = tag),
    }
}

/// Reset `item` to its value in `d`.
pub(super) fn reset(item: Item, s: &mut Settings, d: &Settings) {
    match item {
        Item::Lang => s.lang = d.lang,
        Item::Another => s.another = d.another,
        Item::CheckUpdates => s.check_updates = d.check_updates,
        Item::Format => s.format = d.format,
        Item::Quality => s.quality = d.quality,
        Item::Size => s.size = d.size,
        Item::Precise => s.precise = d.precise,
        Item::Mute => s.mute = d.mute,
        Item::SaveTo => s.save_to = d.save_to,
        Item::Suffix => s.suffix.clone_from(&d.suffix),
        Item::Sound => s.sound = d.sound,
        Item::Skip => s.skip = d.skip,
        Item::StopAtOut => s.stop_at_out = d.stop_at_out,
        Item::Hwdec => s.hwdec = d.hwdec,
        Item::MutePreview => s.mute_preview = d.mute_preview,
        Item::Pad => s.pad = d.pad,
        Item::Sensitivity => s.sensitivity = d.sensitivity,
        _ => {}
    }
}

/// `0.5`, `1.25`, `2`: seconds without trailing zeros.
pub(super) fn fmt_secs(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl Item {
    /// Icon, title key and hint key, for drawing and search. `None` title: not translated.
    pub(super) fn text(self) -> (&'static str, Option<&'static str>, Option<&'static str>) {
        use Item::*;
        let (glyph, title, hint) = match self {
            Lang => (
                icon::TRANSLATE,
                "settings.language",
                Some("settings.language_hint"),
            ),
            Another => (
                icon::COPY_SIMPLE,
                "settings.another",
                Some("settings.another_hint"),
            ),
            ShellMenu => (
                icon::CURSOR_CLICK,
                "settings.shell_menu",
                Some("settings.shell_menu_hint"),
            ),
            CheckUpdates => (
                icon::ARROW_CIRCLE_UP,
                "settings.check_updates",
                Some("settings.check_updates_hint"),
            ),
            Version => (icon::SEAL_CHECK, "settings.version", None),
            Format => (icon::FILE_VIDEO, "export.format", None),
            Quality => (icon::SLIDERS_HORIZONTAL, "export.quality", None),
            Size => (icon::FRAME_CORNERS, "export.resolution", None),
            Precise => (
                icon::CROSSHAIR,
                "export.precise",
                Some("export.precise_hint"),
            ),
            Mute => (icon::SPEAKER_SLASH, "export.mute", Some("export.mute_hint")),
            SaveTo => (icon::FOLDER_OPEN, "settings.save_to", None),
            Suffix => (
                icon::TEXTBOX,
                "settings.suffix",
                Some("settings.suffix_hint"),
            ),
            Sound => (
                icon::BELL_SIMPLE,
                "settings.sound",
                Some("settings.sound_hint"),
            ),
            Skip => (
                icon::CLOCK_CLOCKWISE,
                "settings.skip",
                Some("settings.skip_hint"),
            ),
            StopAtOut => (
                icon::SIGN_OUT,
                "settings.stop_out",
                Some("settings.stop_out_hint"),
            ),
            Hwdec => (icon::CPU, "settings.hwdec", Some("settings.hwdec_hint")),
            MutePreview => (
                icon::SPEAKER_SIMPLE_SLASH,
                "settings.mute_preview",
                Some("settings.mute_preview_hint"),
            ),
            Pad => (
                icon::BRACKETS_CURLY,
                "settings.pad",
                Some("settings.pad_hint"),
            ),
            Sensitivity => (
                icon::WAVEFORM,
                "settings.sensitivity",
                Some("settings.sensitivity_hint"),
            ),
            Ffmpeg => return (icon::FILM_REEL, None, Some("settings.ffmpeg_hint")),
            Mpv => return (icon::PLAY_CIRCLE, None, Some("settings.mpv_hint")),
            Website => (icon::GLOBE, "settings.website", None),
            Source => (icon::GITHUB_LOGO, "settings.source", None),
            Report => (icon::BUG, "settings.report", None),
            License => (icon::SCALES, "settings.license", None),
        };
        (glyph, Some(title), hint)
    }
}

/// Items matching `query` in their title or hint, in English or the interface language. The
/// component rows also match the names they show.
pub(super) fn matches(item: Item, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    let name = match item {
        Item::Ffmpeg => "ffmpeg",
        Item::Mpv => "libmpv",
        _ => "",
    };
    let (_, title, hint) = item.text();
    name.contains(&q)
        || [title, hint].into_iter().flatten().any(|k| {
            i18n::tr(k).to_lowercase().contains(&q) || i18n::en(k).to_lowercase().contains(&q)
        })
}
