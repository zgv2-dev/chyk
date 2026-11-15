//! What a join needs to know about its clips before it runs: their streams (can they be joined
//! without re-encoding?), audio, bitrate caps and sizes; and where the size estimate samples.

use super::*;

/// The stream formats that must match for a lossless join, from `ffmpeg -i`.
#[derive(Clone, PartialEq, Default, Debug)]
pub(super) struct Streams {
    /// Codec, pixel format and color, size and SAR, rate, time base and rotation of the first
    /// video stream.
    pub(super) video: Option<String>,
    pub(super) size: Option<[u32; 2]>,
    pub(super) fps: Option<f64>,
    /// The first video stream's display rotation, in degrees.
    pub(super) rotation: Option<f64>,
    /// Codec, sample rate, layout and sample format of every audio stream.
    pub(super) audio: Option<String>,
}

/// `s` split at the commas that aren't inside parentheses or brackets.
fn top_level(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut from) = (Vec::new(), 0i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[from..i].trim());
                from = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[from..].trim());
    out
}

pub(super) fn parse_streams(listing: &str) -> Streams {
    let mut st = Streams::default();
    // Side data lines follow their stream; only the first video stream's count.
    let mut in_video = false;
    for l in listing.lines().map(str::trim_start) {
        if !l.starts_with("Stream #") {
            if in_video && let Some((_, r)) = l.split_once("rotation of ") {
                st.rotation = r.split(' ').next().and_then(|r| r.parse().ok());
            }
            continue;
        }
        in_video = false;
        if let Some((_, rest)) = l.split_once(": Video: ")
            && st.video.is_none()
        {
            in_video = true;
            let p = top_level(rest);
            // The pixel format with its color range, matrix and primaries; the size with its SAR.
            let pix = p.get(1).copied().unwrap_or("");
            let piece = p.iter().copied().find(|s| {
                s.split(' ')
                    .next()
                    .and_then(|s| s.split_once('x'))
                    .is_some_and(|(w, h)| w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok())
            });
            let size = piece.and_then(|s| {
                let (w, h) = s.split(' ').next()?.split_once('x')?;
                Some([w.parse().ok()?, h.parse().ok()?])
            });
            let unit = |u: &str| {
                p.iter()
                    .find_map(|s| s.split(" (").next()?.strip_suffix(u))
                    .unwrap_or("")
            };
            st.fps = unit(" fps").parse().ok();
            st.size = size;
            st.video = Some(format!(
                "{}|{pix}|{}|{}|{}",
                p[0],
                piece.unwrap_or(""),
                unit(" fps"),
                unit(" tbn")
            ));
        } else if let Some((_, rest)) = l.split_once(": Audio: ") {
            // Every audio stream: the copy maps them all.
            let p = top_level(rest);
            let sig = p.iter().take(4).copied().collect::<Vec<_>>().join("|");
            st.audio = Some(match st.audio.take() {
                Some(a) => format!("{a};{sig}"),
                None => sig,
            });
        }
    }
    if let Some(v) = &mut st.video {
        v.push_str(&format!("|rot {}", st.rotation.unwrap_or(0.0)));
    }
    st
}

/// A join with every part's `audio` filled in, the caps for the whole (the highest clip
/// bitrates), whether it can be joined losslessly, and the clips' total bytes.
pub(super) struct Prepared {
    pub(super) job: Job,
    pub(super) source: Source,
    pub(super) same: bool,
    pub(super) bytes: u64,
}

/// `stop` ends the probing early (a cancelled size estimate); what is left is unprobed.
pub(super) fn prepare(job: &Job, j: &Join, stop: &AtomicBool) -> Prepared {
    let mut j = j.clone();
    let (mut source, mut first, mut same, mut bytes) = (Source::default(), None, true, 0);
    for p in &mut j.parts {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let listing = listing(&p.src);
        let (src, st) = (parse_source(&listing), parse_streams(&listing));
        p.audio = src.audio;
        source.audio |= src.audio;
        source.video_kbps = source.video_kbps.max(src.video_kbps);
        source.audio_kbps = source.audio_kbps.max(src.audio_kbps);
        bytes += std::fs::metadata(&p.src).map_or(0, |m| m.len());
        let rate_ok = j
            .fps
            .is_none_or(|f| st.fps.is_some_and(|s| (s - f).abs() < 0.01));
        same &= st.video.is_some() && st.size == Some(j.frame) && rate_ok && p.keep.is_none();
        same &= p.fit.keeps_frame();
        // A line break would end the concat list's `file` line early.
        same &= !p.src.to_string_lossy().contains(['\n', '\r']);
        match &first {
            None => first = Some(st),
            Some(f) => same &= *f == st,
        }
    }
    let job = Job {
        join: Some(j),
        ..job.clone()
    };
    Prepared {
        job,
        source,
        same,
        bytes,
    }
}

/// `ffmpeg -i` output for `src`, logged as a probe.
pub(super) fn listing(src: &Path) -> String {
    let mut cmd = command("ffmpeg");
    cmd.args(["-hide_banner", "-i"]).arg(src);
    log::output(log::begin(log::Kind::Probe), &mut cmd)
        .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
        .unwrap_or_default()
}

/// The join with each clip kept to the sample windows that fall in it: [`sample_windows`] over
/// the clips laid end to end, so the samples spread over the whole joined time.
pub(super) fn sample_join(j: &Join, n: usize, len: f64) -> Join {
    let mut offs = Vec::new();
    let mut virt = Vec::new();
    let mut off = 0.0;
    for p in &j.parts {
        offs.push(off);
        let keep = p.keep.clone().unwrap_or_else(|| vec![(p.start, p.end)]);
        virt.extend(keep.iter().map(|&(a, b)| (a + off, b + off)));
        off += p.end + 10.0;
    }
    let windows = super::estimate::sample_windows(&virt, n, len);
    let mut out = j.clone();
    for (i, p) in out.parts.iter_mut().enumerate() {
        let (lo, hi) = (offs[i], offs[i] + p.end + 5.0);
        let mine: Vec<_> = windows
            .iter()
            .filter(|w| w.0 >= lo && w.1 <= hi)
            .map(|&(a, b)| (a - lo, b - lo))
            .collect();
        p.keep = Some(mine);
    }
    out.parts
        .retain(|p| p.keep.as_ref().is_some_and(|k| !k.is_empty()));
    out
}
