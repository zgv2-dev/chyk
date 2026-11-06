//! Interface text. Ukrainian when the OS language is Ukrainian, English otherwise.
//! Strings live in `assets/i18n/*.lang` as `key = value` lines; `{name}` placeholders are
//! filled by [`t!`], and plural keys end in `.one`/`.few`/`.many`/`.other` (see [`tn!`]).
//! Edit them with `python3 scripts/i18n-editor.py`. `GLINT_LANG=uk|en` overrides detection.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Uk,
}

const EN: &str = include_str!("../assets/i18n/en.lang");
const UK: &str = include_str!("../assets/i18n/uk.lang");

pub static LANG: LazyLock<Lang> = LazyLock::new(|| {
    detect(
        std::env::var("GLINT_LANG")
            .ok()
            .or_else(sys_locale::get_locale)
            .as_deref(),
    )
});

static TABLES: LazyLock<[HashMap<&'static str, &'static str>; 2]> =
    LazyLock::new(|| [parse(EN), parse(UK)]);

/// `uk`, `uk-UA`, `uk_UA.UTF-8` → Ukrainian; anything else → English.
fn detect(locale: Option<&str>) -> Lang {
    let lang = locale
        .and_then(|l| l.split(['-', '_', '.', '@']).next())
        .unwrap_or_default();
    if lang.eq_ignore_ascii_case("uk") {
        Lang::Uk
    } else {
        Lang::En
    }
}

fn parse(src: &'static str) -> HashMap<&'static str, &'static str> {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        // An empty value means "not translated yet": fall back to English.
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// Text for `key` in the UI language, falling back to English, then to the key itself.
pub fn tr(key: &'static str) -> &'static str {
    let [en, uk] = &*TABLES;
    let table = if *LANG == Lang::Uk { uk } else { en };
    table
        .get(key)
        .or_else(|| en.get(key))
        .copied()
        .unwrap_or(key)
}

/// `text` with each `{name}` replaced.
pub fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    args.iter().fold(text.to_string(), |s, (name, value)| {
        s.replace(&format!("{{{name}}}"), &value.to_string())
    })
}

/// CLDR plural category for a whole number.
fn category(lang: Lang, n: u64) -> &'static str {
    match lang {
        Lang::En if n == 1 => "one",
        Lang::En => "other",
        Lang::Uk => match (n % 10, n % 100) {
            (1, r) if r != 11 => "one",
            (2..=4, r) if !(12..=14).contains(&r) => "few",
            _ => "many",
        },
    }
}

/// Plural form of `base` for `n`, with `{n}` and any other placeholders filled.
pub fn plural(base: &str, n: u64, args: &[(&str, &dyn Display)]) -> String {
    let [en, uk] = &*TABLES;
    let pick = |lang: Lang, table: &HashMap<&'static str, &'static str>| {
        let cat = category(lang, n);
        table
            .get(format!("{base}.{cat}").as_str())
            .or_else(|| table.get(format!("{base}.other").as_str()))
            .copied()
    };
    let text = (*LANG == Lang::Uk)
        .then(|| pick(Lang::Uk, uk))
        .flatten()
        .or_else(|| pick(Lang::En, en))
        .unwrap_or(base);
    let mut all: Vec<(&str, &dyn Display)> = vec![("n", &n)];
    all.extend_from_slice(args);
    fill(text, &all)
}

/// `t!("key")` → `&'static str`; `t!("key", name = value, ...)` → `String` with placeholders filled.
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::tr($key)
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::tr($key),
            &[$((stringify!($name), &$value as &dyn std::fmt::Display)),+],
        )
    };
}

/// `tn!("base", n)` → plural form of `base` for `n`, with `{n}` filled.
macro_rules! tn {
    ($base:literal, $n:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::plural(
            $base,
            $n as u64,
            &[$((stringify!($name), &$value as &dyn std::fmt::Display)),*],
        )
    };
}

pub(crate) use {t, tn};
#[cfg(test)]
mod tests;
