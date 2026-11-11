//! Every ffmpeg job Chyk runs this session, for the log window. An entry is one job (a filmstrip
//! refresh, a scan, an estimate, an export, a frame save, a probe) with each command it ran.
#![expect(dead_code, reason = "the log window reads it")]

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

mod buffer;

pub(super) use buffer::Log;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Thumbnails,
    Scan,
    Estimate,
    Export,
    Frame,
    Probe,
}

#[derive(Clone)]
pub enum RunState {
    Running,
    Exited { code: Option<i32>, took: Duration },
}

/// One ffmpeg process: its command line, state and stderr.
#[derive(Clone)]
pub struct Run {
    pub args: String,
    pub state: RunState,
    pub output: VecDeque<String>,
}

#[derive(Clone)]
pub struct Entry {
    pub id: u64,
    pub at: SystemTime,
    pub kind: Kind,
    pub runs: Vec<Run>,
    /// An export's latest `-progress` block, on one line.
    pub progress: Option<String>,
}

static LOG: Mutex<Log> = Mutex::new(Log::new());

/// Runs `f` on the log. Never across a spawn, a wait or a pipe read.
fn with<T>(f: impl FnOnce(&mut Log) -> T) -> T {
    f(&mut LOG.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Starts a job's entry; pass its id to [`spawn`].
pub fn begin(kind: Kind) -> u64 {
    with(|l| l.begin(kind))
}

/// Records `cmd` as a run of `entry`, then spawns it. Returns the child and the run's index.
pub fn spawn(entry: u64, cmd: &mut Command) -> io::Result<(Child, usize)> {
    let args = describe(cmd);
    let run = with(|l| l.start(entry, args));
    cmd.spawn().map(|c| (c, run)).inspect_err(|e| {
        with(|l| {
            l.line(entry, run, &e.to_string());
            l.exited(entry, run, None, Duration::ZERO);
        });
    })
}

/// [`Command::output`] as a logged run of `entry`: both pipes read together, stderr logged after.
pub fn output(entry: u64, cmd: &mut Command) -> io::Result<Output> {
    let at = Instant::now();
    let (child, run) = spawn(entry, cmd.stdout(Stdio::piped()).stderr(Stdio::piped()))?;
    let out = child.wait_with_output();
    with(|l| {
        if let Ok(o) = &out {
            String::from_utf8_lossy(&o.stderr)
                .lines()
                .for_each(|t| l.line(entry, run, t));
        }
        let code = out.as_ref().ok().and_then(|o| o.status.code());
        l.exited(entry, run, code, at.elapsed());
    });
    out
}

/// Logs `child`'s stderr live on a thread, which returns all of it.
pub fn stderr(entry: u64, run: usize, child: &mut Child) -> JoinHandle<String> {
    let pipe = child.stderr.take();
    std::thread::spawn(move || {
        let mut all = String::new();
        for l in pipe
            .into_iter()
            .flat_map(|p| BufReader::new(p).split(b'\n'))
        {
            let Ok(l) = l else { break };
            let l = String::from_utf8_lossy(&l);
            let l = l.trim_end_matches('\r');
            line(entry, run, l);
            all.push_str(l);
            all.push('\n');
        }
        all
    })
}

pub fn line(entry: u64, run: usize, text: &str) {
    with(|l| l.line(entry, run, text));
}

pub fn progress(entry: u64, text: &str) {
    with(|l| l.progress(entry, text));
}

pub fn exited(entry: u64, run: usize, status: Option<ExitStatus>, took: Duration) {
    with(|l| l.exited(entry, run, status.and_then(|s| s.code()), took));
}

pub fn entries() -> Vec<Entry> {
    with(|l| l.entries())
}

/// Bumped on every change, so a viewer copies [`entries`] only when it moved.
pub fn generation() -> u64 {
    with(|l| l.generation())
}

/// Whether any ffmpeg is running.
pub fn busy() -> bool {
    with(|l| l.busy())
}

/// `program args…`, arguments with spaces quoted.
pub(super) fn describe(cmd: &Command) -> String {
    std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|a| {
            let a = a.to_string_lossy();
            if a.is_empty() || a.contains(char::is_whitespace) {
                format!("\"{a}\"")
            } else {
                a.into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
