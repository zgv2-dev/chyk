//! The settings file's text: one `key = value` line per setting.

use super::*;

const LANGS: [(LangPref, &str); 3] = [
    (LangPref::System, "system"),
    (LangPref::En, "en"),
    (LangPref::Uk, "uk"),
];
const ANOTHER: [(Another, &str); 3] = [
    (Another::Ask, "ask"),
    (Another::Replace, "replace"),
    (Another::NewWindow, "new-window"),
];
const SAVE_TO: [(SaveTo, &str); 2] = [(SaveTo::VideoFolder, "video"), (SaveTo::LastFolder, "last")];
const SENSITIVITY: [(Sensitivity, &str); 3] = [
    (Sensitivity::Low, "low"),
    (Sensitivity::Normal, "normal"),
    (Sensitivity::High, "high"),
];

fn name<T: PartialEq>(v: &T, table: &[(T, &'static str)]) -> &'static str {
    table.iter().find(|(t, _)| t == v).map_or("", |(_, n)| n)
}

fn named<T: Copy>(s: &str, table: &[(T, &str)]) -> Option<T> {
    table.iter().find(|(_, n)| *n == s).map(|(t, _)| *t)
}

impl Settings {
    // shortcut: paths are stored as UTF-8 text, like the session file; a non-UTF-8 last folder
    // (rare, Linux only) is just forgotten.
    pub fn to_text(&self) -> String {
        let flag = |b: bool| u8::from(b);
        let format = self
            .format
            .map_or_else(|| "source".into(), |f| f.to_string());
        let folder = self
            .last_folder
            .as_ref()
            .and_then(|p| p.to_str())
            .map(escape)
            .unwrap_or_default();
        [
            format!("lang = {}", name(&self.lang, &LANGS)),
            format!("another = {}", name(&self.another, &ANOTHER)),
            format!("check_updates = {}", flag(self.check_updates)),
            format!("format = {format}"),
            format!("quality = {}", self.quality),
            format!("size = {}", self.size),
            format!("precise = {}", flag(self.precise)),
            format!("mute = {}", flag(self.mute)),
            format!("save_to = {}", name(&self.save_to, &SAVE_TO)),
            format!("last_folder = {folder}"),
            format!("suffix = {}", self.suffix),
            format!("sound = {}", flag(self.sound)),
            format!("skip = {}", self.skip),
            format!("stop_at_out = {}", flag(self.stop_at_out)),
            format!("hwdec = {}", flag(self.hwdec)),
            format!("pad = {}", self.pad),
            format!("sensitivity = {}", name(&self.sensitivity, &SENSITIVITY)),
        ]
        .join("\n")
            + "\n"
    }

    /// Parse [`Settings::to_text`]. Unknown keys and bad values keep their defaults.
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for (k, raw) in text.lines().filter_map(|l| l.split_once('=')) {
            let v = raw.trim();
            // Text values keep their own spaces: only the one after `=` is the file's.
            let val = raw.strip_prefix(' ').unwrap_or(raw);
            let flag = |old: bool| match v {
                "1" => true,
                "0" => false,
                _ => old,
            };
            let num = |old: f64, (lo, hi): (f64, f64)| {
                v.parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map_or(old, |n| n.clamp(lo, hi))
            };
            match k.trim() {
                "lang" => s.lang = named(v, &LANGS).unwrap_or(s.lang),
                "another" => s.another = named(v, &ANOTHER).unwrap_or(s.another),
                "check_updates" => s.check_updates = flag(s.check_updates),
                "format" => s.format = v.parse().ok().filter(|&f| f < crate::ffmpeg::FORMATS.len()),
                "quality" => s.quality = index(v, crate::ffmpeg::QUALITIES.len(), s.quality),
                "size" => s.size = index(v, crate::ffmpeg::SIZES.len(), s.size),
                "precise" => s.precise = flag(s.precise),
                "mute" => s.mute = flag(s.mute),
                "save_to" => s.save_to = named(v, &SAVE_TO).unwrap_or(s.save_to),
                "last_folder" => {
                    s.last_folder = (!val.is_empty()).then(|| PathBuf::from(unescape(val)));
                }
                "suffix" => s.suffix = clean_suffix(val),
                "sound" => s.sound = flag(s.sound),
                "skip" => s.skip = num(s.skip, SKIP_RANGE),
                "stop_at_out" => s.stop_at_out = flag(s.stop_at_out),
                "hwdec" => s.hwdec = flag(s.hwdec),
                "pad" => s.pad = num(s.pad, PAD_RANGE),
                "sensitivity" => s.sensitivity = named(v, &SENSITIVITY).unwrap_or(s.sensitivity),
                _ => {}
            }
        }
        s
    }
}

fn index(v: &str, len: usize, old: usize) -> usize {
    v.parse().ok().filter(|&i| i < len).unwrap_or(old)
}

/// The suffix without characters a file name can't hold.
pub fn clean_suffix(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .take(40)
        .collect()
}

/// One line per setting: a folder's backslashes and line breaks are written as `\\`, `\n`, `\r`.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            // Not one of ours: kept as written.
            other => out.extend(std::iter::once('\\').chain(other)),
        }
    }
    out
}
