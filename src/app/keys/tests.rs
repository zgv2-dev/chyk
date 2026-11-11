use super::*;
use std::collections::HashSet;

#[test]
fn no_two_bindings_share_a_chord() {
    let mut seen = HashSet::new();
    for b in bindings() {
        assert!(seen.insert((b.mods, b.key)), "{:?} {:?}", b.mods, b.key);
    }
}

#[test]
fn every_command_has_a_label_in_english() {
    let en = include_str!("../../../assets/i18n/en.lang");
    let has = |k: &str| {
        en.lines()
            .any(|l| l.split_once(" = ").is_some_and(|(a, _)| a == k))
    };
    for b in bindings() {
        assert!(has(label(b.cmd)), "{}", label(b.cmd));
    }
}

#[test]
fn caps_follow_the_platform() {
    assert_eq!(
        caps(Modifiers::COMMAND | Modifiers::SHIFT, Key::R, true),
        ["⇧", "⌘", "R"]
    );
    assert_eq!(
        caps(Modifiers::COMMAND | Modifiers::SHIFT, Key::R, false),
        ["Ctrl", "Shift", "R"]
    );
    assert_eq!(caps(Modifiers::NONE, Key::ArrowLeft, true), ["←"]);
    assert_eq!(caps(Modifiers::NONE, Key::Space, false), ["Space"]);
}

#[test]
fn existing_keys_keep_their_commands() {
    let find = |m, k| {
        bindings()
            .into_iter()
            .find(|b| b.mods == m && b.key == k)
            .map(|b| b.cmd)
    };
    let (n, c) = (Modifiers::NONE, Modifiers::COMMAND);
    assert_eq!(find(n, Key::Space), Some(Cmd::PlayPause));
    assert_eq!(find(n, Key::I), Some(Cmd::SetIn));
    assert_eq!(find(n, Key::O), Some(Cmd::SetOut));
    assert_eq!(find(n, Key::C), Some(Cmd::Crop));
    assert_eq!(find(n, Key::Z), Some(Cmd::EditZones));
    assert_eq!(find(c, Key::S), Some(Cmd::Export));
    assert_eq!(find(c, Key::W), Some(Cmd::CloseVideo));
    assert_eq!(find(n, Key::OpenBracket), Some(Cmd::TimelineOut));
    assert_eq!(find(n, Key::CloseBracket), Some(Cmd::TimelineIn));
    for k in [Key::Plus, Key::Equals] {
        assert_eq!(find(n, k), Some(Cmd::ZoomIn));
    }
    assert_eq!(find(Modifiers::SHIFT, Key::Plus), Some(Cmd::ZoomIn));
    assert_eq!(find(Modifiers::SHIFT, Key::Comma), Some(Cmd::Slower));
    assert_eq!(find(Modifiers::SHIFT, Key::Period), Some(Cmd::Faster));
    assert_eq!(find(n, Key::Delete), Some(Cmd::DeleteZone));
    assert_eq!(find(n, Key::Backspace), Some(Cmd::DeleteZone));
}

#[test]
fn tooltip_strings_carry_no_key_suffix() {
    let en = include_str!("../../../assets/i18n/en.lang");
    let keys = [
        "tip.play",
        "tip.pause",
        "tip.zoom_out",
        "tip.zoom_in",
        "tip.zoom_fit",
        "tip.timeline_fit",
        "tip.timeline_selection",
        "tip.timeline_in",
        "tip.timeline_out",
        "strip.edit_zones",
        "tip.close_video",
        "tip.export",
        "tip.speed",
    ];
    for k in keys {
        let v = en
            .lines()
            .find_map(|l| l.split_once(" = ").filter(|(a, _)| *a == k))
            .unwrap_or_else(|| panic!("{k} missing"))
            .1;
        assert!(!v.ends_with(')'), "{k}: {v}");
        for s in ["(Space)", "(+)", "(])", "{key}"] {
            assert!(!v.contains(s), "{k}: {v}");
        }
    }
}
