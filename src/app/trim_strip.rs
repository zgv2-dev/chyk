use super::*;

impl App {
    /// Hatched stretches that strip will cut, and the running scan's line with its wake.
    pub(super) fn paint_strip_cuts(&mut self, ui: &Ui, p: &egui::Painter, tv: &TrimView) {
        let TrimView {
            r, strip, v0, v1, ..
        } = *tv;
        let x = |t: f64| tv.x(t);
        // Inactive stretches that strip will cut, so the selection predicts the export. While a
        // scan runs, they fill in as it goes, minus the last `pad` before the scan front: activity
        // just ahead could still keep that part.
        let found = match (&self.strip.scan, &self.strip.job) {
            (Some(scan), _) => Some((&scan.active, self.trim.1)),
            // Up to the drawn line, but only as far as the scanner has confirmed.
            (None, Some(_)) => Some((
                &self.strip.partial,
                self.trim
                    .1
                    .min(self.strip.front.shown.min(self.strip.scanned) - self.strip.pad),
            )),
            _ => None,
        };
        if self.strip.enabled
            && let Some((active, end)) = found
            && end > self.trim.0
        {
            let keep = strip::keep(active, self.strip.pad, self.trim.0, end);
            let mut cursor = self.trim.0;
            let mut gaps = Vec::new();
            for (a, b) in keep {
                if a > cursor {
                    gaps.push((cursor, a));
                }
                cursor = cursor.max(b);
            }
            if cursor < end {
                gaps.push((cursor, end));
            }
            for (a, b) in gaps {
                let a = a.max(v0);
                let b = b.min(v1);
                if b > a {
                    let gr = Rect::from_x_y_ranges(x(a)..=x(b), r.y_range());
                    hatch(p, gr);
                }
            }
        }
        // Where the running scan has read to.
        if self.strip.enabled && self.strip.job.is_some() {
            let drawn = self.strip.front.tick();
            let front = drawn.clamp(self.trim.0, self.trim.1);
            if (v0..=v1).contains(&front) && drawn > 0.0 {
                let xf = x(front);
                // A green wake behind the line, fading out to the left; its length and glow
                // breathe like a wave while the scan runs.
                let wave = (ui.input(|i| i.time) * 3.0).sin() as f32;
                let len = 56.0 + 14.0 * wave;
                let peak = 0.40 + 0.10 * wave;
                let mut wake = egui::Mesh::default();
                const STEPS: u32 = 8;
                for i in 0..=STEPS {
                    let f = i as f32 / STEPS as f32;
                    // Eased: mostly faint, brightening toward the line.
                    let c = GREEN.gamma_multiply(peak * f * f);
                    let wx = (xf - len * (1.0 - f)).max(strip.min.x);
                    wake.colored_vertex(pos2(wx, r.min.y), c);
                    wake.colored_vertex(pos2(wx, r.max.y), c);
                    if i > 0 {
                        let k = 2 * i;
                        wake.add_triangle(k - 2, k - 1, k);
                        wake.add_triangle(k - 1, k, k + 1);
                    }
                }
                p.add(wake);
                p.vline(
                    xf,
                    r.y_range(),
                    Stroke::new(1.5, GREEN.gamma_multiply(0.85)),
                );
                ui.ctx().request_repaint();
            }
        }
    }
}
