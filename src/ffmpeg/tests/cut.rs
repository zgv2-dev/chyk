use super::*;

#[test]
fn strip_args() {
    let mut j = job(None, "out.mp4");
    j.keep = Some(vec![(1.5, 2.0), (3.0, 4.0)]);
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    // Close intervals share one input, seeked to the first one.
    let sel = "between(t,0.000,0.500)+between(t,1.500,2.500)";
    assert!(
        a.contains("-ss 1.500 -to 4.000 -i in.mkv -filter_complex"),
        "{a}"
    );
    assert!(
        a.contains(&format!(
            "[0:v:0]select='{sel}',setpts='(T-0.000-gte(T,1.500)*1.000)/TB'[v0];"
        )),
        "{a}"
    );
    assert!(
        a.contains(&format!("[0:a:0]aselect='{sel}',asetpts=N/SR/TB[a0];")),
        "{a}"
    );
    assert!(
        a.contains("[v0][a0]concat=n=1:v=1:a=1[v][a] -map [v] -map [a]"),
        "{a}"
    );
    assert!(a.contains("-c:a aac -b:a 128k"), "{a}");
    assert!(!a.contains("-map 0:s?"), "{a}");
    assert!(a.contains("-map_chapters -1"), "{a}");

    j.quality = COMPRESS;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(a.contains("-c:a aac -b:a 96k"), "{a}");

    j.mute = true;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(!a.contains("aselect"), "{a}");
    assert!(a.ends_with("-an out.mp4"), "{a}");

    // No audio stream: video only, nothing that would need one.
    j.mute = false;
    let a = args(&j, Some("libx264"), &Source::default()).join(" ");
    assert!(a.contains("concat=n=1:v=1:a=0[v] -map [v] -c:v"), "{a}");
    assert!(!a.contains("aac"), "{a}");
}

#[test]
fn far_apart_intervals_get_their_own_seek() {
    let mut j = job(Some([640, 360, 2, 4]), "out.mp4");
    j.start = 0.0;
    j.end = 100.0;
    j.keep = Some(vec![(1.0, 2.0), (5.0, 6.0), (50.0, 51.0), (90.0, 91.5)]);
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(a.contains("-ss 1.000 -to 6.000 -i in.mkv"), "{a}");
    assert!(a.contains("-ss 50.000 -to 51.000 -i in.mkv"), "{a}");
    assert!(a.contains("-ss 90.000 -to 91.500 -i in.mkv"), "{a}");
    assert!(
        a.contains("[2:v:0]crop=640:360:2:4,select='between(t,0.000,1.500)'"),
        "{a}"
    );
    assert!(
        a.contains("[v0][a0][v1][a1][v2][a2]concat=n=3:v=1:a=1[v][a]"),
        "{a}"
    );
}

#[test]
fn spans_merge_close_gaps_then_the_smallest() {
    let keep = [
        (0.0, 1.0),
        (3.0, 4.0),
        (30.0, 31.0),
        (60.0, 61.0),
        (75.0, 76.0),
    ];
    assert_eq!(spans(&keep, 32), [(0, 2), (2, 3), (3, 4), (4, 5)]);
    // Two inputs at most: join the 14 s gap, then the 26 s one.
    assert_eq!(spans(&keep, 2), [(0, 3), (3, 5)]);
    assert_eq!(spans(&keep, 1), [(0, 5)]);
    assert_eq!(max_inputs(260), 12);
    assert_eq!(max_inputs(20), 32);
}

#[test]
fn speed_combines_with_strip() {
    let mut j = job(None, "out.mp4");
    j.keep = Some(vec![(1.5, 2.0), (3.0, 4.0)]);
    j.speed = 2.0;
    let a = args(&j, Some("libx264"), &ANY).join(" ");
    assert!(
        a.contains("concat=n=1:v=1:a=1[v][a];[v]setpts=PTS/2.000000[vs];[a]atempo=2.000000[as]"),
        "{a}"
    );
    assert!(a.contains("-map [vs] -map [as]"), "{a}");
}
