//! Files dragged in from other apps on Wayland, where winit 0.30 reports none (only its X11
//! backend does). They reach egui's raw input as if winit had, so the app handles both alike.

mod uri;
#[cfg(all(unix, not(target_os = "macos")))]
mod wayland;

#[cfg(all(unix, not(target_os = "macos")))]
pub use imp::*;

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use eframe::egui::{self, DroppedFileHandle, HoveredFile};
    use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};

    /// What the drop thread saw since the last frame.
    pub(super) struct Drops {
        pub(super) hovering: bool,
        pub(super) dropped: Vec<PathBuf>,
    }

    pub(super) static DROPS: Mutex<Drops> = Mutex::new(Drops {
        hovering: false,
        dropped: Vec::new(),
    });

    pub(super) fn with<R>(f: impl FnOnce(&mut Drops) -> R) -> R {
        f(&mut DROPS.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Listen for drops if the window is on Wayland; on X11 winit already reports them.
    pub fn start(cc: &eframe::CreationContext) {
        let Ok(RawDisplayHandle::Wayland(h)) = cc.display_handle().map(|h| h.as_raw()) else {
            return;
        };
        if let Err(e) = super::wayland::start(h.display, cc.egui_ctx.clone()) {
            eprintln!("chyk: no drag and drop on this Wayland session: {e}");
        }
    }

    /// Add this frame's drops to egui's input (`eframe::App::raw_input_hook`).
    pub fn feed(raw: &mut egui::RawInput) {
        with(|d| {
            if d.hovering {
                // Wayland names the files only once dropped.
                raw.hovered_files.push(HoveredFile::default());
            }
            raw.dropped_files.extend(
                d.dropped
                    .drain(..)
                    .map(|p| Arc::new(Dropped(p)) as DroppedFileHandle),
            );
        });
    }

    #[derive(Debug)]
    struct Dropped(PathBuf);

    impl egui::DroppedFile for Dropped {
        fn path(&self) -> &Path {
            &self.0
        }

        fn bytes(&self) -> Result<Vec<u8>, String> {
            std::fs::read(&self.0).map_err(|e| e.to_string())
        }
    }
}
