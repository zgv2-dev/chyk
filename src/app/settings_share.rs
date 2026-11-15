//! One settings window for every Chyk window. Each process listens on a loopback port; the one
//! showing the settings window writes that port next to the settings file, and a gear click in
//! another process asks it to come forward instead of opening a second window. Every process
//! watches the settings file and takes on what another one saved.

use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

const HELLO: &str = "chyk settings";
const WAIT: Duration = Duration::from_millis(400);
/// The longest line either side reads: a stray sender can't make it buffer more.
const MAX_LINE: u64 = 64;

pub(super) struct Share {
    /// This process shows the settings window and answers the others.
    owner: Arc<AtomicBool>,
    port: Option<u16>,
    /// Another window asked for the settings window.
    raise: Arc<AtomicBool>,
    /// The settings file changed on disk.
    changed: Arc<AtomicBool>,
}

fn port_file() -> Option<PathBuf> {
    crate::settings::path().map(|p| p.with_file_name("settings-window.port"))
}

impl Share {
    pub(super) fn start(ctx: &egui::Context) -> Self {
        let owner = Arc::new(AtomicBool::new(false));
        let raise = Arc::new(AtomicBool::new(false));
        let changed = Arc::new(AtomicBool::new(false));
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .ok()
            .and_then(|l| {
                let port = l.local_addr().ok()?.port();
                let (owner, raise, ctx) = (owner.clone(), raise.clone(), ctx.clone());
                std::thread::spawn(move || {
                    for stream in l.incoming().flatten() {
                        if answer(stream, owner.load(Relaxed)) {
                            raise.store(true, Relaxed);
                            ctx.request_repaint();
                        }
                    }
                });
                Some(port)
            });
        watch(changed.clone(), ctx.clone());
        Self {
            owner,
            port,
            raise,
            changed,
        }
    }

    /// Ask the process showing the settings window to bring it forward. False when none does.
    pub(super) fn ask(&self) -> bool {
        let Some(port) = port_file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| t.trim().parse::<u16>().ok())
            .filter(|&p| Some(p) != self.port)
        else {
            return false;
        };
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, WAIT) else {
            return false;
        };
        allow_focus();
        let _ = stream.set_read_timeout(Some(WAIT));
        if writeln!(stream, "{HELLO}").is_err() {
            return false;
        }
        let mut reply = String::new();
        let _ = BufReader::new(stream.take(MAX_LINE)).read_line(&mut reply);
        reply.trim() == "ok"
    }

    /// This process now shows the settings window.
    pub(super) fn claim(&self) {
        self.owner.store(true, Relaxed);
        if let (Some(port), Some(file)) = (self.port, port_file()) {
            // The settings folder appears with the first save, which may not have happened yet.
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(file, port.to_string());
        }
    }

    pub(super) fn release(&self) {
        if !self.owner.swap(false, Relaxed) {
            return;
        }
        // Only our own: another process may have taken over meanwhile.
        if let Some(file) = port_file()
            && std::fs::read_to_string(&file)
                .ok()
                .map(|t| t.trim().to_string())
                == self.port.map(|p| p.to_string())
        {
            let _ = std::fs::remove_file(file);
        }
    }

    pub(super) fn take_raise(&self) -> bool {
        self.raise.swap(false, Relaxed)
    }

    pub(super) fn take_changed(&self) -> bool {
        self.changed.swap(false, Relaxed)
    }
}

impl Drop for Share {
    fn drop(&mut self) {
        self.release();
    }
}

/// Reply to one asker: yes while this process shows the settings window. True when it asked.
fn answer(stream: TcpStream, owner: bool) -> bool {
    let _ = stream.set_read_timeout(Some(WAIT));
    let mut line = String::new();
    let mut reader = BufReader::new((&stream).take(MAX_LINE));
    if reader.read_line(&mut line).is_err() || line.trim() != HELLO {
        return false;
    }
    let _ = writeln!(&stream, "{}", if owner { "ok" } else { "no" });
    owner
}

/// Check the settings file's time once a second; flag and wake on a change. The first check
/// always flags, for a save made between this process's start and the watcher's.
fn watch(changed: Arc<AtomicBool>, ctx: egui::Context) {
    let Some(path) = crate::settings::path() else {
        return;
    };
    let stamp = move || std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let now = stamp();
            if now != last {
                last = now;
                changed.store(true, Relaxed);
                ctx.request_repaint();
            }
        }
    });
}

/// Windows lets only the foreground process raise a window: hand that right to the owner.
#[cfg(windows)]
fn allow_focus() {
    use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
    // SAFETY: no pointers; it only grants another process the right to take the foreground.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

#[cfg(not(windows))]
fn allow_focus() {}
