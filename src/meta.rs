//! File info for the info popover, read from mpv's properties when it opens.
//! Anything mpv doesn't report is left out rather than shown as unknown.

use crate::app::{fmt_size, fmt_time};
use crate::i18n::t;
use crate::player::Player;
use egui_phosphor::regular as icon;
use std::path::Path;

pub struct Section {
    pub icon: &'static str,
    pub title: String,
    pub rows: Vec<(String, String)>,
}

/// Plain-text version for the clipboard.
pub fn to_text(name: &str, sections: &[Section]) -> String {
    let mut s = format!("{name}\n");
    for sec in sections {
        s.push_str(&format!("\n{}\n", sec.title));
        for (k, v) in &sec.rows {
            s.push_str(&format!("  {k}: {v}\n"));
        }
    }
    s
}

/// `frame` is the displayed (rotated) frame size.
pub fn read(player: &Player, path: &Path, dur: f64, frame: [u32; 2]) -> Vec<Section> {
    let s = |p: &str| player.get::<String>(p).filter(|v| !v.trim().is_empty());
    let i = |p: &str| player.get::<i64>(p).filter(|v| *v > 0);
    let f = |p: &str| player.get::<f64>(p).filter(|v| v.is_finite() && *v > 0.0);
    let mut out = Vec::new();

    let mut file = Vec::new();
    if let Some(ext) = path.extension() {
        file.push((t!("meta.container"), ext.to_string_lossy().to_uppercase()));
    }
    let size = i("file-size");
    if let Some(n) = size {
        file.push((t!("meta.size"), fmt_size(n as u64)));
    }
    if dur > 0.0 {
        file.push((t!("meta.duration"), fmt_time(dur)));
        if let Some(n) = size {
            file.push((t!("meta.bitrate"), bitrate(n as f64 * 8.0 / dur)));
        }
    }
    if let Some(c) = s("metadata/by-key/creation_time").or_else(|| s("metadata/by-key/date")) {
        // "2026-10-28T18:42:11.000000Z" → "2026-10-28 18:42"
        file.push((
            t!("meta.created"),
            c.replace('T', " ").chars().take(16).collect(),
        ));
    }
    let device = [
        s("metadata/by-key/com.apple.quicktime.make"),
        s("metadata/by-key/com.apple.quicktime.model"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if !device.is_empty() {
        file.push((t!("meta.device"), device));
    }
    push(&mut out, icon::FILE, t!("meta.file"), file);

    let mut video = Vec::new();
    let v = "current-tracks/video";
    if let Some(codec) = codec(&s, v) {
        video.push((t!("meta.codec"), codec));
    }
    let [w, h] = frame;
    video.push((
        t!("meta.resolution"),
        format!("{w}×{h} · {}", ratio(w.into(), h.into())),
    ));
    if let Some(fps) = f("container-fps").or_else(|| f(&format!("{v}/demux-fps"))) {
        video.push((
            t!("meta.frame_rate"),
            format!("{} {}", trim_float(fps, 3), t!("unit.fps")),
        ));
    }
    if let Some(px) = s("video-params/hw-pixelformat").or_else(|| s("video-params/pixelformat")) {
        video.push((t!("meta.pixels"), px));
    }
    if let Some(m) = s("video-params/colormatrix") {
        let range = match s("video-params/gamma").as_deref() {
            Some("pq") => "HDR10",
            Some("hlg") => "HLG",
            _ => "SDR",
        };
        video.push((t!("meta.color"), format!("{} · {range}", m.to_uppercase())));
    }
    if let Some(b) = i(&format!("{v}/demux-bitrate")) {
        video.push((t!("meta.bitrate"), bitrate(b as f64)));
    }
    if let Some(r) = player.get::<i64>("video-params/rotate").filter(|r| *r != 0) {
        video.push((t!("meta.rotation"), format!("{r}°")));
    }
    push(&mut out, icon::FILM_STRIP, t!("meta.video"), video);

    let count = player.get::<i64>("track-list/count").unwrap_or(0);
    let tracks = |kind: &str| {
        (0..count)
            .map(|n| format!("track-list/{n}"))
            .filter(|p| s(&format!("{p}/type")).as_deref() == Some(kind))
            .collect::<Vec<_>>()
    };
    let audio = tracks("audio");
    for (n, p) in audio.iter().enumerate() {
        let mut rows = Vec::new();
        if let Some(c) = codec(&s, p) {
            rows.push((t!("meta.codec"), c));
        }
        let channels = s(&format!("{p}/demux-channels")).or_else(|| {
            i(&format!("{p}/demux-channel-count")).map(|c| format!("{c} {}", t!("unit.ch")))
        });
        let rate = i(&format!("{p}/demux-samplerate"))
            .map(|r| format!("{} {}", trim_float(r as f64 / 1000.0, 1), t!("unit.khz")));
        let ch = [channels, rate]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        if !ch.is_empty() {
            rows.push((t!("meta.channels"), ch));
        }
        if let Some(b) = i(&format!("{p}/demux-bitrate")) {
            rows.push((t!("meta.bitrate"), bitrate(b as f64)));
        }
        if let Some(l) = s(&format!("{p}/lang")) {
            rows.push((t!("meta.language"), l));
        }
        let title = if audio.len() > 1 {
            format!("{} · {}", t!("meta.audio"), n + 1)
        } else {
            t!("meta.audio").to_string()
        };
        push(&mut out, icon::WAVEFORM, title, rows);
    }

    let subs = tracks("sub")
        .iter()
        .enumerate()
        .map(|(n, p)| {
            let desc = [s(&format!("{p}/codec")), s(&format!("{p}/lang"))]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
            (t!("meta.track", n = n + 1), desc)
        })
        .collect();
    push(&mut out, icon::SUBTITLES, t!("meta.subtitles"), subs);
    out
}

fn push(
    out: &mut Vec<Section>,
    icon: &'static str,
    title: impl Into<String>,
    rows: Vec<(impl Into<String>, String)>,
) {
    if !rows.is_empty() {
        out.push(Section {
            icon,
            title: title.into(),
            rows: rows.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        });
    }
}

/// "H.264 · High" from mpv's codec description and profile.
fn codec(s: &impl Fn(&str) -> Option<String>, track: &str) -> Option<String> {
    let name = s(&format!("{track}/codec-desc"))
        .map(|d| d.split(" / ").next().unwrap_or(&d).trim().to_string())
        .or_else(|| s(&format!("{track}/codec")).map(|c| c.to_uppercase()))?;
    Some(match s(&format!("{track}/codec-profile")) {
        Some(p) => format!("{name} · {p}"),
        None => name,
    })
}

fn bitrate(bps: f64) -> String {
    if bps >= 1e6 {
        format!("{:.1} {}", bps / 1e6, t!("unit.mbps"))
    } else {
        format!("{:.0} {}", bps / 1e3, t!("unit.kbps"))
    }
}

/// "16:9" for common shapes, else "1.85:1".
fn ratio(w: i64, h: i64) -> String {
    fn gcd(a: i64, b: i64) -> i64 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    let g = gcd(w, h).max(1);
    let (a, b) = (w / g, h / g);
    if a <= 32 && b <= 32 {
        format!("{a}:{b}")
    } else {
        format!("{}:1", trim_float(w as f64 / h as f64, 2))
    }
}

/// `29.970` → "29.97", `48.0` → "48".
fn trim_float(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(ratio(1920, 1080), "16:9");
        assert_eq!(ratio(1080, 1920), "9:16");
        assert_eq!(ratio(1998, 1080), "1.85:1");
        assert_eq!(trim_float(29.97003, 3), "29.97");
        assert_eq!(trim_float(48.0, 1), "48");
        assert_eq!(bitrate(47_300_000.0), "47.3 Mb/s");
        assert_eq!(bitrate(256_000.0), "256 kb/s");
    }
}
