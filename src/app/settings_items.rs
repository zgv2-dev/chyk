//! The settings window's pages and their groups of rows.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Page {
    #[default]
    General,
    Export,
    Editing,
    About,
}

pub(super) const PAGES: [Page; 4] = [Page::General, Page::Export, Page::Editing, Page::About];

/// One row: what it does on the left, its control on the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Item {
    Lang,
    UiSize,
    Another,
    ShellMenu,
    CheckUpdates,
    Version,
    Format,
    Quality,
    Size,
    Precise,
    Mute,
    SaveTo,
    Suffix,
    Sound,
    Skip,
    StopAtOut,
    Hwdec,
    MutePreview,
    Pad,
    Sensitivity,
    Ffmpeg,
    Mpv,
    Website,
    Source,
    Report,
    License,
}

/// A titled card of rows, with an optional note under the title.
pub(super) struct Group {
    pub(super) title: &'static str,
    pub(super) note: Option<&'static str>,
    pub(super) items: &'static [Item],
}

const fn group(title: &'static str, items: &'static [Item]) -> Group {
    Group {
        title,
        note: None,
        items,
    }
}

const GENERAL: &[Group] = &[
    group("settings.interface", &[Item::Lang, Item::UiSize]),
    group("settings.opening", &[Item::Another, Item::ShellMenu]),
    group("settings.updates", &[Item::CheckUpdates, Item::Version]),
];
const EXPORT: &[Group] = &[
    Group {
        title: "settings.defaults",
        note: Some("settings.defaults_hint"),
        items: &[
            Item::Format,
            Item::Quality,
            Item::Size,
            Item::Precise,
            Item::Mute,
        ],
    },
    group(
        "settings.saving",
        &[Item::SaveTo, Item::Suffix, Item::Sound],
    ),
];
const EDITING: &[Group] = &[
    group(
        "settings.playback",
        &[Item::Skip, Item::StopAtOut, Item::MutePreview, Item::Hwdec],
    ),
    Group {
        title: "cmd.strip",
        note: Some("settings.strip_hint"),
        items: &[Item::Pad, Item::Sensitivity],
    },
];
const ABOUT: &[Group] = &[
    group("settings.components", &[Item::Ffmpeg, Item::Mpv]),
    group(
        "settings.links",
        &[Item::Website, Item::Source, Item::Report, Item::License],
    ),
];

impl Page {
    pub(super) fn text(self) -> (&'static str, &'static str) {
        match self {
            Page::General => (icon::GEAR_SIX, "settings.general"),
            Page::Export => (icon::EXPORT, "settings.export"),
            Page::Editing => (icon::SCISSORS, "settings.editing"),
            Page::About => (icon::INFO, "settings.about"),
        }
    }

    pub(super) fn groups(self) -> &'static [Group] {
        match self {
            Page::General => GENERAL,
            Page::Export => EXPORT,
            Page::Editing => EDITING,
            Page::About => ABOUT,
        }
    }

    /// Pages with something to reset (About has no settings).
    pub(super) fn resettable(self) -> bool {
        self != Page::About
    }
}

/// Items matching `query` in their title, hint or search words, in English or the interface
/// language. The component rows also match the names they show.
pub(super) fn matches(item: Item, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    let name = match item {
        Item::Ffmpeg => "ffmpeg",
        Item::Mpv => "libmpv",
        _ => "",
    };
    let (_, title, hint) = item.text();
    name.contains(&q)
        || [title, hint, item.tags()].into_iter().flatten().any(|k| {
            i18n::tr(k).to_lowercase().contains(&q) || i18n::en(k).to_lowercase().contains(&q)
        })
}

impl Item {
    /// Words a search should find this row by besides its title and hint: "mute" finds Remove
    /// audio, "sound" the done sound.
    fn tags(self) -> Option<&'static str> {
        use Item::*;
        Some(match self {
            Lang => "settings.tags_lang",
            UiSize => "settings.tags_ui_size",
            Another => "settings.tags_another",
            ShellMenu => "settings.tags_shell_menu",
            CheckUpdates => "settings.tags_check_updates",
            Version => "settings.tags_version",
            Format => "settings.tags_format",
            Quality => "settings.tags_quality",
            Size => "settings.tags_size",
            Precise => "settings.tags_precise",
            Mute => "settings.tags_mute",
            SaveTo => "settings.tags_save_to",
            Suffix => "settings.tags_suffix",
            Sound => "settings.tags_sound",
            Skip => "settings.tags_skip",
            StopAtOut => "settings.tags_stop_at_out",
            Hwdec => "settings.tags_hwdec",
            MutePreview => "settings.tags_mute_preview",
            Pad => "settings.tags_pad",
            Sensitivity => "settings.tags_sensitivity",
            Ffmpeg | Mpv | Website | Source | Report | License => return None,
        })
    }
}
