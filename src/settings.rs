//! What the user set in the settings window, kept as flat TOML in `settings.toml` in the OS
//! config folder. Read once on launch; the settings window writes it on every change.

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;
mod text;

pub use text::clean_suffix;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LangPref {
    #[default]
    System,
    En,
    Uk,
}

impl LangPref {
    pub fn lang(self) -> Option<crate::i18n::Lang> {
        match self {
            Self::System => None,
            Self::En => Some(crate::i18n::Lang::En),
            Self::Uk => Some(crate::i18n::Lang::Uk),
        }
    }
}

/// What opening a file does while another video is open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Another {
    #[default]
    Ask,
    Replace,
    NewWindow,
}

/// Where Save as… starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SaveTo {
    #[default]
    VideoFolder,
    LastFolder,
}

/// How much has to change on screen for Strip to call it activity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sensitivity {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub lang: LangPref,
    pub another: Another,
    pub check_updates: bool,
    /// Index into `ffmpeg::FORMATS`; `None` keeps the source's container (MP4 when it's another).
    pub format: Option<usize>,
    /// Indexes into `ffmpeg::QUALITIES` and `ffmpeg::SIZES`.
    pub quality: usize,
    pub size: usize,
    pub precise: bool,
    pub mute: bool,
    pub save_to: SaveTo,
    /// The folder of the last export, for [`SaveTo::LastFolder`].
    pub last_folder: Option<PathBuf>,
    /// Added to the video's name for a plain edit, e.g. `-edited`.
    pub suffix: String,
    /// The done sound and the progress on the app icon.
    pub sound: bool,
    /// Seconds the skip keys and buttons jump.
    pub skip: f64,
    pub stop_at_out: bool,
    /// The preview starts muted; the deck's speaker button flips it for the session.
    pub mute_preview: bool,
    pub hwdec: bool,
    /// Strip padding new videos start with, in seconds.
    pub pad: f64,
    pub sensitivity: Sensitivity,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lang: LangPref::System,
            another: Another::Ask,
            check_updates: true,
            format: None,
            quality: 0,
            size: 0,
            precise: false,
            mute: false,
            save_to: SaveTo::VideoFolder,
            last_folder: None,
            suffix: "-edited".into(),
            sound: true,
            skip: 5.0,
            stop_at_out: true,
            mute_preview: false,
            hwdec: true,
            pad: 0.5,
            sensitivity: Sensitivity::Normal,
        }
    }
}

pub const SKIP_RANGE: (f64, f64) = (1.0, 60.0);
pub const PAD_RANGE: (f64, f64) = (0.0, 30.0);

impl Settings {
    /// The saved settings, or the defaults when there's no file yet. An earlier version's
    /// `settings.txt` is read when there's no `settings.toml`; the next save writes the TOML.
    pub fn load() -> Self {
        path()
            .and_then(|p| {
                std::fs::read_to_string(&p)
                    .or_else(|_| std::fs::read_to_string(p.with_extension("txt")))
                    .ok()
            })
            .map_or_else(Self::default, |t| Self::parse(&t))
    }

    /// Write the file, through a temporary one so a crash can't leave half of it.
    pub fn save(&self) -> std::io::Result<()> {
        let path = path().ok_or_else(|| std::io::Error::other("no config folder"))?;
        write(&path, &self.to_text())
    }

    /// `disk` with what changed from `base` to `edited`: edits made from a copy that another
    /// window's save has since overtaken keep that save's other values.
    pub fn merge(base: &Self, edited: &Self, disk: &Self) -> Self {
        let (b, e, d) = (base.to_text(), edited.to_text(), disk.to_text());
        let lines = b.lines().zip(e.lines()).zip(d.lines());
        let text: Vec<&str> = lines
            .map(|((b, e), d)| if b == e { d } else { e })
            .collect();
        Self::parse(&text.join("\n"))
    }
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Per process, so two Chyk windows saving at once don't share one temp file.
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let done = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path));
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done
}

/// `settings.toml` in Chyk's config folder, or in `Chyk\` next to the portable exe.
pub fn path() -> Option<PathBuf> {
    if let Some(dir) = crate::portable::data_dir() {
        return Some(dir.join("settings.toml"));
    }
    let env = |k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support/Chyk")
    } else if cfg!(windows) {
        env("APPDATA")?.join("Chyk")
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".config")))?
            .join("chyk")
    };
    Some(dir.join("settings.toml"))
}
