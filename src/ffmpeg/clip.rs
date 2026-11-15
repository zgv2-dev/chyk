//! What the editor needs to know about a clip it joins, from `ffmpeg -i`: mpv only describes the
//! clip that is playing.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct ClipInfo {
    pub dur: f64,
    /// Frame size after rotation.
    pub size: [u32; 2],
    pub fps: f64,
    /// Video codec as ffmpeg names it (`h264`, `hevc`, ...).
    pub codec: String,
}

pub fn probe_clip(src: &Path) -> Option<ClipInfo> {
    parse_clip(&listing(src))
}

pub(super) fn parse_clip(listing: &str) -> Option<ClipInfo> {
    let st = parse_streams(listing);
    let dur = listing.lines().find_map(|l| {
        let t = l
            .trim_start()
            .strip_prefix("Duration: ")?
            .split(',')
            .next()?;
        let mut p = t.split(':').map(|x| x.trim().parse::<f64>());
        Some(p.next()?.ok()? * 3600.0 + p.next()?.ok()? * 60.0 + p.next()?.ok()?)
    })?;
    let [w, h] = st.size?;
    let quarter = st
        .rotation
        .is_some_and(|r| (r.abs().round() as i64) % 180 == 90);
    let codec = st.video?.split([' ', '|']).next()?.to_string();
    Some(ClipInfo {
        dur,
        size: if quarter { [h, w] } else { [w, h] },
        fps: st.fps.unwrap_or(30.0),
        codec,
    })
}

#[cfg(test)]
#[test]
fn clip_from_the_listing() {
    let l = "  Duration: 00:01:02.50, start: 0.000000, bitrate: 3253 kb/s\n  Stream #0:0[0x1](und): Video: hevc (Main) (hvc1 / 0x31637668), yuv420p(tv, bt709), 1920x1080, 3000 kb/s, 59.94 fps, 59.94 tbr, 600 tbn (default)\n      Side data:\n        displaymatrix: rotation of -90.00 degrees";
    let c = parse_clip(l).unwrap();
    assert_eq!(c.size, [1080, 1920]);
    assert!((c.dur - 62.5).abs() < 1e-9 && (c.fps - 59.94).abs() < 1e-9);
    assert_eq!(c.codec, "hevc");
    assert_eq!(parse_clip("  Duration: N/A"), None);
    // A rotated second video stream (a cover, a depth map) doesn't turn the first.
    let two = "  Duration: 00:00:01.00, start: 0.000000\n  Stream #0:0: Video: h264 (High), yuv420p, 1920x1080, 30 fps, 30 tbr, 15360 tbn\n  Stream #0:1: Video: h264 (High), yuv420p, 1920x1080, 30 fps, 30 tbr, 15360 tbn\n      Side data:\n        displaymatrix: rotation of -90.00 degrees";
    assert_eq!(parse_clip(two).unwrap().size, [1920, 1080]);
}
