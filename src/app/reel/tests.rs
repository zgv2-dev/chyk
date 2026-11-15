use super::*;

fn clip(name: &str, dur: f64, size: [u32; 2], fps: f64) -> Clip {
    let info = ClipInfo {
        dur,
        size,
        fps,
        codec: "h264".into(),
    };
    Clip::new(name.into(), info)
}

fn reel() -> Reel {
    let mut clips = vec![
        clip("a.mp4", 10.0, [1920, 1080], 30.0),
        clip("b,c;d.mov", 5.0, [3840, 2160], 60.0),
        clip("e.mkv", 8.0, [1081, 1921], 25.0),
    ];
    clips[0].trim = (2.0, 6.0);
    clips[1].trim = (1.0, 4.0);
    let (frame, fps) = largest(&clips);
    Reel {
        clips,
        sel: 0,
        frame,
        fps,
        shown: None,
    }
}

#[test]
fn largest_takes_each_side_and_rate_on_its_own() {
    let r = reel();
    assert_eq!(r.frame, [3840, 2160]);
    assert_eq!(r.fps, 60.0);
    let (f, _) = largest(&[clip("x", 1.0, [1081, 1921], 25.0)]);
    assert_eq!(f, [1082, 1922]);
}

#[test]
fn timeline_positions() {
    let r = reel();
    assert_eq!(r.total(), 23.0);
    assert_eq!(r.start(2), 15.0);
    assert_eq!(r.at(9.99), 0);
    assert_eq!(r.at(10.0), 1);
    assert_eq!(r.at(99.0), 2);
    assert_eq!(r.kept_range(1), (11.0, 14.0));
    assert_eq!(r.kept(), 4.0 + 3.0 + 8.0);
    assert_eq!(r.output_time(12.0), 4.0 + 1.0);
}

#[test]
fn playback_skips_what_is_cut() {
    let r = reel();
    assert_eq!(r.play_target(0.5), Ok(Some(2.0)));
    assert_eq!(r.play_target(3.0), Ok(None));
    assert_eq!(r.play_target(6.0), Ok(Some(11.0)));
    assert_eq!(r.play_target(14.5), Ok(Some(15.0)));
    assert_eq!(r.play_target(23.0), Err(()));
}

#[test]
fn edl_quotes_paths_by_length() {
    let e = reel().edl();
    assert!(
        e.starts_with("edl://%5%a.mp4,0,10.000000;%9%b,c;d.mov,0,5.000000;"),
        "{e}"
    );
}

#[test]
fn shifting_moves_the_selection_with_the_clip() {
    let mut r = reel();
    assert_eq!(r.shift(0, 1), 1);
    assert_eq!(r.clips[1].path, PathBuf::from("a.mp4"));
    assert_eq!(r.sel, 1);
    assert_eq!(r.shift(1, 5), 2);
    assert_eq!(r.shift(2, -9), 0);
    assert!(!r.uniform());
}
