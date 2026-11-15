//! Self-update. On launch Chyk asks GitHub for a newer release; the header button downloads it on
//! a click, and "Update now" (or quitting) swaps it in. Nothing downloads or changes without that
//! click. Network, disk and process work all run on background threads, so the UI never waits
//! (antivirus scanning makes even file-size reads slow on Windows).
//! Design: docs/superpowers/specs/2026-11-06-updater-design.md.

mod check;
mod download;
mod paths;
mod swap;
#[cfg(test)]
mod tests;

pub use check::REPO;
use download::Job;
pub use paths::Kind;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

/// Where "Update now" leaves the session for the new copy: beside the download cache, not in it,
/// so cache cleanup can't remove it.
pub fn session_file() -> Option<PathBuf> {
    Some(paths::cache_dir()?.parent()?.join("session.txt"))
}

/// What the header button shows.
pub enum State {
    /// Nothing to offer: checking, up to date, offline, or a build that doesn't update.
    None,
    /// A newer release. `manual`: this install updates through its package manager.
    Available {
        tag: String,
        manual: bool,
    },
    Downloading {
        tag: String,
        job: Job,
    },
    /// Downloaded completely: replaces this copy on "Update now" or when Chyk quits.
    Ready {
        tag: String,
        file: PathBuf,
    },
    /// "Update now" is swapping in the background; Chyk closes once the new copy has started.
    Updating {
        tag: String,
        file: PathBuf,
        done: Receiver<Result<(), String>>,
    },
}

/// What [`Updater::poll`] tells the app.
pub enum Event {
    /// A download or swap failed, with its message for the status line.
    Failed(String),
    /// The new copy is running: close this one.
    Close,
}

type Repaint = Arc<dyn Fn() + Send + Sync>;

pub struct Updater {
    pub state: State,
    kind: Option<Kind>,
    /// Size of this system's release file, from the check.
    size: u64,
    check: Option<Receiver<(State, u64)>>,
    repaint: Repaint,
}

impl Updater {
    /// Start the background check. `repaint` wakes the UI when background work has news.
    pub fn start(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let repaint: Repaint = Arc::new(repaint);
        let kind = paths::kind();
        let mut me = Self {
            state: State::None,
            kind: kind.clone(),
            size: 0,
            check: None,
            repaint: repaint.clone(),
        };
        let Some(kind) = kind else { return me };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            swap::cleanup(&kind);
            let found = check::latest(paths::asset(&kind));
            let _ = tx.send(check::decide(&kind, found));
            repaint();
        });
        me.check = Some(rx);
        me
    }

    /// Take background results: the check's answer, a finished download or swap.
    pub fn poll(&mut self) -> Option<Event> {
        if let Some((state, size)) = self.check.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.check = None;
            (self.state, self.size) = (state, size);
        }
        match &mut self.state {
            State::Downloading { tag, job } => {
                let result = job.done.try_recv().ok()?;
                let tag = std::mem::take(tag);
                match result {
                    Ok(file) => self.state = State::Ready { tag, file },
                    Err(e) => {
                        self.state = State::Available { tag, manual: false };
                        return Some(Event::Failed(e));
                    }
                }
                None
            }
            State::Updating { tag, file, done } => match done.try_recv().ok()? {
                Ok(()) => Some(Event::Close),
                Err(e) => {
                    let (tag, file) = (std::mem::take(tag), std::mem::take(file));
                    self.state = State::Ready { tag, file };
                    Some(Event::Failed(e))
                }
            },
            _ => None,
        }
    }

    /// Swap in the download and start it with `args`, in the background. [`Event::Close`] follows.
    pub fn update_now(&mut self, args: Vec<OsString>) {
        let (State::Ready { tag, file }, Some(kind)) = (&self.state, &self.kind) else {
            return;
        };
        let (tag, file, kind) = (tag.clone(), file.clone(), kind.clone());
        let (tx, done) = channel();
        let repaint = self.repaint.clone();
        let swapped = file.clone();
        std::thread::spawn(move || {
            let result = swap::swap(&kind, &swapped).and_then(|()| {
                let _ = std::fs::remove_file(&swapped);
                swap::relaunch(&kind, &args)
            });
            let _ = tx.send(result.map_err(|e| e.to_string()));
            repaint();
        });
        self.state = State::Updating { tag, file, done };
    }

    /// Chyk is quitting: a finished download is swapped in silently; a running one is dropped.
    pub fn on_exit(&mut self) {
        match std::mem::replace(&mut self.state, State::None) {
            State::Downloading { job, .. } => job.stop(),
            // Let a swap that's already running finish, so the install is never left half-done.
            State::Updating { done, .. } => drop(done.recv()),
            State::Ready { file, .. } => {
                if let Some(kind) = &self.kind
                    && swap::swap(kind, &file).is_ok()
                {
                    let _ = std::fs::remove_file(file);
                }
            }
            _ => {}
        }
    }
}
