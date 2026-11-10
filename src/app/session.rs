//! The open video and its edits as a small `key = value` file, so "Update now" can reopen exactly
//! where the user was. Written just before the swap, read once by the new copy (`--restore`).

use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Session {
    pub(crate) path: PathBuf,
    trim: (f64, f64),
    crop: Option<Rect>,
    preset: usize,
    speed: f64,
    strip: bool,
    pad: f64,
    zones: Vec<Rect>,
    format: usize,
    quality: usize,
    size: usize,
    precise: bool,
    mute: bool,
    pos: f64,
    /// The edits had been exported, so closing shouldn't warn about them.
    exported: bool,
}

fn rect(r: Rect) -> String {
    format!("{} {} {} {}", r.min.x, r.min.y, r.max.x, r.max.y)
}

impl Session {
    /// The file to open and the session to restore from the command line: `chyk <file>`, or
    /// `chyk --restore <session file>` after "Update now" (the file is read once, then deleted).
    pub(crate) fn from_args() -> (Option<PathBuf>, Option<Self>) {
        let mut args = std::env::args_os().skip(1);
        match args.next() {
            Some(flag) if flag == "--restore" => {
                let file = args.next().map(PathBuf::from);
                let text = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
                if let Some(f) = &file {
                    let _ = std::fs::remove_file(f);
                }
                let session = text.as_deref().and_then(Self::parse);
                (session.as_ref().map(|s| s.path.clone()), session)
            }
            other => (other.map(PathBuf::from), None),
        }
    }

    // shortcut: the path is stored as UTF-8 text; a non-UTF-8 path (rare, Linux only) won't reopen.
    pub(crate) fn to_text(&self) -> String {
        let flag = |b: bool| u8::from(b);
        let mut s = format!("path = {}\n", self.path.to_string_lossy());
        s += &format!("trim = {} {}\n", self.trim.0, self.trim.1);
        if let Some(c) = self.crop {
            s += &format!("crop = {}\n", rect(c));
        }
        s += &format!("preset = {}\nspeed = {}\n", self.preset, self.speed);
        s += &format!("strip = {}\npad = {}\n", flag(self.strip), self.pad);
        for z in &self.zones {
            s += &format!("zone = {}\n", rect(*z));
        }
        s += &format!("format = {}\nquality = {}\n", self.format, self.quality);
        s += &format!(
            "size = {}\nprecise = {}\nmute = {}\n",
            self.size,
            flag(self.precise),
            flag(self.mute)
        );
        s += &format!("pos = {}\nexported = {}\n", self.pos, flag(self.exported));
        s
    }

    /// Parse [`Session::to_text`]. Unknown or malformed lines are skipped; no `path` is `None`.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let mut s = Self {
            speed: 1.0,
            pad: 0.5,
            ..Default::default()
        };
        let mut path = None;
        for line in text.lines() {
            let Some((key, value)) = line.split_once(" = ") else {
                continue;
            };
            let nums: Vec<f64> = value.split(' ').filter_map(|v| v.parse().ok()).collect();
            let rect = || {
                (nums.len() == 4).then(|| {
                    Rect::from_min_max(
                        pos2(nums[0] as f32, nums[1] as f32),
                        pos2(nums[2] as f32, nums[3] as f32),
                    )
                })
            };
            let one = nums.first().copied();
            match key {
                "path" => path = Some(PathBuf::from(value)),
                "trim" if nums.len() == 2 => s.trim = (nums[0], nums[1]),
                "crop" => s.crop = rect(),
                "zone" => s.zones.extend(rect()),
                "preset" => s.preset = one.map_or(0, |v| v as usize),
                "speed" => s.speed = one.unwrap_or(1.0),
                "pad" => s.pad = one.unwrap_or(0.5),
                "format" => s.format = one.map_or(0, |v| v as usize),
                "quality" => s.quality = one.map_or(0, |v| v as usize),
                "size" => s.size = one.map_or(0, |v| v as usize),
                "pos" => s.pos = one.unwrap_or(0.0),
                "strip" => s.strip = one == Some(1.0),
                "precise" => s.precise = one == Some(1.0),
                "mute" => s.mute = one == Some(1.0),
                "exported" => s.exported = one == Some(1.0),
                _ => {}
            }
        }
        s.path = path?;
        Some(s)
    }
}

impl App {
    /// The open video and its edits, or `None` on the start screen.
    pub(super) fn session(&self) -> Option<Session> {
        let v = self.video.as_ref()?;
        Some(Session {
            // The new copy starts in another working directory, so a relative path would break.
            path: std::path::absolute(&v.path).unwrap_or_else(|_| v.path.clone()),
            trim: self.trim,
            crop: self.crop,
            preset: self.preset,
            speed: self.speed,
            strip: self.strip.enabled,
            pad: self.strip.pad,
            zones: self.strip.zones.clone(),
            format: self.format,
            quality: self.quality,
            size: self.size,
            precise: self.precise,
            mute: self.mute,
            pos: self.pos(),
            exported: !self.unsaved(),
        })
    }

    /// Put a restored session back once its video has loaded.
    pub(super) fn apply_session(&mut self, s: Session, ctx: &egui::Context) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        self.trim = (s.trim.0.clamp(0.0, dur), s.trim.1.clamp(0.0, dur));
        if self.trim.1 <= self.trim.0 {
            self.trim = (0.0, dur);
        }
        self.crop = s.crop;
        self.preset = s.preset.min(crop::PRESETS.len() - 1);
        self.set_speed(s.speed);
        self.strip.enabled = s.strip;
        self.strip.pad = s.pad;
        self.strip.zones = s.zones;
        self.format = s.format.min(ffmpeg::FORMATS.len() - 1);
        self.quality = s.quality.min(ffmpeg::QUALITIES.len() - 1);
        self.size = s.size.min(ffmpeg::SIZES.len() - 1);
        self.precise = s.precise;
        self.mute = s.mute;
        self.crop_applied(ctx);
        if self.strip.enabled {
            self.ensure_scan(ctx);
        }
        self.player.seek(s.pos.clamp(0.0, dur));
        if !s.exported {
            // Still unexported: leave `saved` at the untouched state so closing warns as before.
            return;
        }
        self.saved = Some(self.edits());
    }
}

#[cfg(test)]
mod tests;
