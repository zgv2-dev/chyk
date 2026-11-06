use super::fixture::Fixture;
use super::*;

/// Needs `ffmpeg` on PATH.
#[test]
fn thumbnails_batch_keyframe_and_crop() {
    let f = Fixture::new("thumbs");
    let src = &f.src;
    // The filmstrip refreshes one batch for the visible range.
    let (tx, rx) = channel();
    thumbnails(
        src.clone(),
        (1.0, 2.0),
        4,
        [96, 72],
        None,
        tx,
        Context::default(),
    );
    let mut got = std::collections::BTreeSet::new();
    while got.len() < 4 {
        match rx.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok((i, _)) => assert!(got.insert(i), "tile {i} sent twice"),
            Err(e) => panic!("thumbnails: {e}"),
        }
    }
    assert_eq!(got.into_iter().collect::<Vec<_>>(), [0, 1, 2, 3]);
    // Far apart tiles take the keyframe path.
    let (tx, rx) = channel();
    thumbnails(
        src.clone(),
        (0.0, 4.0),
        1,
        [96, 72],
        None,
        tx,
        Context::default(),
    );
    let (_, whole) = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("keyframe thumbnail");
    // A crop shows only its part of the frame.
    let (tx, rx) = channel();
    let corner = eframe::egui::Rect::from_min_max(
        eframe::egui::pos2(0.0, 0.0),
        eframe::egui::pos2(0.3, 0.3),
    );
    thumbnails(
        src.clone(),
        (0.0, 4.0),
        1,
        [96, 72],
        Some(corner),
        tx,
        Context::default(),
    );
    let (_, cropped) = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("cropped thumbnail");
    assert_eq!(cropped.size, [96, 72]);
    assert_ne!(cropped.pixels, whole.pixels);
}
