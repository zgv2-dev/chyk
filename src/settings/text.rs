//! The settings file's text: flat TOML, one top-level `key = value` per setting.

use super::*;

const LANGS: [(LangPref, &str); 3] = [
    (LangPref::System, "system"),
    (LangPref::En, "en"),
    (LangPref::Uk, "uk"),
];
const UI_SIZES: [(UiSize, &str); 4] = [
    (UiSize::Auto, "auto"),
    (UiSize::P100, "100"),
    (UiSize::P125, "125"),
    (UiSize::P150, "150"),
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
    /// Flat TOML: one top-level `key = value` per setting, no tables.
    pub fn to_text(&self) -> String {
        let format = self
            .format
            .map_or_else(|| quote("source"), |f| f.to_string());
        let folder = self.last_folder.as_ref().and_then(|p| p.to_str());
        [
            format!("lang = {}", quote(name(&self.lang, &LANGS))),
            format!("ui_size = {}", quote(name(&self.ui_size, &UI_SIZES))),
            format!("another = {}", quote(name(&self.another, &ANOTHER))),
            format!("check_updates = {}", self.check_updates),
            format!("format = {format}"),
            format!("quality = {}", self.quality),
            format!("size = {}", self.size),
            format!("precise = {}", self.precise),
            format!("mute = {}", self.mute),
            format!("save_to = {}", quote(name(&self.save_to, &SAVE_TO))),
            format!("last_folder = {}", quote(folder.unwrap_or_default())),
            format!("suffix = {}", quote(&self.suffix)),
            format!("sound = {}", self.sound),
            format!("skip = {:?}", self.skip),
            format!("stop_at_out = {}", self.stop_at_out),
            format!("mute_preview = {}", self.mute_preview),
            format!("hwdec = {}", self.hwdec),
            format!("pad = {:?}", self.pad),
            format!(
                "sensitivity = {}",
                quote(name(&self.sensitivity, &SENSITIVITY))
            ),
        ]
        .join("\n")
            + "\n"
    }

    /// Parse [`Settings::to_text`], or the `settings.txt` lines of earlier versions (bare text,
    /// `0`/`1` flags). Unknown keys, tables, comments and bad values keep their defaults.
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for (k, raw) in text.lines().filter_map(|l| l.split_once('=')) {
            let raw = raw.strip_prefix(' ').unwrap_or(raw);
            let lead = raw.trim_start();
            let val = match lead.strip_prefix('"') {
                Some(rest) => match unquote(rest) {
                    Some(v) => v,
                    None => continue,
                },
                None if lead.starts_with('\'') => lead[1..].split('\'').next().unwrap_or("").into(),
                // Earlier versions wrote text bare, with only the space after `=` being the file's.
                None if matches!(k.trim(), "suffix" | "last_folder") => legacy(raw),
                None => raw.split('#').next().unwrap_or("").trim().to_string(),
            };
            let v = val.as_str();
            let flag = |old: bool| match v {
                "true" | "1" => true,
                "false" | "0" => false,
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
                "ui_size" => s.ui_size = named(v, &UI_SIZES).unwrap_or(s.ui_size),
                "another" => s.another = named(v, &ANOTHER).unwrap_or(s.another),
                "check_updates" => s.check_updates = flag(s.check_updates),
                "format" => s.format = v.parse().ok().filter(|&f| f < crate::ffmpeg::FORMATS.len()),
                "quality" => s.quality = index(v, crate::ffmpeg::QUALITIES.len(), s.quality),
                "size" => s.size = index(v, crate::ffmpeg::SIZES.len(), s.size),
                "precise" => s.precise = flag(s.precise),
                "mute" => s.mute = flag(s.mute),
                "save_to" => s.save_to = named(v, &SAVE_TO).unwrap_or(s.save_to),
                "last_folder" => s.last_folder = (!v.is_empty()).then(|| PathBuf::from(v)),
                "suffix" => s.suffix = clean_suffix(v),
                "sound" => s.sound = flag(s.sound),
                "skip" => s.skip = num(s.skip, SKIP_RANGE),
                "stop_at_out" => s.stop_at_out = flag(s.stop_at_out),
                "mute_preview" => s.mute_preview = flag(s.mute_preview),
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

/// A TOML basic string.
fn quote(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A TOML basic string after its opening quote. `None` (keep the default) if unclosed or bad.
fn unquote(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => out.push(match chars.next()? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'b' => '\u{8}',
                'f' => '\u{c}',
                e @ ('u' | 'U') => {
                    let hex: String = chars.by_ref().take(if e == 'u' { 4 } else { 8 }).collect();
                    u32::from_str_radix(&hex, 16)
                        .ok()
                        .and_then(char::from_u32)?
                }
                c @ ('"' | '\\') => c,
                _ => return None,
            }),
            c => out.push(c),
        }
    }
    None
}

/// A bare text value from `settings.txt`: backslashes and line breaks written as `\\`, `\n`, `\r`;
/// any other backslash kept as written.
fn legacy(s: &str) -> String {
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
            other => out.extend(std::iter::once('\\').chain(other)),
        }
    }
    out
}
