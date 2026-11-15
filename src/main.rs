#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
#[cfg(bundled)]
mod bundle;
mod chrome;
mod crop;
#[cfg(any(all(unix, not(target_os = "macos")), all(test, unix)))]
mod dnd;
mod ffmpeg;
mod i18n;
mod meta;
mod notify;
mod player;
mod portable;
mod reveal;
mod settings;
mod setup;
mod shell_menu;
mod strip;
mod update;
mod zoom;

use eframe::egui;

/// Wakes the main window from a background thread. `request_repaint` would wake whichever window
/// is mid-pass, which may be the ffmpeg log, and the main window would miss the update.
pub fn wake(ctx: &egui::Context) {
    ctx.request_repaint_of(egui::ViewportId::ROOT);
}

/// Loads libmpv. The portable build unpacks it first (and ffmpeg) into `data`, with a small
/// progress window, then starts itself again: one process gets one window loop. The installed
/// copy has them beside it; setup (the installer anywhere else) needs neither.
#[cfg(bundled)]
fn libraries() -> std::io::Result<()> {
    if portable::ON {
        if !bundle::ready() {
            app::run_unpack()?;
            std::process::Command::new(std::env::current_exe()?)
                .args(std::env::args_os().skip(1))
                .spawn()?;
            std::process::exit(0);
        }
        bundle::clean_old_builds();
        return bundle::load();
    }
    if bundle::ready() {
        return bundle::load();
    }
    Ok(())
}

fn main() -> eframe::Result {
    // Under Wayland, egui-winit still opens an X11 clipboard via $DISPLAY, which blocks forever
    // when Xwayland doesn't answer. Wayland has its own clipboard, so don't touch X11 at all.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        // SAFETY: first thing in main, before any other thread exists.
        unsafe { std::env::remove_var("DISPLAY") };
    }
    let settings = settings::Settings::load();
    i18n::set(settings.lang.lang());
    // Debug builds: `--unpack-preview` shows the portable build's unpack window with fake work.
    #[cfg(debug_assertions)]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--unpack-preview")
    {
        let _ = app::run_unpack_with(|progress| {
            for i in 0..=100 {
                progress(i as f32 / 100.0);
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
            Ok(())
        });
        return Ok(());
    }
    // The updater runs a newer setup with `--update <folder>`: no window, just the files.
    if let Some(code) = setup::update_from_args() {
        std::process::exit(code);
    }
    // Setup and uninstall first, before any library is loaded: the installer may run from a
    // folder others can write to (Downloads), elevated.
    if let Some(mode) = setup::mode() {
        return app::run_setup(mode);
    }
    #[cfg(bundled)]
    if let Err(e) = libraries() {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Chyk")
            .set_description(i18n::t!("error.unpack", error = e))
            .show();
        std::process::exit(1);
    }
    // After loading: the Windows 11 menu's package registers from the libraries' folder.
    if let Some(code) = shell_menu::from_args() {
        std::process::exit(code);
    }
    shell_menu::refresh();
    setup::refresh();
    let (file, restore) = app::Session::from_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Chyk")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/chyk.png"))
                    .expect("assets/chyk.png is a valid PNG"),
            )
            .with_inner_size([1040.0, 620.0])
            .with_min_inner_size([980.0, 460.0])
            // On macOS the content view extends under the titlebar, so the native traffic
            // lights sit inside the branded header.
            .with_fullsize_content_view(cfg!(target_os = "macos"))
            .with_titlebar_shown(!cfg!(target_os = "macos"))
            .with_title_shown(!cfg!(target_os = "macos"))
            // Elsewhere winit can't put native buttons in the content, so Chyk draws its own.
            .with_decorations(!chrome::CUSTOM)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Chyk",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, file, restore, settings)?))),
    )
}
