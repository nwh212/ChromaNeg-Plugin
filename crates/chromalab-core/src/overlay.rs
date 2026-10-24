//! The border buffer preview.

use serde_json::Value;

/// The border buffer preview ("Show border buffer"): dims what lies outside the measured area in a
/// safelight red. Positions come from the host's `_image` (the band's place in the frame); the
/// measured area is the frame inset by the border buffer on every side, as Measure uses it when the
/// host measures the frame the filter renders (`"analyzeArea": "crop"`).
pub struct BorderOverlay {
    /// The band's top-left in the frame, its width, and the measured area in frame pixels.
    x: usize,
    y: usize,
    band_w: usize,
    inside: (usize, usize, usize, usize),
}

impl BorderOverlay {
    pub fn new(image: Option<&Value>, band_w: usize, border: f32) -> BorderOverlay {
        let get = |k: &str| image.and_then(|v| v.get(k)).and_then(Value::as_f64).filter(|f| f.is_finite() && *f >= 0.0).map(|f| f as usize);
        let (x, y) = (get("x").unwrap_or(0), get("y").unwrap_or(0));
        let (cw, ch) = (get("canvasWidth").unwrap_or(band_w), get("canvasHeight").unwrap_or(usize::MAX / 2));
        let inset = |n: usize| ((n as f32 * border / 100.0) as usize).min(n.saturating_sub(1) / 2);
        let (ix, iy) = (inset(cw), inset(ch));
        BorderOverlay { x, y, band_w: band_w.max(1), inside: (ix, iy, cw.saturating_sub(ix), ch.saturating_sub(iy)) }
    }

    /// Whether frame pixel (`fx`, `fy`) is measured.
    pub(crate) fn measured(&self, fx: usize, fy: usize) -> bool {
        let (x0, y0, x1, y1) = self.inside;
        (x0..x1).contains(&fx) && (y0..y1).contains(&fy)
    }

    /// Dims the pixels of `px` (interleaved RGB, starting at band pixel `start`) outside the measured area.
    pub fn apply(&self, px: &mut [f32], start: usize) {
        for (i, c) in px.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            let n = start + i;
            if !self.measured(self.x + n % self.band_w, self.y + n / self.band_w) {
                let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                *c = [0.08 + 0.35 * l, 0.15 * l, 0.12 * l];
            }
        }
    }
}
