//! Each settings row's text and control, and what Reset page puts back.

use super::settings_items::Item;
use super::settings_text::fmt_secs;
use super::settings_widgets::*;
use super::settings_window::{SettingsView, UpdateInfo};
use super::*;
use crate::settings::{Another, LangPref, PAD_RANGE, SKIP_RANGE, SaveTo, Sensitivity};

const LANGS: [LangPref; 3] = [LangPref::System, LangPref::En, LangPref::Uk];
const ANOTHER: [Another; 3] = [Another::Ask, Another::Replace, Another::NewWindow];
const SAVE_TO: [SaveTo; 2] = [SaveTo::VideoFolder, SaveTo::LastFolder];
const SENS: [Sensitivity; 3] = [Sensitivity::Low, Sensitivity::Normal, Sensitivity::High];
/// Skip lengths the stepper goes through, in seconds.
const SKIPS: [f64; 8] = [1.0, 2.0, 3.0, 5.0, 10.0, 15.0, 30.0, 60.0];

pub(super) fn control(ui: &mut Ui, item: Item, view: &mut SettingsView) {
    let s = &mut view.settings;
    match item {
        Item::Lang => {
            let system = match *i18n::SYSTEM {
                i18n::Lang::En => "English",
                i18n::Lang::Uk => "Українська",
            };
            let names = [
                t!("settings.lang_system", lang = system),
                "English".into(),
                "Українська".into(),
            ];
            pick_list(ui, "lang", &names, &LANGS, &mut s.lang, 250.0);
        }
        Item::Another => {
            let names = [
                t!("settings.ask"),
                t!("open.replace"),
                t!("open.new_window"),
            ]
            .map(String::from);
            seg(ui, "another", &names, &ANOTHER, &mut s.another);
        }
        Item::Format => {
            let mut names = vec![t!("settings.same_format").to_string()];
            names.extend(crate::ffmpeg::FORMATS.iter().map(|f| f.to_uppercase()));
            let values: Vec<Option<usize>> = std::iter::once(None)
                .chain((0..crate::ffmpeg::FORMATS.len()).map(Some))
                .collect();
            seg(ui, "format", &names, &values, &mut s.format);
        }
        Item::Quality => {
            let names: Vec<String> = crate::ffmpeg::QUALITIES
                .iter()
                .map(|k| i18n::tr(k).into())
                .collect();
            let values: Vec<usize> = (0..names.len()).collect();
            seg(ui, "quality", &names, &values, &mut s.quality);
        }
        Item::Size => {
            let names: Vec<String> = crate::ffmpeg::SIZES
                .iter()
                .enumerate()
                .map(|(i, (n, _))| {
                    if i == 0 {
                        t!("settings.res_original").into()
                    } else {
                        (*n).into()
                    }
                })
                .collect();
            let values: Vec<usize> = (0..names.len()).collect();
            pick_list(ui, "size", &names, &values, &mut s.size, 170.0);
        }
        Item::SaveTo => {
            let names = [t!("settings.save_video"), t!("settings.save_last")].map(String::from);
            seg(ui, "save_to", &names, &SAVE_TO, &mut s.save_to);
        }
        Item::Sensitivity => {
            let names = [
                t!("settings.sens_low"),
                t!("settings.sens_normal"),
                t!("settings.sens_high"),
            ]
            .map(String::from);
            seg(ui, "sens", &names, &SENS, &mut s.sensitivity);
        }
        Item::ShellMenu => {
            let (on, busy) = view
                .shell
                .as_ref()
                .map_or((false, true), |r| (r.on, r.busy));
            if ui
                .add_enabled_ui(!busy, |ui| switch(ui, on))
                .inner
                .clicked()
            {
                view.shell_toggle = true;
            }
        }
        Item::CheckUpdates => flip(ui, &mut s.check_updates),
        Item::Precise => flip(ui, &mut s.precise),
        Item::Mute => flip(ui, &mut s.mute),
        Item::Sound => flip(ui, &mut s.sound),
        Item::StopAtOut => flip(ui, &mut s.stop_at_out),
        Item::Hwdec => flip(ui, &mut s.hwdec),
        Item::Suffix => {
            let mut text = s.suffix.clone();
            ui.add(
                egui::TextEdit::singleline(&mut text)
                    .font(mono(12.0))
                    .desired_width(130.0),
            );
            s.suffix = crate::settings::clean_suffix(&text);
        }
        Item::Skip => {
            let at = SKIPS
                .iter()
                .position(|&v| v >= s.skip)
                .unwrap_or(SKIPS.len() - 1);
            if let Some(i) = stepper(ui, &format!("{} s", s.skip), at, SKIPS.len()) {
                s.skip = SKIPS[i].clamp(SKIP_RANGE.0, SKIP_RANGE.1);
            }
        }
        Item::Pad => {
            let steps = (s.pad * 4.0).round() as usize;
            let max = (PAD_RANGE.1 * 4.0) as usize + 1;
            if let Some(i) = stepper(ui, &format!("{} s", fmt_secs(s.pad)), steps, max) {
                s.pad = i as f64 / 4.0;
            }
        }
        Item::Version => {
            let busy = matches!(view.update, UpdateInfo::Unsupported | UpdateInfo::Checking);
            let check = ui.add_enabled_ui(!busy, |ui| {
                button(
                    ui,
                    icon::ARROWS_CLOCKWISE,
                    t!("settings.check_now"),
                    false,
                    CHIP,
                )
            });
            if check.inner.clicked() {
                view.check_now = true;
                view.update = UpdateInfo::Checking;
                // The main window starts the check on its next pass.
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
        }
        Item::Ffmpeg | Item::Mpv => {
            let v = if item == Item::Ffmpeg {
                &view.ffmpeg
            } else {
                &view.mpv
            };
            let text = v.clone().unwrap_or_else(|| "…".into());
            ui.label(RichText::new(text).font(mono(12.0)).color(MUTED));
        }
        Item::Website | Item::Source | Item::Report | Item::License => link(ui, item),
    }
}
