//! The note under the export popover's Quality row.

use super::*;

/// Short encoder name for the quality note: "VideoToolbox", "x264".
fn encoder_label(enc: &str) -> String {
    let name = match enc.rsplit('_').next().unwrap_or(enc) {
        "videotoolbox" => "VideoToolbox",
        "nvenc" => "NVENC",
        "qsv" => "QSV",
        "amf" => "AMF",
        "mf" => "Media Foundation",
        "libx264" => "x264",
        "libx265" => "x265",
        "libopenh264" => "OpenH264",
        other => other,
    };
    name.into()
}

impl App {
    /// "{hint} · {codec}, {encoder}" under the quality row; a stream copy names no encoder.
    pub(super) fn quality_note(&self) -> String {
        let hint = match self.quality {
            0 => t!("quality.high_hint"),
            1 => t!("quality.medium_hint"),
            2 => t!("quality.low_hint"),
            _ => t!("quality.compress_hint"),
        };
        let Some(v) = &self.video else {
            return hint.into();
        };
        if self.stream_copy(self.size) {
            return format!("{hint} · {}", t!("export.copy"));
        }
        match ffmpeg::first_encoder(&v.codec, self.quality == ffmpeg::COMPRESS) {
            Some(e) => {
                let codec = if e.starts_with("hevc") || e == "libx265" {
                    "HEVC"
                } else {
                    "H.264"
                };
                format!("{hint} · {codec}, {}", encoder_label(e))
            }
            None => hint.into(),
        }
    }
}
