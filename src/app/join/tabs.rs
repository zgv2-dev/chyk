//! Where the clip tabs go: over each clip's place on the timeline, but never narrower than a
//! badge, so a clip far shorter than the rest still has a tab to pick and remove it.

use super::*;

/// Tab spans for a join's clips on the timeline `tv`.
pub(super) fn reel_tabs(r: &reel::Reel, tv: &TrimView) -> Vec<Option<(f32, f32)>> {
    let (lo, hi) = (tv.strip.min.x, tv.strip.max.x);
    let spans: Vec<(f32, f32)> = (0..r.clips.len())
        .map(|i| r.bounds(i))
        .map(|(a, b)| (tv.x(a).max(lo), tv.x(b).min(hi)))
        .collect();
    tab_spans(&spans, lo, hi)
}

/// The narrowest a tab gets (points): its number badge with a little room.
pub(super) const MIN_TAB: f32 = 30.0;

/// Tab spans `(x0, x1)` from the clips' spans on screen, already clipped to `[lo, hi]`. A clip
/// out of view (`x1 <= x0`) gets `None`; a narrow one widens into its neighbors.
pub(super) fn tab_spans(clips: &[(f32, f32)], lo: f32, hi: f32) -> Vec<Option<(f32, f32)>> {
    let shown: Vec<usize> = (0..clips.len())
        .filter(|&i| clips[i].1 > clips[i].0)
        .collect();
    let mut spans: Vec<(f32, f32)> = shown.iter().map(|&i| clips[i]).collect();
    // Left to right, each tab at least MIN_TAB wide and starting where the last one ends; then
    // right to left, the same, so the last tabs fit inside `hi` too.
    let mut edge = lo;
    for s in &mut spans {
        s.0 = s.0.max(edge);
        s.1 = s.1.max(s.0 + MIN_TAB);
        edge = s.1;
    }
    let mut edge = hi;
    for s in spans.iter_mut().rev() {
        s.1 = s.1.min(edge);
        s.0 = s.0.min(s.1 - MIN_TAB).max(lo);
        edge = s.0;
    }
    let mut out = vec![None; clips.len()];
    for (k, &i) in shown.iter().enumerate() {
        out[i] = Some(spans[k]).filter(|s| s.1 - s.0 >= 6.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2 s clip before a 5 h one gets a full badge-wide tab; the long one starts after it.
    #[test]
    fn a_tiny_clip_keeps_a_tab() {
        let s = tab_spans(&[(0.0, 0.1), (0.1, 1000.0)], 0.0, 1000.0);
        assert_eq!(s[0], Some((0.0, MIN_TAB)));
        assert_eq!(s[1], Some((MIN_TAB, 1000.0)));
    }

    #[test]
    fn a_tiny_last_clip_pushes_back_into_the_one_before() {
        let s = tab_spans(&[(0.0, 999.9), (999.9, 1000.0)], 0.0, 1000.0);
        assert_eq!(s[1], Some((1000.0 - MIN_TAB, 1000.0)));
        assert_eq!(s[0], Some((0.0, 1000.0 - MIN_TAB)));
    }

    #[test]
    fn clips_out_of_view_have_no_tab() {
        let s = tab_spans(&[(0.0, 0.0), (0.0, 500.0), (500.0, 500.0)], 0.0, 500.0);
        assert_eq!(s, vec![None, Some((0.0, 500.0)), None]);
    }
}
