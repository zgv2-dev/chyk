#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
#[cfg(bundled)]
mod bundle;
mod chrome;
mod crop;
mod ffmpeg;
mod i18n;
mod meta;
mod notify;
mod player;
mod strip;
mod zoom;

use eframe::egui;

fn main() -> eframe::Result {
    #[cfg(bundled)]
    if let Err(e) = bundle::unpack() {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Chyk")
            .set_description(i18n::t!("error.unpack", error = e))
            .show();
        std::process::exit(1);
    }
    let file = std::env::args_os().nth(1).map(std::path::PathBuf::from);
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
        Box::new(|cc| Ok(Box::new(app::App::new(cc, file)?))),
    )
}
