use super::*;
use Button::*;

#[test]
fn parses_gnome_layouts() {
    let l = parse("appmenu:minimize,maximize,close");
    assert_eq!((l.left, l.right), (vec![], vec![Minimize, Maximize, Close]));
    let l = parse("close,minimize,maximize:");
    assert_eq!((l.left, l.right), (vec![Close, Minimize, Maximize], vec![]));
    let l = parse("icon:close");
    assert_eq!((l.left, l.right), (vec![], vec![Close]));
    let l = parse("minimize, close");
    assert_eq!((l.left, l.right), (vec![], vec![Minimize, Close]));
}
