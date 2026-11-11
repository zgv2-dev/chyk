use super::*;

impl App {
    /// Start screen: a clickable drop target over faint digital rain, and the shortcuts below it.
    pub(super) fn empty(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        let p = ui.painter_at(full);
        p.rect_filled(full, 0.0, BG);

        let hot = ui.input(|i| !i.raw.hovered_files.is_empty());
        // A file the system names (not on Wayland) that isn't a video: grayed out, it won't open.
        let refused = ui.input(|i| {
            i.raw
                .hovered_files
                .first()
                .and_then(|f| f.path.as_deref())
                .is_some_and(|p| !videos::is_video(p))
        });
        let hot = hot && !refused;
        let loading = self.loading.is_some();
        // One vertical stack, centered: drop box, Recent list (when there are files), legend.
        let recent_h = recent_store::height(self.recent_shown().len());
        // 16:9 when there is room; on short windows the box gives up height so the legend fits.
        let w = (full.width() - 120.0).clamp(320.0, 560.0);
        let h = (w * 9.0 / 16.0)
            .min(full.height() - 40.0 - STACK_GAP - recent_h - LEGEND_H)
            .max(200.0);
        let block = h + STACK_GAP + recent_h + LEGEND_H;
        let top = full.center().y - block / 2.0;
        let r = Rect::from_min_size(pos2(full.center().x - w / 2.0, top), vec2(w, h));
        let legend_top = r.max.y + STACK_GAP + recent_h;
        // The rain fades out toward the drop box and the legend.
        let content = Rect::from_min_max(r.min, pos2(r.max.x, legend_top + LEGEND_H));
        let time = ui.input(|i| i.time);
        rain(&p, full, content, time);
        // ~30 fps is plenty for falling glyphs, and only while the start screen shows.
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        // A file dragged over it, or the pointer: light the target up, easing in and out.
        let lit = hot || (!loading && ui.rect_contains_pointer(r));
        let k = ui
            .ctx()
            .animate_bool_with_time(ui.id().with("drop_glow"), lit, 0.2);
        drop_box::drop_box(ui, &p, r, (hot, refused), loading, k);
        let click = ui.interact(r, ui.id().with("drop"), Sense::click());
        if click.clicked() && !loading {
            self.open_dialog();
        }
        if click.hovered() && !loading {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        click.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t!("open.drop"))
        });
        if !loading && !hot && !refused {
            let caps = keys::cmd_caps(Cmd::Open);
            let label = t!("cmd.open");
            let bw = open_button_width(ui.painter(), label, &caps);
            let at = Rect::from_min_size(
                pos2(r.center().x - bw / 2.0, drop_box::button_top(r)),
                vec2(bw, MODE.height),
            );
            let opened = ui.put(at, |ui: &mut Ui| {
                button_caps(ui, icon::FOLDER_OPEN, label, true, MODE, &caps)
            });
            if opened.clicked() {
                self.open_dialog();
            }
        }
        if recent_h > 0.0 {
            self.recent_list(ui, &p, r, r.max.y + STACK_GAP);
        }
        legend(&p, legend_top, full.width() - 40.0, full.center().x);
    }
}

/// Gap between the drop box and what follows it.
pub(super) const STACK_GAP: f32 = 26.0;

fn open_button_width(p: &egui::Painter, label: &str, caps: &[String]) -> f32 {
    let text = p
        .layout_no_wrap(label.into(), mono(MODE.text), TEXT)
        .size()
        .x;
    MODE.icon + MODE.gap + text + 8.0 + keys::caps_width(p, caps) + MODE.pad * 2.0
}
