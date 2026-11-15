//! The settings window's pages, their groups and rows, and each row's icon and text.

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
        &[Item::Skip, Item::StopAtOut, Item::Hwdec],
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
