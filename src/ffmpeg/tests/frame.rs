use super::fixture::*;
use super::*;

#[test]
fn args_crop_then_png() {
    let a = frame_args(
        Path::new("in.mkv"),
        1.5,
        None,
        Some([640, 360, 2, 4]),
        Path::new("o.png"),
    );
    let s = a.join(" ");
    assert!(s.contains("-ss 1.500 -i in.mkv"));
    // `-update 1`: the name is literal, `%d` in it is not an image2 pattern.
    assert!(s.ends_with("-vf crop=640:360:2:4 -update 1 o.png"));
    let plain = frame_args(Path::new("in.mkv"), 0.0, None, None, Path::new("o.png"));
    assert!(!plain.contains(&"-vf".to_string()));
}

#[test]
fn saves_a_name_with_a_percent_pattern() {
    let f = Fixture::new("frame_pct");
    let dst = f.dir.join("f%d.png");
    save_frame(&f.src, 1.0, None, None, &dst).unwrap();
    assert!(dst.exists());
}

#[test]
fn saves_a_png_of_the_crop() {
    let f = Fixture::new("frame");
    let dst = f.dir.join("f.png");
    save_frame(&f.src, 1.0, None, Some([64, 36, 10, 10]), &dst).unwrap();
    assert_eq!(probe(&dst, "v:0", "stream=width,height"), "64,36");
    assert_eq!(probe(&dst, "v:0", "stream=codec_name"), "png");
}

#[test]
fn saves_a_join_clip_fitted_into_its_frame_then_cropped() {
    let f = Fixture::new("frame_join");
    let dst = f.dir.join("f.png");
    let fit = Some((Fit::Fit, [1280, 720]));
    save_frame(&f.src, 1.0, fit, None, &dst).unwrap();
    assert_eq!(probe(&dst, "v:0", "stream=width,height"), "1280,720");
    save_frame(&f.src, 1.0, fit, Some([640, 360, 0, 0]), &dst).unwrap();
    assert_eq!(probe(&dst, "v:0", "stream=width,height"), "640,360");
}
