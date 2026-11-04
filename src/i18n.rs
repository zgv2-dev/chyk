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
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    const PLURAL: [&str; 4] = [".one", ".few", ".many", ".other"];

    fn base(key: &str) -> &str {
        PLURAL
            .iter()
            .find_map(|s| key.strip_suffix(s))
            .unwrap_or(key)
    }

    fn placeholders(v: &str) -> BTreeSet<&str> {
        v.split('{')
            .skip(1)
            .filter_map(|s| s.split_once('}').map(|(name, _)| name))
            .filter(|n| *n != "n")
            .collect()
    }

    #[test]
    fn detects_ukrainian_only() {
        for l in ["uk", "uk-UA", "uk_UA.UTF-8", "UK"] {
            assert_eq!(detect(Some(l)), Lang::Uk, "{l}");
        }
        for l in ["en-US", "ru-UA", "de", ""] {
            assert_eq!(detect(Some(l)), Lang::En, "{l}");
        }
        assert_eq!(detect(None), Lang::En);
    }

    #[test]
    fn plural_rules() {
        let uk = |n| category(Lang::Uk, n);
        assert_eq!([uk(1), uk(21), uk(101)], ["one"; 3]);
        assert_eq!([uk(2), uk(4), uk(23)], ["few"; 3]);
        assert_eq!([uk(0), uk(5), uk(11), uk(12), uk(14), uk(111)], ["many"; 6]);
        assert_eq!(
            [category(Lang::En, 1), category(Lang::En, 2)],
            ["one", "other"]
        );
    }

    #[test]
    fn files_agree() {
        for (name, src) in [("en", EN), ("uk", UK)] {
            let mut seen = BTreeSet::new();
            for l in src.lines().map(str::trim) {
                if l.is_empty() || l.starts_with('#') {
                    continue;
                }
                let (k, v) = l
                    .split_once('=')
                    .unwrap_or_else(|| panic!("{name}: no `=` in {l}"));
                assert!(seen.insert(k.trim()), "{name}: duplicate {k}");
                assert!(!v.trim().is_empty(), "{name}: empty {k}");
            }
        }
        let [en, uk] = &*TABLES;
        // Placeholders per base key, across all its plural forms.
        let shape = |t: &HashMap<&'static str, &'static str>| {
            let mut m: BTreeMap<&'static str, BTreeSet<&'static str>> = BTreeMap::new();
            for (k, v) in t {
                m.entry(base(k)).or_default().extend(placeholders(v));
            }
            m
        };
        assert_eq!(
            shape(en),
            shape(uk),
            "keys or placeholders differ between en and uk"
        );
        for k in en.keys().filter(|k| k.ends_with(".one")) {
            for form in PLURAL {
                let key = format!("{}{form}", base(k));
                assert!(uk.contains_key(key.as_str()), "uk is missing {key}");
            }
        }
    }

    /// Every literal key in the sources exists in English.
    #[test]
    fn used_keys_exist() {
        let [en, _] = &*TABLES;
        let sources = [
            include_str!("app.rs"),
            include_str!("chrome.rs"),
            include_str!("ffmpeg.rs"),
            include_str!("strip.rs"),
            include_str!("main.rs"),
        ];
        for src in sources {
            for (mac, plural) in [("t!(\"", false), ("tn!(\"", true)] {
                for (at, _) in src.match_indices(mac) {
                    // Skip `format!("`, `print!("` and friends.
                    let before = src[..at].chars().next_back().unwrap_or(' ');
                    if before.is_alphanumeric() || before == '_' {
                        continue;
                    }
                    let key = src[at + mac.len()..].split('"').next().unwrap();
                    let ok = if plural {
                        en.contains_key(format!("{key}.other").as_str())
                    } else {
                        en.contains_key(key)
                    };
                    assert!(ok, "missing key {key}");
                }
            }
        }
    }

    #[test]
    fn fills_placeholders() {
        assert_eq!(fill("{a} and {b}", &[("a", &1), ("b", &"x")]), "1 and x");
    }
}
