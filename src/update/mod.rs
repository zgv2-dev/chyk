//! Self-update. On launch Chyk asks GitHub for a newer release; the header button downloads it on
//! a click, and "Update now" (or quitting) swaps it in. Nothing downloads or changes without that
//! click. Design: docs/superpowers/specs/2026-11-06-updater-design.md.

mod check;
mod download;
mod paths;
mod swap;
#[cfg(test)]
mod tests;

pub use check::REPO;
pub use paths::Kind;

/// Where "Update now" leaves the session for the new copy: beside the download cache, not in it,
/// so cache cleanup can't remove it.
pub fn session_file() -> Option<PathBuf> {
    Some(paths::cache_dir()?.parent()?.join("session.txt"))
}
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc::{Receiver, channel};

/// What the header button shows.
pub enum State {
    /// Nothing to offer: checking, up to date, offline, or a build that doesn't update.
    None,
    /// A newer release. `manual`: this install updates through its package manager.
    Available { tag: String, manual: bool },
    Downloading {
        tag: String,
        size: u64,
        child: Child,
        part: PathBuf,
    },
    /// Downloaded completely: replaces this copy on "Update now" or when Chyk quits.
    Ready { tag: String, file: PathBuf },
}

pub struct Updater {
    pub state: State,
    kind: Option<Kind>,
    size: Option<u64>,
    check: Option<Receiver<Option<check::Found>>>,
}

impl Updater {
    /// Start the background check. `repaint` is called when its answer arrives.
    pub fn start(repaint: impl Fn() + Send + 'static) -> Self {
        let kind = paths::kind();
        let mut me = Self {
            state: State::None,
            kind: kind.clone(),
            size: None,
            check: None,
        };
        let Some(kind) = kind else { return me };
        swap::cleanup(&kind);
        let asset = paths::asset(&kind);
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(check::latest(asset));
            repaint();
        });
        me.check = Some(rx);
        me
    }

    /// Take the check's answer when it arrives, and notice a download finishing. `Err` carries a
    /// failed download's message for the status line.
    pub fn poll(&mut self) -> Result<(), String> {
        if let Some(found) = self.check.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.check = None;
            self.found(found);
        }
        let State::Downloading {
            tag, child, part, ..
        } = &mut self.state
        else {
            return Ok(());
        };
        let done = match child.try_wait() {
            Ok(None) => return Ok(()),
            Ok(Some(status)) => status.success(),
            Err(_) => false,
        };
        let (tag, part) = (tag.clone(), part.clone());
        let file = self.target(&tag);
        match file.filter(|f| done && std::fs::rename(&part, f).is_ok()) {
            Some(file) => {
                self.state = State::Ready { tag, file };
                Ok(())
            }
            None => {
                let _ = std::fs::remove_file(&part);
                self.state = State::Available { tag, manual: false };
                Err("download failed".into())
            }
        }
    }

    /// The check's answer: offer a newer release, or a finished download of it from the cache.
    fn found(&mut self, found: Option<check::Found>) {
        let Some(kind) = &self.kind else { return };
        let (asset, dir) = (paths::asset(kind), paths::cache_dir());
        let current = env!("CARGO_PKG_VERSION");
        let cached = asset
            .zip(dir.as_deref())
            .and_then(|(a, d)| paths::newest_cached(d, a));
        let Some(found) = found else {
            // Offline: a download from an earlier run can still be swapped in.
            if let Some((tag, file)) = cached.filter(|(t, _)| check::newer(t, current)) {
                self.state = State::Ready { tag, file };
            }
            return;
        };
        let newer = check::newer(&found.tag, current);
        let ready = cached.filter(|(t, _)| newer && *t == found.tag);
        if let Some(d) = &dir {
            paths::clean(d, ready.as_ref().map(|(_, f)| f.as_path()));
        }
        if !newer {
            return;
        }
        self.size = found.size;
        self.state = match (ready, kind) {
            (Some((tag, file)), _) => State::Ready { tag, file },
            (None, Kind::Package) => State::Available {
                tag: found.tag,
                manual: true,
            },
            // No file for this system in the release: nothing to offer.
            (None, _) if found.size.is_none() => State::None,
            (None, _) => State::Available {
                tag: found.tag,
                manual: false,
            },
        };
    }

    fn target(&self, tag: &str) -> Option<PathBuf> {
        let asset = paths::asset(self.kind.as_ref()?)?;
        Some(paths::cached_file(&paths::cache_dir()?, tag, asset))
    }

    /// Swap in the downloaded release and start it with `args`. On failure Chyk keeps running.
    pub fn update_now(&mut self, args: &[OsString]) -> Result<(), String> {
        let (State::Ready { file, .. }, Some(kind)) = (&self.state, &self.kind) else {
            return Err("nothing to update".into());
        };
        swap::swap(kind, file).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(file);
        self.state = State::None;
        swap::relaunch(kind, args).map_err(|e| e.to_string())
    }

    /// Chyk is quitting: a finished download is swapped in silently; a running one is dropped.
    pub fn on_exit(&mut self) {
        self.cancel();
        if let (State::Ready { file, .. }, Some(kind)) = (&self.state, &self.kind)
            && swap::swap(kind, file).is_ok()
        {
            let _ = std::fs::remove_file(file);
        }
    }
}
