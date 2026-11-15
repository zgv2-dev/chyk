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
    group("settings.interface", &[Item::Lang]),
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
    group("cmd.strip", &[Item::Pad, Item::Sensitivity]),
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
