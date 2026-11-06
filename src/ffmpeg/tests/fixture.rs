use super::*;

/// A 4 s 640×360 H.264 + AAC clip with a keyframe every second, in a temp dir removed on drop.
pub(super) struct Fixture {
    pub(super) dir: PathBuf,
    pub(super) src: PathBuf,
}

impl Fixture {
    pub(super) fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("glint-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.mkv");
        let ok = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x360:rate=25:duration=4",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=duration=4",
                "-c:v",
                "libx264",
                "-g",
                "25",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&src)
            .status()
            .unwrap();
        assert!(ok.success());
        Self { dir, src }
    }

    /// A plain whole-clip export of the fixture to `name`, for a test to adjust.
    pub(super) fn job(&self, name: &str) -> Job {
        Job {
            src: self.src.clone(),
            dst: self.dir.join(name),
            start: 0.0,
            end: 4.0,
            crop: None,
            precise: false,
            codec: "h264".into(),
            quality: 0,
            mute: false,
            speed: 1.0,
            keep: None,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Run an export to the end: bytes written, or the error.
pub(super) fn wait(run: Running) -> Result<u64, String> {
    loop {
        match run
            .rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .expect("export timed out")
        {
            Msg::Done(r) => break r,
            Msg::Progress(_) => {}
        }
    }
}

/// `ffprobe` `entries` of `path` for the `streams` selector (like `v:0`), as CSV.
pub(super) fn probe(path: &Path, streams: &str, entries: &str) -> String {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", streams])
        .args(["-show_entries", entries, "-of", "csv=p=0"])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
