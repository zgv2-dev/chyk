use super::*;

#[test]
fn no_undo_while_loading() {
    assert!(can_undo(true, false));
    assert!(!can_undo(true, true));
    assert!(!can_undo(false, false));
}

#[test]
fn one_change_one_step() {
    let mut h = History::default();
    h.reset(1);
    h.settle(&2);
    assert_eq!(h.undo(&2), Some(1));
    assert_eq!(h.redo(&1), Some(2));
}

#[test]
fn a_new_edit_clears_redo() {
    let mut h = History::default();
    h.reset(1);
    h.settle(&2);
    assert_eq!(h.undo(&2), Some(1));
    h.settle(&3);
    assert_eq!(h.redo(&3), None);
}

#[test]
fn keeps_the_last_100() {
    let mut h = History::default();
    h.reset(0);
    for i in 1..=150 {
        h.settle(&i);
    }
    let mut now = 150;
    let mut n = 0;
    while let Some(p) = h.undo(&now) {
        now = p;
        n += 1;
    }
    assert_eq!(n, 100);
}

#[test]
fn reset_forgets_the_previous_file() {
    let mut h = History::default();
    h.reset(1);
    h.settle(&2);
    h.reset(9);
    assert_eq!(h.undo(&9), None);
}

#[test]
fn a_full_frame_crop_snapshots_as_no_crop() {
    // Opening Crop puts a whole-frame box up; that is not an edit, so not an undo step.
    assert_eq!(edit_crop(Some(crop::full())), None);
    let r = Rect::from_min_max(pos2(0.1, 0.0), pos2(1.0, 1.0));
    assert_eq!(edit_crop(Some(r)), Some(r));
    assert_eq!(edit_crop(None), None);
}
