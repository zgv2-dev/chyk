use super::*;

mod args;
mod cut;
mod fixture;
mod real_file;
mod round_trip;
mod thumbs;
mod units;

const LIST: &str = " V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)\n V....D libx264              libx264 H.264\n V....D hevc_videotoolbox    VideoToolbox H.265\n";

/// A source with audio and no known bitrates: no caps.
const ANY: Source = Source {
    audio: true,
    video_kbps: None,
    audio_kbps: None,
    strict: false,
};

fn job(crop: Option<[u32; 4]>, dst: &str) -> Job {
    Job {
        src: "in.mkv".into(),
        dst: dst.into(),
        start: 1.5,
        end: 4.0,
        crop,
        precise: false,
        codec: "hevc".into(),
        quality: 0,
        mute: false,
        speed: 1.0,
        keep: None,
    }
}
