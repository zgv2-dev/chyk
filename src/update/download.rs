//! Downloading the offered release into the cache with `curl`, as a child process the × can kill.

use super::{REPO, State, Updater, check, paths};
use std::process::Stdio;

impl Updater {
    /// Start downloading the offered release into the cache.
    pub fn download(&mut self) -> Result<(), String> {
        let State::Available { tag, manual: false } = &self.state else {
            return Ok(());
        };
        let tag = tag.clone();
        let (Some(file), Some(asset)) =
            (self.target(&tag), self.kind.as_ref().and_then(paths::asset))
        else {
            return Err("no download for this system".into());
        };
        let part = file.with_extension("part");
        std::fs::create_dir_all(file.parent().unwrap_or(&file)).map_err(|e| e.to_string())?;
        let child = check::curl()
            .args(["-fsSL", "--retry", "2", "--connect-timeout", "15", "-o"])
            .arg(&part)
            .arg(format!("{REPO}/releases/download/{tag}/{asset}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let size = self.size.unwrap_or(0);
        self.state = State::Downloading {
            tag,
            size,
            child,
            part,
        };
        Ok(())
    }

    /// Download progress, 0 to 1.
    pub fn progress(&self) -> f32 {
        let State::Downloading { size, part, .. } = &self.state else {
            return 0.0;
        };
        let got = std::fs::metadata(part).map_or(0, |m| m.len());
        if *size == 0 {
            0.0
        } else {
            (got as f64 / *size as f64).min(1.0) as f32
        }
    }

    /// Stop the download and forget the partial file.
    pub fn cancel(&mut self) {
        if let State::Downloading {
            tag, child, part, ..
        } = &mut self.state
        {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&*part);
            self.state = State::Available {
                tag: tag.clone(),
                manual: false,
            };
        }
    }
}
