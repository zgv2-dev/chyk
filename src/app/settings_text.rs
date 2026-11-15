//! Each settings row's title and hint, and what Reset page puts back.

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
