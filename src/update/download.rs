//! Downloading the offered release into the cache. A thread owns `curl` and the partial file; the
//! UI only reads the byte count it publishes and can ask it to stop.

use super::{REPO, State, Updater, check, paths};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::thread::JoinHandle;
use std::time::Duration;

pub struct Job {
    got: Arc<AtomicU64>,
    size: u64,
    stop: Arc<AtomicBool>,
    /// The finished file, or why it failed.
    pub(super) done: Receiver<Result<PathBuf, String>>,
    thread: Option<JoinHandle<()>>,
}

impl Job {
    /// Stop curl, delete the partial file and wait for the thread (a fraction of a second).
    pub(super) fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Updater {
    /// Start downloading the offered release into the cache.
    pub fn download(&mut self) {
        let State::Available { tag, manual: false } = &self.state else {
            return;
        };
        let tag = tag.clone();
        let asset = self.kind.as_ref().and_then(paths::asset);
        let (Some(asset), Some(dir)) = (asset, paths::cache_dir()) else {
            return;
        };
        let file = paths::cached_file(&dir, &tag, asset);
        let url = format!("{REPO}/releases/download/{tag}/{asset}");
        let (got, stop) = (
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(false)),
        );
        let (tx, done) = channel();
        let repaint = self.repaint.clone();
        let (g, s) = (got.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let result = fetch(&url, &file, &g, &s, &*repaint);
            if !s.load(Ordering::Relaxed) {
                let _ = tx.send(result);
                repaint();
            }
        });
        let job = Job {
            got,
            size: self.size,
            stop,
            done,
            thread: Some(thread),
        };
        self.state = State::Downloading { tag, job };
    }

    /// Download progress, 0 to 1.
    pub fn progress(&self) -> f32 {
        let State::Downloading { job, .. } = &self.state else {
            return 0.0;
        };
        if job.size == 0 {
            return 0.0;
        }
        (job.got.load(Ordering::Relaxed) as f64 / job.size as f64).min(1.0) as f32
    }

    /// Stop the download and forget the partial file. The thread finishes cleaning up on its own.
    pub fn cancel(&mut self) {
        if let State::Downloading { tag, job } = &mut self.state {
            job.stop.store(true, Ordering::Relaxed);
            self.state = State::Available {
                tag: std::mem::take(tag),
                manual: false,
            };
        }
    }
}

/// Run curl into `<file>.part`, publishing its size, then rename it to `file` when complete.
fn fetch(
    url: &str,
    file: &Path,
    got: &AtomicU64,
    stop: &AtomicBool,
    repaint: &dyn Fn(),
) -> Result<PathBuf, String> {
    // A fresh name per attempt, so a cancelled attempt still cleaning up can't touch this one.
    static ATTEMPT: AtomicU32 = AtomicU32::new(0);
    let part = file.with_extension(format!("{}.part", ATTEMPT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(file.parent().unwrap_or(file)).map_err(|e| e.to_string())?;
    let mut child = check::curl()
        .args(["-fsSL", "--retry", "2", "--connect-timeout", "15", "-o"])
        .arg(&part)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let ok = loop {
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break false;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) => {}
            Err(_) => break false,
        }
        got.store(
            std::fs::metadata(&part).map_or(0, |m| m.len()),
            Ordering::Relaxed,
        );
        repaint();
        std::thread::sleep(Duration::from_millis(200));
    };
    if ok && std::fs::rename(&part, file).is_ok() {
        return Ok(file.to_path_buf());
    }
    let _ = std::fs::remove_file(&part);
    Err("download failed".into())
}
