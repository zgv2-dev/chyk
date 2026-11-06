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
    fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(std::fs::read_to_string(&path).unwrap());
            }
        }
    }
    let mut sources = Vec::new();
    walk(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
        &mut sources,
    );
    for src in &sources {
        for (mac, plural) in [("t!(\"", false), ("tn!(\"", true)] {
            for (at, _) in src.match_indices(mac) {
                // Skip `format!("`, `print!("` and friends.
                let before = src[..at].chars().next_back().unwrap_or(' ');
                if before.is_alphanumeric() || before == '_' {
                    continue;
                }
                // Skip examples in comments.
                let line = src[..at].rfind('\n').map_or(0, |i| i + 1);
                if src[line..at].trim_start().starts_with("//") {
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
