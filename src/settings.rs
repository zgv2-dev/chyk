//! What the user set in the settings window, kept as `key = value` lines in `settings.txt` in the
//! OS config folder. Read once on launch; the settings window writes it on every change.

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
            hwdec: true,
            pad: 0.5,
            sensitivity: Sensitivity::Normal,
        }
    }
}

pub const SKIP_RANGE: (f64, f64) = (1.0, 60.0);
pub const PAD_RANGE: (f64, f64) = (0.0, 30.0);

impl Settings {
    /// The saved settings, or the defaults when there's no file yet.
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(Self::default, |t| Self::parse(&t))
    }

    /// Write the file, through a temporary one so a crash can't leave half of it.
    pub fn save(&self) -> std::io::Result<()> {
        let path = path().ok_or_else(|| std::io::Error::other("no config folder"))?;
        write(&path, &self.to_text())
    }
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// `settings.txt` in Chyk's config folder.
pub fn path() -> Option<PathBuf> {
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
    Some(dir.join("settings.txt"))
}
