//! Each settings row's text and control, and what Reset page puts back.

use super::settings_items::Item;
use super::settings_text::fmt_secs;
use super::settings_widgets::*;
use super::settings_window::{SettingsView, Step, UpdateInfo};
use super::*;
use crate::settings::{Another, LangPref, PAD_RANGE, SKIP_RANGE, SaveTo, Sensitivity, UiSize};

const LANGS: [LangPref; 3] = [LangPref::System, LangPref::En, LangPref::Uk];
const UI_SIZES: [UiSize; 4] = [UiSize::Auto, UiSize::P100, UiSize::P125, UiSize::P150];
const ANOTHER: [Another; 3] = [Another::Ask, Another::Replace, Another::NewWindow];
const SAVE_TO: [SaveTo; 2] = [SaveTo::VideoFolder, SaveTo::LastFolder];
const SENS: [Sensitivity; 3] = [Sensitivity::Low, Sensitivity::Normal, Sensitivity::High];
/// Skip lengths the stepper goes through, in seconds.
const SKIPS: [f64; 8] = [1.0, 2.0, 3.0, 5.0, 10.0, 15.0, 30.0, 60.0];

/// Draws the row's control; a switch is named by the row's title.
pub(super) fn control(ui: &mut Ui, item: Item, view: &mut SettingsView) {
    let name = super::settings_text::label(item, view).0;
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
        Item::UiSize => {
            let names = [
                t!("settings.ui_size_auto").into(),
                "100%".into(),
                "125%".into(),
                "150%".into(),
            ];
            seg(ui, "ui_size", &names, &UI_SIZES, &mut s.ui_size);
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
            let resp = ui.add_enabled_ui(!busy, |ui| switch(ui, on, &name)).inner;
            if resp.clicked() {
                view.shell_toggle = true;
                // The main window runs the toggle on its next pass.
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
        }
        Item::CheckUpdates => flip(ui, &mut s.check_updates, &name),
        Item::Precise => flip(ui, &mut s.precise, &name),
        Item::Mute => flip(ui, &mut s.mute, &name),
        Item::Sound => flip(ui, &mut s.sound, &name),
        Item::StopAtOut => flip(ui, &mut s.stop_at_out, &name),
        Item::Hwdec => flip(ui, &mut s.hwdec, &name),
        Item::MutePreview => flip(ui, &mut s.mute_preview, &name),
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
            if let UpdateInfo::Available(version, step) = &view.update {
                let (version, step) = (version.clone(), *step);
                return update_control(ui, view, &version, step);
            }
            let busy = matches!(view.update, UpdateInfo::Unsupported | UpdateInfo::Checking);
            let latest = view.update == UpdateInfo::Latest;
            if let Some((start, from)) = view.cheer
                && !confetti(ui, start, from)
            {
                view.cheer = None;
            }
            if view.won && latest {
                // Nothing to check again until the page changes or the window reopens.
                let trophy = RichText::new(icon::TROPHY).font(mono(18.0)).color(GREEN);
                ui.add_space(10.0);
                ui.label(trophy).on_hover_text(t!("settings.up_to_date"));
                return;
            }
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
                (view.check_now, view.asked) = (true, true);
                view.update = UpdateInfo::Checking;
                // The main window starts the check on its next pass.
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            } else if view.asked && view.update != UpdateInfo::Checking {
                view.asked = false;
                view.won = latest;
                if latest {
                    view.cheer = Some((ui.input(|i| i.time), check.inner.rect.center()));
                }
            }
        }
        Item::Ffmpeg | Item::Mpv => {
            let v = if item == Item::Ffmpeg {
                &view.ffmpeg
            } else {
                &view.mpv
            };
            let text = v.clone().unwrap_or_else(|| "…".into());
            // Right to left: the version at the edge, the project's site before it.
            ui.label(RichText::new(text).font(mono(12.0)).color(MUTED));
            ui.add_space(4.0);
            link(ui, item);
        }
        Item::Website | Item::Source | Item::Report | Item::License => link(ui, item),
    }
}

/// The Version row's control once a newer release is out: what the header's update button does,
/// in one click. Right to left, like every row's control.
fn update_control(ui: &mut Ui, view: &mut SettingsView, version: &str, step: Step) {
    let tag = format!("v{version}");
    let (glyph, label, tip) = match step {
        Step::Downloading(pct) => {
            let cancel = button(ui, icon::X, "", false, CHIP);
            if cancel.on_hover_text(t!("tip.update_cancel")).clicked() {
                view.update_cancel = true;
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
            ui.label(
                RichText::new(format!("{pct}%"))
                    .font(mono(11.0))
                    .color(MUTED),
            );
            let (bar, _) = ui.allocate_exact_size(vec2(90.0, 4.0), Sense::hover());
            ui.painter().rect_filled(bar, 2.0, LINE);
            progress_fill(ui.painter(), bar, f32::from(pct) / 100.0, 2.0, GREEN);
            return;
        }
        Step::Page => (
            icon::ARROW_UP_RIGHT,
            t!("settings.update_page").to_string(),
            t!("tip.update_manual", tag = tag).to_string(),
        ),
        Step::Offered => (
            icon::DOWNLOAD_SIMPLE,
            t!("settings.update_install").to_string(),
            t!("tip.update_install", tag = tag).to_string(),
        ),
        Step::Ready => (
            icon::ARROW_CLOCKWISE,
            t!("update.now").to_string(),
            t!("tip.update_now", tag = tag).to_string(),
        ),
        Step::Waiting => (
            icon::ARROW_CLOCKWISE,
            t!("update.now").to_string(),
            t!("tip.update_wait").to_string(),
        ),
        Step::Updating => (
            icon::ARROW_CLOCKWISE,
            t!("update.updating").to_string(),
            t!("update.updating").to_string(),
        ),
    };
    let enabled = !matches!(step, Step::Waiting | Step::Updating);
    let resp = ui.add_enabled_ui(enabled, |ui| button(ui, glyph, &label, enabled, CHIP));
    let resp = resp.inner.on_hover_text(tip);
    if resp.clicked() && enabled {
        view.update_click = true;
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    if step == Step::Updating {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}
