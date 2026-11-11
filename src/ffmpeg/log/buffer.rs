//! The log itself, bounded: [`ENTRIES`] entries of [`LINES`] lines per run.

use super::*;

/// Entries kept, oldest dropped first.
const ENTRIES: usize = 200;
/// Output lines kept per run, oldest dropped first.
const LINES: usize = 200;

impl Entry {
    fn running(&self) -> bool {
        self.runs
            .iter()
            .any(|r| matches!(r.state, RunState::Running))
    }
}

pub(crate) struct Log {
    entries: VecDeque<Entry>,
    next: u64,
    generation: u64,
}

impl Log {
    pub(crate) const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            next: 0,
            generation: 0,
        }
    }

    pub(crate) fn begin(&mut self, kind: Kind) -> u64 {
        self.generation += 1;
        let id = self.next;
        self.next += 1;
        // A scrolling filmstrip keeps one entry.
        if kind == Kind::Thumbnails
            && let Some(last) = self.entries.back()
            && last.kind == Kind::Thumbnails
            && !last.running()
        {
            self.entries.pop_back();
        }
        self.entries.push_back(Entry {
            id,
            at: SystemTime::now(),
            kind,
            runs: Vec::new(),
            progress: None,
        });
        if self.entries.len() > ENTRIES {
            self.entries.pop_front();
        }
        id
    }

    /// Entry `id`, if not dropped yet. Counts as a change.
    fn entry(&mut self, id: u64) -> Option<&mut Entry> {
        self.generation += 1;
        self.entries.iter_mut().rev().find(|e| e.id == id)
    }

    /// Adds a running command to `entry`; returns its index there.
    pub(crate) fn start(&mut self, entry: u64, args: String) -> usize {
        let Some(e) = self.entry(entry) else { return 0 };
        e.runs.push(Run {
            args,
            state: RunState::Running,
            output: VecDeque::new(),
        });
        e.runs.len() - 1
    }

    pub(crate) fn line(&mut self, entry: u64, run: usize, text: &str) {
        if let Some(r) = self.entry(entry).and_then(|e| e.runs.get_mut(run)) {
            r.output.push_back(text.into());
            if r.output.len() > LINES {
                r.output.pop_front();
            }
        }
    }

    pub(crate) fn progress(&mut self, entry: u64, text: &str) {
        if let Some(e) = self.entry(entry) {
            e.progress = Some(text.into());
        }
    }

    pub(crate) fn exited(&mut self, entry: u64, run: usize, code: Option<i32>, took: Duration) {
        if let Some(r) = self.entry(entry).and_then(|e| e.runs.get_mut(run)) {
            r.state = RunState::Exited { code, took };
        }
    }

    pub(crate) fn entries(&self) -> Vec<Entry> {
        self.entries.iter().cloned().collect()
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn busy(&self) -> bool {
        self.entries.iter().any(Entry::running)
    }
}
