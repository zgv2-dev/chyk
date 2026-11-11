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
