use super::*;

/// Active intervals expanded by `pad` seconds on both sides, merged, and clamped to `[lo, hi]`.
/// `active` must be sorted (the scanner produces it that way).
pub fn keep(active: &[(f64, f64)], pad: f64, lo: f64, hi: f64) -> Vec<(f64, f64)> {
    let pad = pad.max(0.0);
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in active {
        let a = (a - pad).max(lo);
        let b = (b + pad).min(hi);
        if b <= a {
            continue;
        }
        match out.last_mut() {
            Some(last) if a <= last.1 + 1e-6 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Pixel size used for comparisons: about `SAMPLE_AREA` pixels at display `aspect`, but no larger
/// than the source's own `[w, h]` pixels (0 = unknown), even dimensions.
pub(super) fn sample_size(aspect: f32, source: [usize; 2]) -> (usize, usize) {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
    };
    let (w, h) = ((SAMPLE_AREA * aspect).sqrt(), (SAMPLE_AREA / aspect).sqrt());
    let fit = |have: usize, want: f32| if have == 0 { 1.0 } else { have as f32 / want };
    let s = fit(source[0], w).min(fit(source[1], h)).min(1.0);
    let even = |v: f32| (v.round() as usize).max(2) & !1;
    (even(w * s), even(h * s))
}

/// Flat pixel indices outside every ignore zone.
pub(super) fn compared_pixels(w: usize, h: usize, zones: &[Rect]) -> Vec<usize> {
    let mut mask = vec![false; w * h];
    for z in zones {
        let x0 = (z.min.x.clamp(0.0, 1.0) * w as f32).floor() as usize;
        let y0 = (z.min.y.clamp(0.0, 1.0) * h as f32).floor() as usize;
        let x1 = ((z.max.x.clamp(0.0, 1.0) * w as f32).ceil() as usize).min(w);
        let y1 = ((z.max.y.clamp(0.0, 1.0) * h as f32).ceil() as usize).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                mask[y * w + x] = true;
            }
        }
    }
    (0..w * h).filter(|&i| !mask[i]).collect()
}

pub(super) fn changed_pixels(prev: &[u8], cur: &[u8], pixels: &[usize]) -> usize {
    pixels
        .iter()
        .filter(|&&i| prev[i].abs_diff(cur[i]) > DIFF)
        .count()
}
