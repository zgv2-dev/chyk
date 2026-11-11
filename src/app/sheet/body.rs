use super::*;

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

/// The caps a row shows: its chords apart by "/", or "first – last" for a family.
fn row_caps(ui: &mut Ui, cmd: Cmd, bs: &[Binding]) {
    let mac = cfg!(target_os = "macos");
    let c = |b: &Binding| caps(b.mods, b.key, mac);
    if matches!(cmd, Cmd::CropPreset(_) | Cmd::OpenRecent(_)) && bs.len() > 1 {
        keycaps(ui, &c(&bs[bs.len() - 1]));
        ui.label(RichText::new("–").font(mono(11.0)).color(MUTED));
        keycaps(ui, &c(&bs[0]));
        return;
    }
    // Right to left, so the last chord goes first.
    for (n, b) in shown(bs).into_iter().rev().enumerate() {
        if n > 0 {
            ui.label(RichText::new("/").font(mono(11.0)).color(MUTED));
        }
        keycaps(ui, &c(b));
    }
}

/// Groups split over three columns, in order, about as many rows each.
pub(super) fn columns(rows: &[(Group, Cmd, Vec<Binding>)]) -> [Vec<Group>; 3] {
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
    pub(super) fn sheet_body(&self, ui: &mut Ui, filter: &str) {
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
