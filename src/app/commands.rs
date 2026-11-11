//! Running a command from the key table.

use super::*;

#[cfg(test)]
mod tests;

/// Consumes the key events that match a binding. Bindings with more modifiers go first, so
/// ⇧I is taken before I. Shift and Alt must match exactly (egui's own matching ignores extra
/// ones), except for `?`, which keyboards report with or without Shift.
pub(super) fn pressed(ctx: &egui::Context, typing: bool, focused: bool) -> Vec<Cmd> {
    let weight = |m: Modifiers| [m.ctrl, m.alt, m.shift, m.command, m.mac_cmd].map(usize::from);
    let mut all = bindings();
    all.sort_by_key(|b| std::cmp::Reverse(weight(b.mods).iter().sum::<usize>()));
    let mut out = Vec::new();
    ctx.input_mut(|i| {
        for b in all {
            if typing && !(b.mods.command || b.mods.ctrl) {
                continue;
            }
            // A focused control in an open popover keeps these; leave them in the input.
            let own = matches!(
                b.cmd,
                Cmd::PlayPause | Cmd::FrameBack | Cmd::FrameNext | Cmd::Back5 | Cmd::Forward5
            );
            if focused && own {
                continue;
            }
            i.events.retain(|e| {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers: m,
                    ..
                } = e
                else {
                    return true;
                };
                // Shift+/ is `?` on a US layout.
                let key = if *key == Key::Slash && m.shift {
                    Key::Questionmark
                } else {
                    *key
                };
                let loose_shift = b.key == Key::Questionmark;
                let hit = key == b.key
                    && m.alt == b.mods.alt
                    && (loose_shift || m.shift == b.mods.shift)
                    && m.matches_logically(b.mods);
                if hit {
                    out.push(b.cmd);
                }
                !hit
            });
        }
    });
    out
}

impl App {
    pub(super) fn enabled(&self, cmd: Cmd) -> bool {
        use Cmd::*;
        match cmd {
            Open | Shortcuts | ToggleLog => true,
            OpenRecent(_) => self.video.is_none(),
            _ if self.video.is_none() => false,
            DeleteZone => self.strip.edit,
            EditZones => self.panel == Some(Panel::Strip),
            CropPreset(_) => self.panel == Some(Panel::Crop),
            Rescan | StripOff => self.strip.enabled,
            _ => true,
        }
    }

    pub(super) fn run(&mut self, cmd: Cmd, ctx: &egui::Context) {
        use Cmd::*;
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        match cmd {
            PlayPause => self.toggle_play(),
            FrameBack => self.player.cmd("frame-back-step", &[]),
            FrameNext => self.player.cmd("frame-step", &[]),
            Back5 => self.skip(-5.0),
            Forward5 => self.skip(5.0),
            Slower => self.nudge_speed(false),
            Faster => self.nudge_speed(true),
            SetIn => self.set_in(self.pos()),
            SetOut => self.set_out(self.pos()),
            GoIn => self.player.seek(self.trim.0),
            GoOut => self.player.seek(self.trim.1),
            ClearSelection => self.trim = (0.0, dur),
            Crop => self.toggle_crop(ctx),
            CropPreset(i) => self.set_crop_preset(i),
            RemoveCrop => self.remove_crop(ctx),
            Strip => self.toggle_strip(ctx),
            AddZone => {
                if self.panel != Some(Panel::Strip) {
                    self.toggle_strip(ctx);
                }
                self.add_zone();
            }
            EditZones => self.toggle_zone_edit(),
            DeleteZone => self.remove_selected_zone(),
            Rescan => self.start_scan(ctx),
            StripOff => self.strip_off(),
            ZoomIn => self.zoom_video(1.25, None),
            ZoomOut => self.zoom_video(0.8, None),
            ZoomFit => self.reset_video_zoom(),
            TimelineIn => self.zoom_timeline(1.5, dur),
            TimelineOut => self.zoom_timeline(1.0 / 1.5, dur),
            TimelineToSelection => self.fit_timeline_selection(dur),
            TimelineWhole => self.fit_timeline(dur),
            Open => self.open_dialog(),
            // Same as the Export button: open the settings; pressed again, save.
            Export => {
                if self.pop == Some(Pop::Export) {
                    self.pop = None;
                    self.export(ctx);
                } else {
                    self.pop = Some(Pop::Export);
                }
            }
            Info => {
                self.close_panel(ctx);
                self.toggle_pop(Pop::Info);
            }
            CloseVideo => self.request_close(ctx),
            // Filled in by the tasks that add these features.
            Shortcuts => {
                self.sheet = self.sheet.is_none().then(String::new);
                // The ? that opened it is not the first letter of the filter.
                ctx.input_mut(|i| {
                    i.events
                        .retain(|e| !matches!(e, egui::Event::Text(s) if s == "?"));
                });
            }
            Undo | Redo | SaveFrame | RevealExport | OpenRecent(_) | ToggleLog => {}
        }
    }
}
