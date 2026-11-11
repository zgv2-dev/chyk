use super::body::columns;
use super::*;
use std::collections::{BTreeSet, HashSet};

#[test]
fn sheet_lists_every_binding_once() {
    let rows = sheet_rows("");
    let names: HashSet<_> = bindings().iter().map(|b| label(b.cmd)).collect();
    assert_eq!(rows.len(), names.len());
    assert_eq!(
        rows.iter().map(|r| r.2.len()).sum::<usize>(),
        bindings().len()
    );
}

#[test]
fn presets_and_recents_share_a_row() {
    let rows = sheet_rows("");
    let n = |f: fn(&Cmd) -> bool| rows.iter().find(|r| f(&r.1)).unwrap().2.len();
    assert_eq!(n(|c| matches!(c, Cmd::CropPreset(_))), 6);
    assert_eq!(n(|c| matches!(c, Cmd::OpenRecent(_))), 5);
}

#[test]
fn filter_narrows_rows() {
    let rows = sheet_rows("ZOOM");
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| matches!(
        r.1,
        Cmd::ZoomIn | Cmd::ZoomOut | Cmd::ZoomFit | Cmd::TimelineIn | Cmd::TimelineOut
    )));
    assert!(sheet_rows("no such command").is_empty());
}

#[test]
fn every_group_lands_in_a_column() {
    let rows = sheet_rows("");
    let placed: BTreeSet<_> = columns(&rows).into_iter().flatten().collect();
    assert_eq!(placed, rows.iter().map(|r| r.0).collect());
}

#[test]
fn every_binding_is_listed_or_folds_into_a_listed_one() {
    for (_, cmd, bs) in sheet_rows("") {
        if matches!(cmd, Cmd::CropPreset(_) | Cmd::OpenRecent(_)) {
            continue;
        }
        let listed = shown(&bs);
        for b in &bs {
            let listed_itself = listed.iter().any(|s| std::ptr::eq(*s, b));
            let folded = listed
                .iter()
                .any(|s| s.key == b.key || (b.key == Key::Equals && s.key == Key::Plus));
            assert!(listed_itself || folded, "{:?}", b.cmd);
        }
    }
}

#[test]
fn alternatives_that_are_not_layout_variants_stay() {
    let n = |c: Cmd| shown(&sheet_rows("").into_iter().find(|r| r.1 == c).unwrap().2).len();
    assert_eq!(n(Cmd::DeleteZone), 2);
    assert_eq!(n(Cmd::Slower), 1);
    assert_eq!(n(Cmd::ZoomIn), 1);
    assert_eq!(n(Cmd::Redo), if cfg!(target_os = "macos") { 1 } else { 2 });
}
