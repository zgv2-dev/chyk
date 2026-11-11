use super::*;

fn item(path: &str, opened: u64) -> Recent {
    Recent {
        path: path.into(),
        opened,
        codec: "h264".into(),
        size: [1920, 1080],
        dur: 12.5,
    }
}

#[test]
fn round_trips_unicode_and_spaces() {
    let list = vec![item("/Users/a/Відео/my clip.mov", 5), item("/x\ty/z", 4)];
    assert_eq!(parse(&format(&list)), list);
}

#[test]
fn push_dedupes_and_keeps_ten() {
    let mut list = Vec::new();
    for i in 0..12 {
        push(&mut list, item(&format!("/v/{i}.mp4"), i));
    }
    assert_eq!(list.len(), 10);
    assert_eq!(list[0].path, PathBuf::from("/v/11.mp4"));
    push(&mut list, item("/v/5.mp4", 99));
    assert_eq!(list.len(), 10);
    assert_eq!(
        (list[0].path.clone(), list[0].opened),
        ("/v/5.mp4".into(), 99)
    );
    assert_eq!(
        list.iter()
            .filter(|r| r.path.as_path() == Path::new("/v/5.mp4"))
            .count(),
        1
    );
}

#[test]
fn parse_skips_broken_lines() {
    let text = format!("garbage\n\n1\t2\n{}x\tnope\n", "");
    let good = format(&[item("/a b.mp4", 7)]);
    let list = parse(&format!("{text}{good}1\t2.0\t10x\th264\t/p\n"));
    assert_eq!(list, vec![item("/a b.mp4", 7)]);
}

fn unique(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("chyk-recent-{name}-{}", std::process::id()))
}

#[test]
fn save_into_missing_folder_creates_it() {
    let dir = unique("missing").join("deeper");
    let file = dir.join("recent");
    let list = vec![item("/Users/a/Відео/my clip.mov", 5)];
    save_to(&file, &list).unwrap();
    assert_eq!(parse(&std::fs::read_to_string(&file).unwrap()), list);
    let _ = std::fs::remove_dir_all(unique("missing"));
}

#[test]
fn unwritable_folder_returns_err() {
    let blocker = unique("blocker");
    std::fs::write(&blocker, "a file, not a folder").unwrap();
    assert!(save_to(&blocker.join("recent"), &[item("/a.mp4", 1)]).is_err());
    let _ = std::fs::remove_file(&blocker);
}

#[test]
fn thumb_header_round_trips() {
    let bytes = encode_thumb([2, 1], &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        decode_thumb(&bytes),
        Some(([2, 1], vec![1, 2, 3, 4, 5, 6, 7, 8]))
    );
    assert_eq!(decode_thumb(&bytes[..10]), None);
}
