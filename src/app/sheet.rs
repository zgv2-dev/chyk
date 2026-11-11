//! The `?` sheet: every shortcut, grouped, with a filter.

use super::*;

#[cfg(test)]
mod tests;

/// One row per command name: all the bindings of a family (crop presets, recent files) share one.
/// Matches `filter` against the English or current-language name, ignoring case.
pub(super) fn sheet_rows(filter: &str) -> Vec<(Group, Cmd, Vec<Binding>)> {
    let mut rows: Vec<(Group, Cmd, Vec<Binding>)> = Vec::new();
    for b in bindings() {
        match rows.iter_mut().find(|r| label(r.1) == label(b.cmd)) {
            Some(r) => r.2.push(b),
            None => rows.push((b.group, b.cmd, vec![b])),
        }
    }
    rows.sort_by_key(|r| r.0);
    let f = filter.trim().to_lowercase();
    rows.retain(|r| {
        let l = label(r.1);
        i18n::en(l).to_lowercase().contains(&f) || i18n::tr(l).to_lowercase().contains(&f)
    });
    rows
}

fn group_label(g: Group) -> &'static str {
    i18n::tr(match g {
        Group::Playback => "group.playback",
        Group::Trim => "group.trim",
        Group::Crop => "group.crop",
        Group::Strip => "group.strip",
        Group::Edit => "group.edit",
        Group::View => "group.view",
        Group::File => "group.file",
    })
}

/// The caps a row shows: the first chord, or "first – last" for a family.
fn row_caps(ui: &mut Ui, cmd: Cmd, bs: &[Binding]) {
    let mac = cfg!(target_os = "macos");
    let c = |b: &Binding| caps(b.mods, b.key, mac);
    if matches!(cmd, Cmd::CropPreset(_) | Cmd::OpenRecent(_)) && bs.len() > 1 {
        keycaps(ui, &c(&bs[bs.len() - 1]));
        ui.label(RichText::new("–").font(mono(11.0)).color(MUTED));
        keycaps(ui, &c(&bs[0]));
    } else {
        keycaps(ui, &c(&bs[0]));
    }
}

/// Groups split over three columns, in order, about as many rows each.
fn columns(rows: &[(Group, Cmd, Vec<Binding>)]) -> [Vec<Group>; 3] {
    let mut cols: [Vec<Group>; 3] = Default::default();
    let per = (rows.len()
        + rows
            .iter()
            .map(|r| r.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len())
    .div_ceil(3);
    let (mut col, mut used) = (0, 0);
    for g in rows.iter().map(|r| r.0) {
        if cols[col].last() == Some(&g) {
            continue;
        }
        let n = rows.iter().filter(|r| r.0 == g).count() + 1;
        if used >= per && col < 2 {
            (col, used) = (col + 1, 0);
        }
        cols[col].push(g);
        used += n;
    }
    cols
}

impl App {
    pub(super) fn sheet_ui(&mut self, ctx: &egui::Context) {
        let Some(mut filter) = self.sheet.take() else {
            return;
        };
        let screen = ctx.content_rect();
        let scrim = egui::Area::new("sheet_scrim".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(r, 0.0, Color32::from_black_alpha(150));
                resp
            })
            .inner;
        let width = (screen.width() - 32.0).min(1000.0);
        let shown = egui::Area::new("sheet".into())
            .fixed_pos(pos2(screen.center().x - width / 2.0, screen.min.y + 64.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(14.0)
                    .inner_margin(egui::Margin::symmetric(18, 14))
                    .shadow(shadow())
                    .show(ui, |ui| {
                        ui.set_width(width - 36.0);
                        let id = ui.id().with("filter");
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon::KEYBOARD).font(mono(17.0)).color(GREEN));
                            ui.label(
                                RichText::new(t!("sheet.title"))
                                    .font(bold(14.0))
                                    .color(TEXT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut filter)
                                        .id(id)
                                        .hint_text(t!("sheet.filter"))
                                        .font(mono(12.0))
                                        .desired_width(220.0),
                                );
                            });
                        });
                        // The filter keeps the keyboard while the sheet is open.
                        ui.memory_mut(|m| m.request_focus(id));
                        ui.add_space(8.0);
                        self.sheet_body(ui, &filter);
                    });
            });
        ctx.move_to_top(shown.response.layer_id);
        // Clicking outside closes it, like the popovers.
        let outside = scrim.clicked()
            && ctx
                .pointer_interact_pos()
                .is_some_and(|p| !shown.response.rect.contains(p));
        if !outside {
            self.sheet = Some(filter);
        }
    }

    fn sheet_body(&self, ui: &mut Ui, filter: &str) {
        let rows = sheet_rows(filter);
        if rows.is_empty() {
            ui.label(
                RichText::new(t!("sheet.none"))
                    .font(mono(12.0))
                    .color(MUTED),
            );
            return;
        }
        let cols = columns(&rows);
        ui.columns(3, |uis| {
            for (ui, groups) in uis.iter_mut().zip(cols) {
                for g in groups {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(group_label(g).to_uppercase())
                            .font(mono(10.0))
                            .color(MUTED),
                    );
                    for (_, cmd, bs) in rows.iter().filter(|r| r.0 == g) {
                        ui.horizontal(|ui| {
                            ui.set_height(28.0);
                            ui.label(
                                RichText::new(i18n::tr(label(*cmd)))
                                    .font(mono(12.0))
                                    .color(TEXT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                row_caps(ui, *cmd, bs);
                            });
                        });
                    }
                }
            }
        });
    }
}
