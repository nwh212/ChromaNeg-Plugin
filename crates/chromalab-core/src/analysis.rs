//! Measure: the whole-image measurement stored with the photo (`_analysis`).

use crate::{math::*, params::*};
use serde_json::{Value, json};

/// Percentiles (in %) stored by the analysis for each end of each histogram.
pub const PERCENTILES: [f64; NP] = [0.0, 0.1, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0];
/// How many percentiles are stored per histogram end.
pub const NP: usize = 11;
/// The film base: the 0.1 % thin end of each channel.
pub(crate) const BASE_I: usize = 1;

/// The analysis format; an older measurement is ignored (the photo is measured again).
pub(crate) const ANALYSIS_VERSION: i64 = 7;

/// Density histogram range and resolution.
pub(crate) const D_MIN: f32 = -0.5;
pub(crate) const D_MAX: f32 = 5.0;
pub(crate) const BINS: usize = 5500;
/// Edges stronger than this (in density, any channel) are film holder, sprocket holes or text, not scene.
pub(crate) const MAX_EDGE: f32 = 0.6;

/// The whole-image measurement.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// For R, G, B and the mean density (index 3, used for B&W): the density at each of
    /// [`PERCENTILES`] from the thin end (`lo`) and the dense end (`hi`).
    pub lo: [[f32; NP]; 4],
    pub hi: [[f32; NP]; 4],
    /// Each channel's contrast relative to green, from the edges (green = 1).
    pub gain: [f32; 3],
    /// The brightness (mean green-equivalent density above the base) at each of [`PERCENTILES`],
    /// from the thin end and the dense end.
    pub lum_lo: [f32; NP],
    pub lum_hi: [f32; NP],
    /// Per channel, the balanced density above the base at each of [`PERCENTILES`] from the thin
    /// end: where each channel's black lies once the layers are balanced.
    pub ge_lo: [[f32; NP]; 3],
    /// The brightness at the 25th, 50th and 75th percentiles.
    pub lum_mid: [f32; 3],
    /// Per channel, the median balanced density of the brightest 5 % of the frame: the highlight
    /// reference, neutralised when it is nearly neutral already.
    pub hl_ref: [f32; 3],
    /// The sample grid measured (width, height, step in pixels): a diagnostic, not used by the conversion.
    pub measured: [u32; 3],
    /// Per channel, the mean of the cube root of the linear sample (the domain colour negatives are
    /// balanced in), and the median density.
    pub vmean: [f32; 3],
    pub dmid: [f32; 3],
    /// The border buffer it was measured with (`None` from before 0.9.2): the border preview hides
    /// once this matches the setting, so a measured photo exports without it.
    pub border: Option<f32>,
}

impl Analysis {
    pub fn to_json(&self) -> Value {
        json!({"version": ANALYSIS_VERSION, "percentiles": PERCENTILES, "lo": self.lo, "hi": self.hi,
               "gain": self.gain, "lumLo": self.lum_lo, "lumHi": self.lum_hi, "geLo": self.ge_lo, "lumMid": self.lum_mid, "hlRef": self.hl_ref,
               "vmean": self.vmean, "dmid": self.dmid, "measured": self.measured, "borderBuffer": self.border})
    }

    pub fn from_json(v: &Value) -> Option<Analysis> {
        if v.get("version")?.as_i64()? != ANALYSIS_VERSION {
            return None;
        }
        fn floats<const N: usize>(v: &Value) -> Option<[f32; N]> {
            let a = v.as_array()?;
            let mut out = [0.0; N];
            for (o, x) in out.iter_mut().zip(a) {
                *o = x.as_f64().filter(|f| f.is_finite())? as f32;
            }
            (a.len() == N).then_some(out)
        }
        let table = |k: &str| -> Option<[[f32; NP]; 4]> {
            let a = v.get(k)?.as_array()?;
            Some([floats(a.first()?)?, floats(a.get(1)?)?, floats(a.get(2)?)?, floats(a.get(3)?)?])
        };
        let gain: [f32; 3] = floats(v.get("gain")?)?;
        if gain.iter().any(|g| !(0.05..=20.0).contains(g)) {
            return None;
        }
        Some(Analysis {
            lo: table("lo")?,
            hi: table("hi")?,
            gain,
            lum_lo: floats(v.get("lumLo")?)?,
            lum_hi: floats(v.get("lumHi")?)?,
            ge_lo: {
                let a = v.get("geLo")?.as_array()?;
                [floats(a.first()?)?, floats(a.get(1)?)?, floats(a.get(2)?)?]
            },
            lum_mid: floats(v.get("lumMid")?)?,
            hl_ref: floats(v.get("hlRef")?)?,
            vmean: floats(v.get("vmean")?)?,
            dmid: floats(v.get("dmid")?)?,
            measured: v.get("measured").and_then(|m| serde_json::from_value(m.clone()).ok()).unwrap_or_default(),
            border: v.get("borderBuffer").and_then(Value::as_f64).filter(|f| f.is_finite()).map(|f| f as f32),
        })
    }

    /// What we assume before the photo is measured: no mask, two density units of range, no
    /// balance. The result keeps the film's cast, which is the cue to press Measure.
    pub(crate) fn unmeasured() -> Analysis {
        Analysis {
            lo: [[0.1; NP]; 4],
            hi: [[2.1; NP]; 4],
            gain: [1.0; 3],
            lum_lo: [0.0; NP],
            lum_hi: [2.0; NP],
            ge_lo: [[0.0; NP]; 3],
            lum_mid: [0.5, 1.0, 1.5],
            hl_ref: [2.0; 3],
            vmean: [0.5; 3],
            dmid: [1.1; 3],
            measured: [0; 3],
            border: None,
        }
    }
}

#[cfg(test)]
pub(crate) fn density(x: f32) -> f32 {
    -x.max(FLOOR).log10()
}

pub(crate) fn bin(d: f32) -> usize {
    (((d - D_MIN) / (D_MAX - D_MIN)) * BINS as f32).clamp(0.0, (BINS - 1) as f32) as usize
}

pub(crate) fn bin_density(i: usize) -> f32 {
    D_MIN + (i as f32 + 0.5) * (D_MAX - D_MIN) / BINS as f32
}

/// A histogram that answers "the value below which `frac` of the samples lie".
pub(crate) struct Hist {
    pub(crate) bins: Vec<u32>,
    pub(crate) count: u64,
}

impl Hist {
    pub(crate) fn new() -> Hist {
        Hist { bins: vec![0; BINS], count: 0 }
    }

    pub(crate) fn add(&mut self, v: f32) {
        if let Some(b) = self.bins.get_mut(bin(v)) {
            *b = b.saturating_add(1);
            self.count += 1;
        }
    }

    pub(crate) fn at(&self, frac: f64) -> f32 {
        let target = (frac * self.count as f64).floor() as u64;
        let mut seen = 0u64;
        for (i, b) in self.bins.iter().enumerate() {
            seen += u64::from(*b);
            if seen > target {
                return bin_density(i);
            }
        }
        bin_density(BINS - 1)
    }

    /// The value at each of [`PERCENTILES`] from the low end and from the high end.
    pub(crate) fn ends(&self) -> ([f32; NP], [f32; NP]) {
        let (mut lo, mut hi) = ([0.0; NP], [0.0; NP]);
        for (i, pct) in PERCENTILES.iter().enumerate() {
            lo[i] = self.at(pct / 100.0);
            hi[i] = self.at(1.0 - pct / 100.0 - 1e-9);
        }
        (lo, hi)
    }
}

/// The share of the frame, by brightness, that forms the highlight reference.
pub(crate) const HIGHLIGHT_SHARE: f64 = 0.05;
/// The highlight reference is neutralised fully when no channel's range differs from the mean by more
/// than the first value, not at all beyond the second (a blue sky stays blue), and partly between.
pub(crate) const HIGHLIGHT_NEUTRAL: (f32, f32) = (0.15, 0.30);

/// The brightness band (green-density percentiles) whose edges set the colour balance.
pub(crate) const EDGE_BAND: (f64, f64) = (0.10, 0.80);

/// Most pixels the measurement reads: plenty for histograms, and it keeps Measure well inside the
/// host's instruction budget whatever size it is handed.
pub(crate) const MAX_SAMPLES: usize = 300_000;

/// Bare light is at least this share of the measured pixels (holes are areas, not specks)…
pub(crate) const BARE_LIGHT_MIN_SHARE: f64 = 0.005;
/// …at most this share…
pub(crate) const BARE_LIGHT_MAX_SHARE: f64 = 0.10;
/// …and a density bin between it and the film counts as empty below this share (the soft edges of
/// the holes leave a thin trail), judged over windows of this much density.
pub(crate) const BARE_LIGHT_EMPTY_BIN: f64 = 2e-5;
pub(crate) const BARE_LIGHT_WINDOW: f32 = 0.02;
/// …and lies this far (in density) below the film in some channel, with almost nothing between.
pub(crate) const BARE_LIGHT_GAP: f32 = 0.12;

/// The density below which a channel's samples are bare light rather than film: the middle of an empty
/// gap of at least [`BARE_LIGHT_GAP`] above a thin cluster of [`BARE_LIGHT_MIN_SHARE`] to
/// [`BARE_LIGHT_MAX_SHARE`] of the samples. `None`
/// when the thin end runs continuously into the film (no sprocket holes in view, or no gap).
pub(crate) fn bare_light_floor(h: &Hist) -> Option<f32> {
    // judged over windows of BARE_LIGHT_WINDOW of density, not single bins: quantised input (an
    // 8-bit file) puts the thin trail between holes and film in a few spiky bins
    let total = h.count as f64;
    let win = ((BARE_LIGHT_WINDOW / (D_MAX - D_MIN) * BINS as f32).round() as usize).max(1);
    let empty = (total * BARE_LIGHT_EMPTY_BIN * win as f64).max(1.0);
    let gap_windows = (BARE_LIGHT_GAP / BARE_LIGHT_WINDOW).ceil() as usize;
    let (mut seen, mut empty_run) = (0f64, 0usize);
    for (k, w) in h.bins.chunks(win).enumerate() {
        let n: f64 = w.iter().map(|b| f64::from(*b)).sum();
        if n <= empty {
            empty_run += 1;
        } else {
            if seen >= BARE_LIGHT_MIN_SHARE * total && empty_run >= gap_windows {
                // the middle of the gap: clear of both the light and the film's thinnest samples
                return Some(bin_density((k - empty_run / 2) * win));
            }
            empty_run = 0;
        }
        seen += n;
        if seen > BARE_LIGHT_MAX_SHARE * total {
            return None;
        }
    }
    None
}

/// Marks samples of bare light (see [`bare_light_floor`], in any channel) as not measured.
pub(crate) fn exclude_bare_light(grid: &mut [[f32; 3]]) {
    let mut hists: Vec<Hist> = (0..3).map(|_| Hist::new()).collect();
    for d in grid.iter().filter(|d| d[0].is_finite()) {
        for (h, v) in hists.iter_mut().zip(d) {
            h.add(*v);
        }
    }
    let floors: Vec<Option<f32>> = hists.iter().map(bare_light_floor).collect();
    if floors.iter().all(Option::is_none) {
        return;
    }
    for d in grid.iter_mut() {
        if d.iter().zip(&floors).any(|(v, f)| f.is_some_and(|f| *v < f)) {
            *d = [f32::NAN; 3];
        }
    }
}

/// The outer band of the measured area, each side, where samples denser than the centre's are
/// left out; the margin they must exceed the centre's densest 0.5 % by.
const EDGE_BAND_SHARE: f32 = 0.2;
const EDGE_DENSE_MARGIN: f32 = 0.1;
/// …and samples thinner than the centre's thinnest 0.5 % by this much (bare film base).
const EDGE_THIN_MARGIN: f32 = 0.03;
/// Dense edge samples are left out only when they cover at most this share of the area (edge
/// print is small), thin ones only when their green densities lie within this range (film base is
/// uniform).
const EDGE_DENSE_MAX_SHARE: f32 = 0.08;
const EDGE_THIN_SPREAD: f32 = 0.08;

/// Leaves out samples in the outer band of the measured area (`gw × gh` cells) that are clearly
/// denser, in any channel, than the densest of its centre (edge print, frame numbers, a light leak
/// on the rebate) or thinner than its thinnest (bare film base), as when the whole strip is
/// measured: they would set white and black outside the picture's own range, flattening it. A
/// bright or dark object inside the picture (a white clock face, a shadow) is in the centre's
/// range and stays.
pub(crate) fn exclude_strip_edges(grid: &mut [[f32; 3]], gw: usize, gh: usize) {
    let (bx, by) = ((gw as f32 * EDGE_BAND_SHARE) as usize, (gh as f32 * EDGE_BAND_SHARE) as usize);
    let edge = |i: usize| {
        let (x, y) = (i % gw.max(1), i / gw.max(1));
        x < bx || y < by || x + bx >= gw || y + by >= gh
    };
    let mut centre: Vec<Hist> = (0..3).map(|_| Hist::new()).collect();
    for (i, d) in grid.iter().enumerate() {
        if !edge(i) && d[0].is_finite() {
            for (h, v) in centre.iter_mut().zip(d) {
                h.add(*v);
            }
        }
    }
    if centre.iter().any(|h| h.count == 0) {
        return;
    }
    let limit: Vec<f32> = centre.iter().map(|h| h.at(0.995) + EDGE_DENSE_MARGIN).collect();
    let floor: Vec<f32> = centre.iter().map(|h| h.at(0.005) - EDGE_THIN_MARGIN).collect();
    let dense = |d: &[f32; 3]| d.iter().zip(&limit).any(|(v, l)| *v > *l);
    let thin = |d: &[f32; 3]| d.iter().zip(&floor).any(|(v, f)| *v < *f);
    // edge print is small: a large dense area along the edge is the picture's (a bright sky)
    let finite = grid.iter().filter(|d| d[0].is_finite()).count();
    let n_dense = grid.iter().enumerate().filter(|(i, d)| edge(*i) && dense(d)).count();
    let drop_dense = n_dense as f32 <= EDGE_DENSE_MAX_SHARE * finite as f32;
    // film base is uniform: thin edge samples spread over a range of densities are the picture's
    // (a dark corner, a gradient)
    let mut spread = Hist::new();
    for (i, d) in grid.iter().enumerate() {
        if edge(i) && thin(d) {
            spread.add(d[1]);
        }
    }
    let drop_thin = spread.count > 0 && spread.at(0.9) - spread.at(0.1) <= EDGE_THIN_SPREAD;
    for (i, d) in grid.iter_mut().enumerate() {
        if edge(i) && ((drop_dense && dense(d)) || (drop_thin && thin(d))) {
            *d = [f32::NAN; 3];
        }
    }
}

/// Measures `px` (interleaved linear RGB, `w × h`, the whole uncropped photo) inside the border buffer.
pub fn analyze(px: &[f32], w: usize, h: usize, p: &Params) -> Analysis {
    analyze_with(|i| px.get(i * 3..i * 3 + 3).map(|c| [c[0], c[1], c[2]]), w, h, p)
}

/// [`analyze`] reading pixel `i` (`y · w + x`) through `pixel`, so the caller needn't copy the image.
pub fn analyze_with(pixel: impl Fn(usize) -> Option<[f32; 3]>, w: usize, h: usize, p: &Params) -> Analysis {
    let inset = |n: usize| ((n as f32 * p.border / 100.0) as usize).min(n.saturating_sub(1) / 2);
    let (ix, iy) = (inset(w), inset(h));
    let (x0, x1, y0, y1) = (ix, w.saturating_sub(ix), iy, h.saturating_sub(iy));
    // every `step`-th pixel each way, densities computed once
    let area = (x1 - x0).saturating_mul(y1 - y0);
    let step = ((area as f64 / MAX_SAMPLES as f64).sqrt().ceil() as usize).max(1);
    let (gw, gh) = ((x1 - x0).div_ceil(step), (y1 - y0).div_ceil(step));
    let log = FastLog::new();
    let mut grid: Vec<[f32; 3]> = Vec::with_capacity(gw * gh);
    for gy in 0..gh {
        for gx in 0..gw {
            let d = pixel((y0 + gy * step) * w + x0 + gx * step).map(|c| [log.density(c[0]), log.density(c[1]), log.density(c[2])]);
            grid.push(d.unwrap_or([f32::NAN; 3]));
        }
    }
    // bare light (sprocket holes, gaps in the film holder) inside the measured area would set the
    // black point far below the film: drop it before measuring
    exclude_bare_light(&mut grid);
    // and so would the film's rebate around the picture: edge print or a light leak setting white,
    // bare film base setting black
    exclude_strip_edges(&mut grid, gw, gh);
    // pass 1: per-channel and mean-density histograms
    let mut hists: Vec<Hist> = (0..4).map(|_| Hist::new()).collect();
    let mut vsum = [0f64; 3];
    let cube = Lut::new(D_MIN, D_MAX, 4400, cube_root_of);
    let cell = |gx: usize, gy: usize| grid.get(gy * gw + gx).copied().filter(|d| d[0].is_finite());
    for d in grid.iter().filter(|d| d[0].is_finite()) {
        for (k, v) in d.iter().copied().chain([(d[0] + d[1] + d[2]) / 3.0]).enumerate() {
            if let Some(hk) = hists.get_mut(k) {
                hk.add(v);
            }
        }
        for (s, v) in vsum.iter_mut().zip(d) {
            *s += f64::from(cube.get(*v));
        }
    }
    if hists[3].count == 0 {
        return Analysis::unmeasured();
    }
    let mut a = Analysis::unmeasured();
    for (k, hk) in hists.iter().enumerate() {
        (a.lo[k], a.hi[k]) = hk.ends();
    }
    for c in 0..3 {
        a.vmean[c] = (vsum[c] / hists[3].count as f64) as f32;
        a.dmid[c] = hists[c].at(0.5);
    }
    // pass 2: edge energy per channel, between mid-tone pixels only (green density between its
    // 10th and 80th percentiles): large bright or dark areas, where a scan's densities are least
    // reliable, would otherwise dominate the balance
    let (band_lo, band_hi) = (hists[1].at(EDGE_BAND.0), hists[1].at(EDGE_BAND.1));
    let in_band = |d: &[f32; 3]| (band_lo..=band_hi).contains(&d[1]);
    let mut edge = [0f64; 3];
    for gy in 0..gh {
        for gx in 0..gw {
            let Some(d) = cell(gx, gy).filter(in_band) else { continue };
            // differences to the right and below
            for n in [(gx + 1 < gw).then(|| cell(gx + 1, gy)).flatten(), (gy + 1 < gh).then(|| cell(gx, gy + 1)).flatten()].into_iter().flatten() {
                let g = [(n[0] - d[0]).abs(), (n[1] - d[1]).abs(), (n[2] - d[2]).abs()];
                if in_band(&n) && g.iter().all(|v| *v < MAX_EDGE) {
                    for c in 0..3 {
                        edge[c] += f64::from(g[c] * g[c]);
                    }
                }
            }
        }
    }
    if edge[1] > 0.0 {
        for c in 0..3 {
            a.gain[c] = ((edge[c] / edge[1]).sqrt() as f32).clamp(0.2, 5.0);
        }
    }
    // pass 3: brightness histogram of the balanced densities
    let base = [a.lo[0][BASE_I], a.lo[1][BASE_I], a.lo[2][BASE_I]];
    let mut lum = Hist::new();
    let mut ge: Vec<Hist> = (0..3).map(|_| Hist::new()).collect();
    for d in grid.iter().filter(|d| d[0].is_finite()) {
        let g = [(d[0] - base[0]) / a.gain[0], (d[1] - base[1]) / a.gain[1], (d[2] - base[2]) / a.gain[2]];
        lum.add((g[0] + g[1] + g[2]) / 3.0);
        for (h, v) in ge.iter_mut().zip(g) {
            h.add(v);
        }
    }
    (a.lum_lo, a.lum_hi) = lum.ends();
    for (c, h) in ge.iter().enumerate() {
        a.ge_lo[c] = h.ends().0;
    }
    a.lum_mid = [lum.at(0.25), lum.at(0.5), lum.at(0.75)];
    // pass 4: the highlight reference, per channel over the brightest 5 %
    let top = lum.at(1.0 - HIGHLIGHT_SHARE);
    let mut hl: Vec<Hist> = (0..3).map(|_| Hist::new()).collect();
    for d in grid.iter().filter(|d| d[0].is_finite()) {
        let g = [(d[0] - base[0]) / a.gain[0], (d[1] - base[1]) / a.gain[1], (d[2] - base[2]) / a.gain[2]];
        if (g[0] + g[1] + g[2]) / 3.0 >= top {
            for (h, v) in hl.iter_mut().zip(g) {
                h.add(v);
            }
        }
    }
    for (c, h) in hl.iter().enumerate() {
        if h.count > 0 {
            a.hl_ref[c] = h.at(0.5);
        }
    }
    a.measured = [gw as u32, gh as u32, step as u32];
    a.border = Some(p.border);
    a
}
