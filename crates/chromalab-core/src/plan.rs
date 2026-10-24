//! The conversion plan: from the parameters and the measurement, how each sample becomes light.

use crate::{analysis::*, math::*, params::*, presets::*};

/// White and black clip 0 pick these percentiles (5 % and 3 %, fitted to three reference conversions:
/// a colour negative portrait, a Harman Phoenix landscape and a Kodak 200 daylight frame); each clip step moves one entry along
/// [`PERCENTILES`].
pub(crate) const WHITE_I: i64 = 8;
pub(crate) const BLACK_I: i64 = 7;
/// Colour negatives (the cube-root domain): black at 1 % from the thin end, white at 0.5 % from the
/// dense end, each channel normalised at 5 % from its thin end, and this share of the remaining
/// difference between the channels' averages balanced; fitted to reference conversions.
pub(crate) const CUBE_BLACK_I: i64 = 4;
pub(crate) const CUBE_AVERAGE: f32 = 0.5;
/// Red's contrast relative to the other channels in the cube-root domain. Camera captures came
/// out short of red in their brighter parts (golden leaves pale cyan, skin cool): fitted to the
/// reference conversions, it helps warm and indoor scenes most and costs a blue-sky frame a little.
const CUBE_RED_GAIN: f32 = 1.1;
pub(crate) const CUBE_WHITE_I: i64 = 3;
pub(crate) const CUBE_BALANCE_I: usize = 8;
/// How much steeper than a profile's own power the cube-root domain's `t` is turned into light.
pub(crate) const CUBE_POWER: f32 = 1.25;

/// Auto exposure: the linear light a frame's median brightness should reach, and the most it is
/// lifted, in stops.
pub(crate) const AUTO_MID: f32 = 0.18;
pub(crate) const AUTO_MAX_STOPS: f32 = 2.0;

pub(crate) fn clip_index(clip: i64, zero: i64) -> usize {
    (clip + zero).clamp(0, NP as i64 - 1) as usize
}

/// How each channel's density becomes `t`: `t = (d − origin) · scale`, then per channel `gamma`.
pub(crate) struct Plan {
    pub(crate) origin: [f32; 3],
    pub(crate) scale: [f32; 3],
    pub(crate) flip: bool,
    pub(crate) bw: bool,
    pub(crate) offset: [f32; 3],
    pub(crate) sat: f32,
    pub(crate) crosstalk: f32,
    pub(crate) hue_sat: Option<[f32; 6]>,
    pub(crate) shadows: [f32; 3],
    pub(crate) highlights: [f32; 3],
    pub(crate) tinted: bool,
    /// Colour negatives: density → cube root of the sample, and `t = (origin − that) · scale`.
    pub(crate) cube: Option<Lut>,
    /// …that cube root times this per channel first.
    pub(crate) mul: [f32; 3],
    /// `t` (before tone) → `t` (after tone and sliders).
    pub(crate) tone: Lut,
    /// `t` (after tone, tint and fade) → linear light.
    pub(crate) linear: Lut,
}

pub(crate) fn plan(p: &Params) -> Plan {
    let a = p.analysis.clone().unwrap_or_else(Analysis::unmeasured);
    let (wi, bi) = (clip_index(p.white_clip, WHITE_I), clip_index(p.black_clip, BLACK_I));
    let bw = p.film == FilmType::BwNegative;
    let flip = p.film == FilmType::Positive;
    // on a slide the thin end is white, so the clips swap ends
    let (thin_i, dense_i) = if flip { (wi, bi) } else { (bi, wi) };
    let mut origin = [0.0; 3];
    let mut scale = [1.0; 3];
    // where the frame's median brightness lands in `t` (before the flip), for auto exposure
    let mut mid = 0.5;
    let cube = !bw && !flip && !p.per_channel;
    let mut mul = [1.0; 3];
    if bw {
        let (thin, dense) = (a.lo[3][thin_i], a.hi[3][dense_i].max(a.lo[3][thin_i] + 0.05));
        origin = [thin; 3];
        scale = [1.0 / (dense - thin); 3];
    } else if cube {
        // each channel divided by its own thin end (the mask removed: a grey's channels then
        // agree), then in the cube-root domain (bright samples high): black at green's thin end,
        // white at its dense end, every channel the same range, so the layers keep their contrast
        let refd = |c: usize| a.lo[c][CUBE_BALANCE_I];
        mul = [0, 1, 2].map(|c| 1.0 / cube_root_of(refd(c)));
        let v = |c: usize, d: f32| cube_root_of(d - refd(c));
        let (bi, wi) = (clip_index(p.black_clip, CUBE_BLACK_I), clip_index(p.white_clip, CUBE_WHITE_I));
        let black = |c: usize| v(c, a.lo[c][bi]);
        let range = |c: usize| (black(c) - v(c, a.hi[c][wi])).max(0.01);
        // and half of what remains between the channels' averages (a cast of the frame) balanced
        let average = |c: usize| a.vmean[c] * mul[c] - a.vmean[1] * mul[1];
        for c in 0..3 {
            (origin[c], scale[c]) = match p.wb {
                // each channel to its own black and white
                Wb::Color => (black(c), 1.0 / range(c)),
                Wb::None => (black(1), 1.0 / range(1)),
                _ => (black(1) + CUBE_AVERAGE * average(c), if c == 0 { CUBE_RED_GAIN } else { 1.0 } / range(1)),
            };
        }
        mid = (black(1) - v(1, a.dmid[1])) / range(1);
    } else if p.wb == Wb::Color {
        for c in 0..3 {
            let (thin, dense) = (a.lo[c][thin_i], a.hi[c][dense_i].max(a.lo[c][thin_i] + 0.05));
            origin[c] = thin;
            scale[c] = 1.0 / (dense - thin);
        }
    } else {
        // base removed and layers balanced (unless None); the thin end per channel, so the
        // shadows (of a negative) come out neutral, the dense end shared from the brightness
        let neutral = p.wb != Wb::None;
        let gain = if neutral { a.gain } else { [1.0; 3] };
        let thin = if neutral { [a.ge_lo[0][thin_i], a.ge_lo[1][thin_i], a.ge_lo[2][thin_i]] } else { [a.lum_lo[thin_i]; 3] };
        let thin_mean = (thin[0] + thin[1] + thin[2]) / 3.0;
        let dense = a.lum_hi[dense_i].max(thin_mean + 0.05);
        // the highlight reference's range per channel relative to their mean: neutralise it when it is
        // nearly neutral already (a white wall), leave it when it is clearly coloured (a blue sky)
        let span = [a.hl_ref[0] - thin[0], a.hl_ref[1] - thin[1], a.hl_ref[2] - thin[2]];
        let mean_span = (span[0] + span[1] + span[2]) / 3.0;
        let ratio = if neutral && mean_span > 0.05 { span.map(|v| v / mean_span) } else { [1.0; 3] };
        let dev = ratio.iter().map(|r| (r - 1.0).abs()).fold(0.0, f32::max);
        let weight = ((HIGHLIGHT_NEUTRAL.1 - dev) / (HIGHLIGHT_NEUTRAL.1 - HIGHLIGHT_NEUTRAL.0)).clamp(0.0, 1.0);
        for c in 0..3 {
            // t = ((d − base)/gain − thin_c)/((dense − thin_mean) · highlight correction_c)
            let hl = 1.0 + weight * (ratio[c] - 1.0);
            origin[c] = a.lo[c][BASE_I] + thin[c] * gain[c];
            scale[c] = 1.0 / (gain[c] * (dense - thin_mean) * hl.max(0.2));
        }
        mid = (a.lum_mid[1] - thin_mean) / (dense - thin_mean);
    }
    let st = p.stock;
    let temp = p.temp
        + st.temp
        + match p.wb {
            Wb::Warm => 10.0,
            Wb::Cool => -10.0,
            _ => 0.0,
        };
    let tint = p.tint + st.tint;
    let offset = if bw { [0.0; 3] } else { [temp / 400.0, -tint / 400.0, -temp / 400.0] };
    let sat = if bw { 0.0 } else { (1.0 + (p.pre_saturation - 3.0) * 0.15) * st.sat * p.profile.sat };
    // the shared range keeps the layers' own saturation: half the cross-talk correction suits it
    let crosstalk = if bw { 0.0 } else { st.crosstalk * if cube { 0.5 } else { 1.0 } };
    let mut pr = p.profile;
    // t in the cube-root domain needs a steeper power to reach the same light
    if cube {
        pr.power *= CUBE_POWER;
    }
    let power = pr.power;
    let contrast = p.contrast + st.contrast;
    let mut params = p.clone();
    params.darks += st.darks;
    if p.auto_exposure && p.analysis.is_some() {
        let mid = if flip { 1.0 - mid } else { mid };
        // lift a dark frame's median toward mid grey (at most 2 stops, never darker), and roll the highlights off
        let light = mid.max(1e-3).powf(power);
        let stops = (AUTO_MID / light).log2().clamp(0.0, AUTO_MAX_STOPS);
        params.exposure += stops;
        params.soft_highs |= stops > 0.0;
    }
    let tone = Lut::new(-0.6, 1.6, 2200, move |t| tone(t, &params, contrast, &pr));
    // the profile's and the stock's lift (in t); the Fade and Glow sliders act on the display values below
    let fade = pr.lift + st.fade / 2.0;
    let (fade_s, glow_s) = ((p.fade / 50.0).clamp(-1.0, 1.0), (p.glow / 50.0).clamp(-1.0, 1.0));
    let linear = Lut::new(0.0, T_MAX, 1300, move |t| {
        let t = fade + t.clamp(0.0, T_MAX) * (1.0 - fade);
        let l = shoulder(t.powf(power));
        if fade_s == 0.0 && glow_s == 0.0 {
            return l;
        }
        let v = display_curve(display_curve(srgb_encode(l), &FADE_CURVE, fade_s), &GLOW_CURVE, glow_s);
        srgb_decode(v)
    });
    let shadows = st.shadows;
    let highlights = st.highlights;
    let tinted = !bw && shadows.iter().chain(highlights.iter()).any(|v| *v != 0.0);
    let hue_sat = (!bw && st.hue_sat != [1.0; 6]).then_some(st.hue_sat);
    let cube = cube.then(|| Lut::new(D_MIN, D_MAX, 4400, cube_root_of));
    Plan { origin, scale, flip, bw, offset, sat, crosstalk, hue_sat, shadows, highlights, tinted, cube, mul, tone, linear }
}

/// The saturation factor for the hue of `t` (an RGB triple around its mean), from six sectors
/// (red, yellow, green, cyan, blue, magenta) interpolated linearly.
pub(crate) fn hue_factor(t: &[f32; 3], hs: &[f32; 6]) -> f32 {
    let x = t[0] - 0.5 * (t[1] + t[2]);
    let y = 0.866_025_4 * (t[1] - t[2]);
    if x * x + y * y < 1e-12 {
        return 1.0;
    }
    let deg = y.atan2(x).to_degrees().rem_euclid(360.0);
    let f = deg / 60.0;
    let i = (f as usize).min(5);
    let (a, b) = (hs.get(i).copied().unwrap_or(1.0), hs.get((i + 1) % 6).copied().unwrap_or(1.0));
    a + (b - a) * (f - i as f32)
}

/// Headroom above white (`t` = 1) that the tone stage keeps for the highlight roll-off.
pub(crate) const T_MAX: f32 = 1.3;

/// The profile and the tone sliders, on one channel's normalised log exposure.
pub(crate) fn tone(mut t: f32, p: &Params, contrast: f32, pr: &Profile) -> f32 {
    let (bp, wp) = (-p.blacks / 250.0, 1.0 - p.whites / 250.0);
    t = (t - bp) / (wp - bp).max(0.05);
    // E stops of light: t^power scales by 2^E
    t *= 2f32.powf(p.exposure / pr.power);
    t = 0.5 + (t - 0.5) * (1.0 + contrast / 100.0);
    t += 0.25 * (p.lights / 100.0 * bell(t, 0.75, 0.2) + p.darks / 100.0 * bell(t, 0.25, 0.2));
    let g = pr.gamma * 2f32.powf(-p.brightness / 50.0);
    if t > 0.0 && t < 1.0 && g != 1.0 {
        t = t.powf(g);
    }
    if (0.0..=1.0).contains(&t) {
        t = profile_curve(t, &pr.slopes);
    }
    t = if p.soft_highs && t > 0.85 { 0.85 + 0.15 * ((t - 0.85) / 0.15).tanh() } else { t };
    t = if p.soft_lows && t < 0.15 { 0.15 - 0.15 * ((0.15 - t) / 0.15).tanh() } else { t };
    // a little headroom above white: the linear stage rolls it off instead of clipping
    t.clamp(0.0, T_MAX)
}
