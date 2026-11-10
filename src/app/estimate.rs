//! Rough output size, shown in the export popover before saving.

use super::*;

/// Share of the source bitrate a re-encode keeps at each [`ffmpeg::QUALITIES`] level. A guess
/// from the encoder settings: each step down is a few CRF points, and 6 points halve the bitrate.
const KEEP: [f64; 4] = [0.8, 0.5, 0.3, 0.2];

/// Guessed output bytes from a source of `bytes`. `part` is the share of the source duration
/// kept, `pixels` the output/source pixel ratio, `quality` `None` for a stream copy. Bitrate
/// grows slower than pixel count (power 0.75), and a re-encode never outgrows its source.
// shortcut: ignores audio (mute, re-encoded AAC) and keyframe overshoot; fine for a "≈" label.
pub(super) fn guess(bytes: u64, part: f64, pixels: f64, quality: Option<usize>) -> u64 {
    let share = quality.map_or(1.0, |q| {
        (KEEP[q.min(KEEP.len() - 1)] * pixels.powf(0.75)).min(1.0)
    });
    (bytes as f64 * part.clamp(0.0, 1.0) * share) as u64
}

impl App {
    /// Guessed output size at resolution `size` (index into [`ffmpeg::SIZES`]) with the other
    /// current settings. `None` without a video or while the strip scan is missing.
    pub(super) fn estimate(&self, size: usize) -> Option<u64> {
        let v = self.video.as_ref()?;
        // Retiming keeps every frame, so the size follows the source time kept, not the output's.
        let part = self.output_length()? * self.speed / v.dur.max(0.001);
        let [w, h] = self.scale(size).or(self.cropped())?;
        let px = |w: u32, h: u32| f64::from(w) * f64::from(h);
        let pixels = px(w, h) / px(v.size[0], v.size[1]).max(1.0);
        let quality = (!self.stream_copy(size)).then_some(self.quality);
        Some(guess(v.bytes, part, pixels, quality))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_follow_quality_resolution_and_length() {
        let gb4 = 4_000_000_000;
        // A stream copy of half the video is half the file.
        assert_eq!(guess(gb4, 0.5, 1.0, None), 2_000_000_000);
        // 4K to 1080p at Medium: a quarter of the pixels, ~700 MB.
        let medium_1080 = guess(gb4, 1.0, 0.25, Some(1));
        assert!(
            (650_000_000..750_000_000).contains(&medium_1080),
            "{medium_1080}"
        );
        // Lower quality is smaller; upscaling never outgrows the source.
        assert!(guess(gb4, 1.0, 0.25, Some(3)) < medium_1080);
        assert_eq!(guess(gb4, 1.0, 4.0, Some(0)), gb4);
    }
}
