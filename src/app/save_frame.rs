use super::*;

impl App {
    /// Ask where, then save the frame at `t` (cropped like an export) on a thread.
    pub(super) fn save_frame(&mut self, t: f64, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        if self.frame_job.is_some() {
            return;
        }
        let stem = v.path.file_stem().unwrap_or_default().to_string_lossy();
        let time = fmt_time(t).replace([':', '.'], "-");
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(format!("{stem}-{time}.png"))
            .add_filter("PNG", &["png"]);
        if let Some(dir) = v.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut dst) = dialog.save_file() else {
            return;
        };
        if dst.extension().is_none() {
            dst.set_extension("png");
        }
        if std::fs::canonicalize(&dst).ok() == std::fs::canonicalize(&v.path).ok() {
            self.status = Some((t!("status.same_file").into(), true));
            return;
        }
        let [fw, fh] = v.size;
        let crop = self
            .crop
            .map(|c| crop::to_pixels(c, fw, fh))
            .filter(|&c| c != [fw, fh, 0, 0]);
        let (src, ctx) = (v.path.clone(), ctx.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let out = dst.clone();
        std::thread::spawn(move || {
            let _ = tx.send(ffmpeg::save_frame(&src, t, crop, &out));
            ctx.request_repaint();
        });
        self.frame_job = Some((rx, dst));
    }

    pub(super) fn poll_frame(&mut self) {
        let Some((rx, dst)) = &self.frame_job else {
            return;
        };
        let Ok(res) = rx.try_recv() else { return };
        match res {
            Ok(()) => {
                let msg: String = t!("status.saved").into();
                self.last_export = Some(dst.clone());
                self.saved_toast = Some(msg.clone());
                self.status = Some((msg, false));
            }
            Err(e) => self.status = Some((t!("status.export_failed", error = e), true)),
        }
        self.frame_job = None;
    }
}
