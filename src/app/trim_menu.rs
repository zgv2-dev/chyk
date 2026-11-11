use super::*;

const ROW: f32 = 28.0;
const WIDTH: f32 = 250.0;

/// What a menu row does, at the time that was right-clicked.
#[derive(Clone, Copy, PartialEq)]
enum Item {
    Start,
    End,
    Fit,
    Clear,
    Frame,
}

impl Item {
    fn cmd(self) -> Cmd {
        match self {
            Item::Start => Cmd::SetIn,
            Item::End => Cmd::SetOut,
            Item::Fit => Cmd::TimelineToSelection,
            Item::Clear => Cmd::ClearSelection,
            Item::Frame => Cmd::SaveFrame,
        }
    }
}

impl App {
    /// Right-click menu of the trim bar. The time under the pointer is taken on the click's own
    /// frame, since the pointer moves before a row is chosen.
    pub(super) fn trim_menu(&mut self, resp: &Response, tv: &TrimView) {
        let id = resp.id.with("menu_at");
        if let Some(p) = resp
            .secondary_clicked()
            .then(|| resp.interact_pointer_pos())
            .flatten()
        {
            resp.ctx.data_mut(|d| d.insert_temp(id, tv.t_at(p.x)));
        }
        let at: f64 = resp.ctx.data(|d| d.get_temp(id)).unwrap_or(0.0);
        let mut chosen = None;
        resp.context_menu(|ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            for item in [Item::Start, Item::End, Item::Fit, Item::Clear, Item::Frame] {
                // Task 16 wires saving a frame; until then the row stays disabled.
                let on = item != Item::Frame;
                if menu_row(ui, item.cmd(), on) {
                    chosen = Some(item);
                }
                if item == Item::End {
                    ui.add_space(4.0);
                }
            }
        });
        let dur = tv.dur;
        match chosen {
            Some(Item::Start) => self.set_in(at),
            Some(Item::End) => self.set_out(at),
            Some(Item::Fit) => self.fit_timeline_selection(dur),
            Some(Item::Clear) => self.trim = (0.0, dur),
            _ => {}
        }
    }
}

/// One menu row: label left, caps right. True when clicked.
fn menu_row(ui: &mut Ui, cmd: Cmd, enabled: bool) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (r, row) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), sense);
    let label = i18n::tr(keys::label(cmd));
    let (fill, color) = if !enabled {
        (Color32::TRANSPARENT, MUTED.gamma_multiply(0.5))
    } else if row.hovered() {
        (CARD_HI, TEXT)
    } else {
        (Color32::TRANSPARENT, MUTED)
    };
    let p = ui.painter();
    p.rect_filled(r, 6.0, fill);
    let x = vec2(9.0, 0.0);
    p.text(
        r.left_center() + x,
        Align2::LEFT_CENTER,
        label,
        mono(12.0),
        color,
    );
    let caps = keys::cmd_caps(cmd);
    let w = keys::caps_width(p, &caps);
    keys::paint_caps(p, pos2(r.right() - 9.0 - w, r.center().y - 11.0), &caps);
    if row.hovered() && enabled {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    row.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let hit = enabled && row.clicked();
    if hit {
        ui.close();
    }
    hit
}
