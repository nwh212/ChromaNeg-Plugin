//! Film grain reduction for scanned film, independent of the plug-in ABI.
//!
//! Sources: the à trous (B3-spline) wavelet transform (J.-L. Starck, F. Murtagh, "Image
//! restoration with noise suppression using the wavelet transform", A&A 288, 1994) and wavelet
//! shrinkage (D. Donoho, "De-noising by soft-thresholding", IEEE TIT 41(3), 1995), with a smooth,
//! Wiener-like shrink instead of a hard threshold; own design otherwise.
//!
//! 1. Each sample goes to a perceptual scale (`√` of linear light, where grain is about equally
//!    strong in shadows and highlights) and into brightness and two colour differences, so colour
//!    grain can be cleaned harder than brightness grain, which hides less detail.
//! 2. Each plane is split into detail layers of doubling size plus a smooth residual (the à
//!    trous transform: no downsampling, so nothing shifts and the layers add back exactly).
//! 3. Each detail coefficient `d` is shrunk to `d · d² / (d² + T²)`: kept where it stands well
//!    above the grain's level `T`, faded out where it doesn't. `T` per layer comes from the
//!    settings only (never measured per band, which would show seams between the host's bands):
//!    Fine, Medium and Coarse set the brightness layers of 1, 2 and 4 px; Colour grain the
//!    colour planes, worked at half size.
//! 4. The layers are added back, a share of what was removed is returned (Keep texture), and the
//!    result goes back to linear RGB.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde_json::Value;

/// Detail layers of brightness: 1, 2 and 4 px apart (Fine, Medium, Coarse); coarser structure
/// stays in the residual.
pub const LEVELS: usize = 3;
/// Detail layers of the colour planes, worked at half size (2 and 4 px of the full image).
const CHROMA_LEVELS: usize = 2;

/// Pixels of context a band needs around it: the B3 kernel reaches 2 taps, dilated 1, 2, 4 times
/// (14 px), and the colour planes' 2 half-size layers as far, plus their resampling. The host
/// pads every band by this much, so it is kept small: the bands are thin on wide photos.
pub const OVERLAP: u32 = 16;

/// The resolved settings, each 0..=1.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Brightness grain removed at the finest layer (1 px), the next (2 px) and the coarse one (4 px).
    pub fine: f32,
    pub medium: f32,
    pub coarse: f32,
    /// Colour grain removed (every layer).
    pub color: f32,
    /// Share of what was removed that is returned.
    pub keep: f32,
}

impl Params {
    pub fn from_json(v: &Value) -> Params {
        let num = |k: &str, d: f64| (v.get(k).and_then(Value::as_f64).filter(|f| f.is_finite()).unwrap_or(d) as f32 / 100.0).clamp(0.0, 1.0);
        Params {
            fine: num("fine", 35.0),
            medium: num("medium", 20.0),
            coarse: num("coarse", 35.0),
            color: num("colorGrain", 40.0),
            keep: num("keepTexture", 0.0),
        }
    }

    /// The grain level `T` per detail layer (finest first) for brightness, and per layer of the
    /// half-size colour planes (2 px, 4 px), in the `√` scale. 100 removes grain of about
    /// 6 % of the range at that size. Picked by A/B comparisons on scans: the defaults suit a
    /// fine-grained 400 stock (Fuji 400); a very grainy one (Harman Phoenix 200) wanted about
    /// 90 / 70 / 100, with colour 40 and no texture returned.
    fn thresholds(&self) -> ([f32; LEVELS], [f32; CHROMA_LEVELS]) {
        const FULL: f32 = 0.06;
        let c = FULL * 1.5 * self.color;
        ([self.fine, self.medium, self.coarse].map(|a| a * FULL), [c, 0.6 * c])
    }

    /// Whether the settings change anything.
    pub fn is_noop(&self) -> bool {
        (self.fine == 0.0 && self.medium == 0.0 && self.coarse == 0.0 && self.color == 0.0) || self.keep >= 1.0
    }
}

/// The B3 kernel (1, 4, 6, 4, 1) / 16 applied to five samples.
#[inline(always)]
fn b3(a: f32, b: f32, c: f32, d: f32, e: f32) -> f32 {
    0.0625 * (a + e) + 0.25 * (b + d) + 0.375 * c
}

/// `out[i] = b3(src[i - 2s], src[i - s], src[i], src[i + s], src[i + 2s])` along a line of `n`
/// samples, edges repeated. The interior runs over zipped slices, with no bounds checks.
fn line(src: &[f32], out: &mut [f32], s: usize) {
    let n = src.len().min(out.len());
    let at = |i: isize| src.get(i.clamp(0, n as isize - 1) as usize).copied().unwrap_or(0.0);
    if n <= 4 * s {
        for (i, o) in out.iter_mut().enumerate().take(n) {
            let i = i as isize;
            let s = s as isize;
            *o = b3(at(i - 2 * s), at(i - s), at(i), at(i + s), at(i + 2 * s));
        }
        return;
    }
    for i in (0..2 * s).chain(n - 2 * s..n) {
        let (j, k) = (i as isize, s as isize);
        if let Some(o) = out.get_mut(i) {
            *o = b3(at(j - 2 * k), at(j - k), at(j), at(j + k), at(j + 2 * k));
        }
    }
    let m = n - 4 * s;
    let (Some(a), Some(b), Some(c), Some(d), Some(e)) =
        (src.get(..m), src.get(s..s + m), src.get(2 * s..2 * s + m), src.get(3 * s..3 * s + m), src.get(4 * s..4 * s + m))
    else {
        return;
    };
    if let Some(o) = out.get_mut(2 * s..2 * s + m) {
        for (o, ((((a, b), c), d), e)) in o.iter_mut().zip(a.iter().zip(b).zip(c).zip(d).zip(e)) {
            *o = b3(*a, *b, *c, *d, *e);
        }
    }
}

/// One à trous step: `out` = `src` (`w × h`) smoothed with the B3 kernel, taps `step` px apart,
/// along rows into `tmp`, then down columns (five rows at a time, zipped) into `out`.
fn smooth(src: &[f32], out: &mut [f32], tmp: &mut [f32], w: usize, h: usize, step: usize) {
    for (s, t) in src.chunks_exact(w).zip(tmp.chunks_exact_mut(w)) {
        line(s, t, step);
    }
    let row = |y: isize| {
        let y = y.clamp(0, h as isize - 1) as usize;
        tmp.get(y * w..(y + 1) * w).unwrap_or(&[])
    };
    for (y, o) in out.chunks_exact_mut(w).enumerate().take(h) {
        let (y, s) = (y as isize, step as isize);
        let (a, b, c, d, e) = (row(y - 2 * s), row(y - s), row(y), row(y + s), row(y + 2 * s));
        for (o, ((((a, b), c), d), e)) in o.iter_mut().zip(a.iter().zip(b).zip(c).zip(d).zip(e)) {
            *o = b3(*a, *b, *c, *d, *e);
        }
    }
}

/// Removes grain from one plane in place (`w × h`): the detail of each layer is shrunk with its
/// grain level `t` and added back to the smooth residual.
fn denoise_plane(plane: &mut [f32], w: usize, h: usize, t: &[f32], keep: f32) {
    if t.iter().all(|v| *v <= 0.0) {
        return;
    }
    let n = w * h;
    let mut cur = plane.to_vec();
    let mut next = vec![0.0f32; n];
    let mut tmp = vec![0.0f32; n];
    // accumulates the shrunk details; the residual is added at the end
    let mut acc = vec![0.0f32; n];
    for (level, tl) in t.iter().enumerate() {
        smooth(&cur, &mut next, &mut tmp, w, h, 1 << level);
        let t2 = tl * tl;
        for ((a, c), s) in acc.iter_mut().zip(&cur).zip(&next) {
            let d = c - s;
            *a += if t2 > 0.0 { d * d * d / (d * d + t2) } else { d };
        }
        std::mem::swap(&mut cur, &mut next);
    }
    for ((p, a), r) in plane.iter_mut().zip(&acc).zip(&cur) {
        let clean = a + r;
        *p = clean + keep * (*p - clean);
    }
}

/// [`denoise_plane`] on a half-size copy (2 × 2 box averages), its change brought back to full
/// size (bilinear) and applied: colour grain is coarse, and this costs a quarter as much.
fn denoise_plane_half(plane: &mut [f32], w: usize, h: usize, t: &[f32], keep: f32) {
    if t.iter().all(|v| *v <= 0.0) {
        return;
    }
    let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
    let px = |x: usize, y: usize| plane.get(y.min(h - 1) * w + x.min(w - 1)).copied().unwrap_or(0.0);
    let half: Vec<f32> = (0..hh)
        .flat_map(|y| (0..hw).map(move |x| (x, y)))
        .map(|(x, y)| 0.25 * (px(2 * x, 2 * y) + px(2 * x + 1, 2 * y) + px(2 * x, 2 * y + 1) + px(2 * x + 1, 2 * y + 1)))
        .collect();
    let mut clean = half.clone();
    denoise_plane(&mut clean, hw, hh, t, keep);
    let delta: Vec<f32> = clean.iter().zip(&half).map(|(c, o)| c - o).collect();
    // bilinear, half-pixel centres
    let d = |x: isize, y: isize| delta.get(y.clamp(0, hh as isize - 1) as usize * hw + x.clamp(0, hw as isize - 1) as usize).copied().unwrap_or(0.0);
    for (y, row) in plane.chunks_exact_mut(w).enumerate().take(h) {
        let fy = (y as f32 - 0.5) * 0.5;
        let (y0, ty) = (fy.floor() as isize, fy - fy.floor());
        for (x, p) in row.iter_mut().enumerate() {
            let fx = (x as f32 - 0.5) * 0.5;
            let (x0, tx) = (fx.floor() as isize, fx - fx.floor());
            let top = d(x0, y0) + tx * (d(x0 + 1, y0) - d(x0, y0));
            let bot = d(x0, y0 + 1) + tx * (d(x0 + 1, y0 + 1) - d(x0, y0 + 1));
            *p += top + ty * (bot - top);
        }
    }
}

/// Degrains an interleaved linear RGB `f32` image (`w × h`) in place.
pub fn degrain(px: &mut [f32], w: usize, h: usize, p: &Params) {
    let n = w.saturating_mul(h);
    if p.is_noop() || n == 0 || px.len() < n * 3 {
        return;
    }
    let (tl, tc) = p.thresholds();
    // to the √ scale, as brightness and two colour differences
    let enc = |v: f32| if v.is_finite() { v.max(0.0).sqrt() } else { 0.0 };
    let (mut y, mut cb, mut cr) = (vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    for (i, c) in px.as_chunks::<3>().0.iter().take(n).enumerate() {
        let (r, g, b) = (enc(c[0]), enc(c[1]), enc(c[2]));
        let l = 0.25 * r + 0.5 * g + 0.25 * b;
        y[i] = l;
        cb[i] = b - l;
        cr[i] = r - l;
    }
    denoise_plane(&mut y, w, h, &tl, p.keep);
    denoise_plane_half(&mut cb, w, h, &tc, p.keep);
    denoise_plane_half(&mut cr, w, h, &tc, p.keep);
    for (i, c) in px.as_chunks_mut::<3>().0.iter_mut().take(n).enumerate() {
        let (l, b_, r_) = (y[i], cb[i], cr[i]);
        let r = r_ + l;
        let b = b_ + l;
        let g = (l - 0.25 * r - 0.25 * b) / 0.5;
        // keep the input's sign conventions: negative light stays at 0
        for (o, v) in c.iter_mut().zip([r, g, b]) {
            *o = v.max(0.0) * v.max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A deterministic pseudo-random number in -1..1 per index (an integer hash).
    fn noise(i: usize, seed: u32) -> f32 {
        let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ seed.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// A scene in the √ scale: a grey step (left dark, right bright) over a soft gradient,
    /// with brightness grain and stronger colour grain of `amp`, back in linear light.
    fn scene(w: usize, h: usize, amp: f32) -> (Vec<f32>, Vec<f32>) {
        let mut clean = Vec::with_capacity(w * h * 3);
        let mut grainy = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let base = if x < w / 2 { 0.35 } else { 0.7 } + 0.05 * y as f32 / h as f32;
                let i = y * w + x;
                let g = amp * noise(i, 1);
                for c in 0..3 {
                    clean.push(base * base);
                    let v = (base + g + 0.6 * amp * noise(i, 2 + c as u32)).max(0.0);
                    grainy.push(v * v);
                }
            }
        }
        (clean, grainy)
    }

    fn rms_in(a: &[f32], b: &[f32], w: usize, xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> f32 {
        let mut s = 0.0f64;
        let mut n = 0.0;
        for y in ys {
            for x in xs.clone() {
                for c in 0..3 {
                    let i = (y * w + x) * 3 + c;
                    let d = f64::from(a[i].sqrt() - b[i].sqrt());
                    s += d * d;
                    n += 1.0;
                }
            }
        }
        (s / n).sqrt() as f32
    }

    #[test]
    fn grain_in_flat_areas_drops_a_lot_and_the_edge_stays_sharp() {
        let (w, h) = (160, 96);
        let (clean, grainy) = scene(w, h, 0.03);
        let mut out = grainy.clone();
        degrain(&mut out, w, h, &Params::from_json(&json!({"fine": 53, "medium": 40, "coarse": 19, "colorGrain": 53, "keepTexture": 0})));
        let flat = |img: &[f32]| rms_in(img, &clean, w, 10..70, 10..86);
        assert!(flat(&out) < 0.5 * flat(&grainy), "flat grain {} -> {}", flat(&grainy), flat(&out));
        // the step stays a step: the columns next to it keep most of its height
        let col = |img: &[f32], x: usize| (20..76).map(|y| img[(y * w + x) * 3 + 1].sqrt()).sum::<f32>() / 56.0;
        let (lo, hi) = (col(&out, w / 2 - 2), col(&out, w / 2 + 1));
        assert!(hi - lo > 0.8 * 0.35, "edge softened: {lo} .. {hi}");
    }

    #[test]
    fn the_fast_smoothing_matches_the_plain_definition() {
        for (w, h) in [(37, 23), (5, 9), (64, 3), (1, 1)] {
            let src: Vec<f32> = (0..w * h).map(|i| noise(i, 7)).collect();
            for step in [1, 2, 4, 8] {
                let (mut out, mut tmp) = (vec![0.0; w * h], vec![0.0; w * h]);
                smooth(&src, &mut out, &mut tmp, w, h, step);
                let at = |x: isize, y: isize| src[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
                let k = [1.0, 4.0, 6.0, 4.0, 1.0].map(|v: f32| v / 16.0);
                for y in 0..h as isize {
                    for x in 0..w as isize {
                        let mut want = 0.0;
                        for (j, kj) in k.iter().enumerate() {
                            for (i, ki) in k.iter().enumerate() {
                                want += kj * ki * at(x + (i as isize - 2) * step as isize, y + (j as isize - 2) * step as isize);
                            }
                        }
                        let got = out[y as usize * w + x as usize];
                        assert!((got - want).abs() < 1e-5, "{w}x{h} step {step} at {x},{y}: {got} vs {want}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_flat_image_and_zero_settings_stay_as_they_are() {
        let (w, h) = (40, 30);
        let flat = vec![0.2f32; w * h * 3];
        let mut out = flat.clone();
        degrain(&mut out, w, h, &Params::from_json(&json!({})));
        assert!(out.iter().zip(&flat).all(|(a, b)| (a - b).abs() < 1e-5));
        let (_, grainy) = scene(w, h, 0.03);
        let mut out = grainy.clone();
        degrain(&mut out, w, h, &Params::from_json(&json!({"fine": 0, "medium": 0, "coarse": 0, "colorGrain": 0})));
        assert_eq!(out, grainy);
    }

    #[test]
    fn more_amount_removes_more_and_keep_texture_returns_some() {
        let (w, h) = (80, 60);
        let (clean, grainy) = scene(w, h, 0.03);
        let left = |s: serde_json::Value| {
            let mut out = grainy.clone();
            degrain(&mut out, w, h, &Params::from_json(&s));
            rms_in(&out, &clean, w, 5..35, 5..55)
        };
        let (weak, strong) = (left(json!({"fine": 20, "medium": 20, "keepTexture": 0})), left(json!({"fine": 90, "medium": 90, "keepTexture": 0})));
        assert!(strong < weak, "{weak} {strong}");
        assert!(left(json!({"fine": 90, "medium": 90, "keepTexture": 50})) > strong);
    }

    #[test]
    fn hostile_input_never_panics_or_leaves_non_finite_values() {
        for (w, h) in [(0, 0), (1, 1), (1, 7), (7, 1), (3, 2)] {
            let mut px: Vec<f32> = (0..w * h * 3).map(|i| [f32::NAN, f32::INFINITY, -1.0, 1e30, 0.5][i % 5]).collect();
            degrain(&mut px, w, h, &Params::from_json(&json!({"fine": 100, "medium": 100, "coarse": 100, "colorGrain": 100})));
            assert!(px.iter().all(|v| v.is_finite()), "{w}x{h}");
        }
        // a buffer shorter than it claims is left alone
        let mut short = vec![0.5f32; 5];
        degrain(&mut short, 4, 4, &Params::from_json(&json!({})));
        assert_eq!(short, vec![0.5f32; 5]);
        let _ = Params::from_json(&json!({"fine": "lots", "coarse": [3], "keepTexture": null}));
    }
}
