use super::fixture::*;
use super::*;

#[test]
fn args_crop_then_png() {
    let a = frame_args(
        Path::new("in.mkv"),
        1.5,
        Some([640, 360, 2, 4]),
        Path::new("o.png"),
    );
    let s = a.join(" ");
    assert!(s.contains("-ss 1.500 -i in.mkv"));
    assert!(s.ends_with("-vf crop=640:360:2:4 o.png"));
    let plain = frame_args(Path::new("in.mkv"), 0.0, None, Path::new("o.png"));
    assert!(!plain.contains(&"-vf".to_string()));
}

#[test]
fn saves_a_png_of_the_crop() {
    let f = Fixture::new("frame");
    let dst = f.dir.join("f.png");
    save_frame(&f.src, 1.0, Some([64, 36, 10, 10]), &dst).unwrap();
    assert_eq!(probe(&dst, "v:0", "stream=width,height"), "64,36");
    assert_eq!(probe(&dst, "v:0", "stream=codec_name"), "png");
}
