//! Small numeric helpers: lookup tables, transfer curves and display curves.

/// Smallest linear sample considered (density 6): keeps `log10` finite on black pixels.
pub(crate) const FLOOR: f32 = 1e-6;

/// The cube root of the linear sample whose density is `d`: the domain colour negatives are
/// balanced and mapped in. Each channel gets the same range there and its own offset, as dye layers
/// of one film respond alike; stretching each channel to its own range oversaturates.
pub(crate) fn cube_root_of(d: f32) -> f32 {
    10f32.powf(-d / 3.0)
}

pub(crate) fn bell(t: f32, centre: f32, width: f32) -> f32 {
    let x = (t - centre) / width;
    (-x * x).exp()
}

/// The profile's curve: a cubic Hermite through (0,0), (½,½), (1,1) with the profile's slopes.
pub(crate) fn profile_curve(t: f32, s: &[f32; 3]) -> f32 {
    let (y0, m0, m1) = if t < 0.5 { (0.0, s[0], s[1]) } else { (0.5, s[1], s[2]) };
    let u = ((t - y0) * 2.0).clamp(0.0, 1.0);
    let (u2, u3) = (u * u, u * u * u);
    // h00·y0 + h10·Δ·m0 + h01·y1 + h11·Δ·m1 with y1 = y0 + Δ, Δ = ½, and h00 + h01 = 1
    let h10 = u3 - 2.0 * u2 + u;
    let h01 = -2.0 * u3 + 3.0 * u2;
    let h11 = u3 - u2;
    y0 + 0.5 * (h01 + m0 * h10 + m1 * h11)
}

/// `log2(x)` for positive finite `x`, from the float's exponent and a table of the mantissa (error < 1e-5).
pub(crate) struct FastLog {
    pub(crate) table: [f32; 257],
}

impl FastLog {
    pub(crate) fn new() -> FastLog {
        let mut table = [0.0; 257];
        for (i, v) in table.iter_mut().enumerate() {
            *v = (1.0 + i as f32 / 256.0).log2();
        }
        FastLog { table }
    }

    pub(crate) fn log2(&self, x: f32) -> f32 {
        let bits = x.to_bits();
        let e = ((bits >> 23) & 0xff) as i32 - 127;
        let m = (bits & 0x7f_ffff) as f32 / (1 << 23) as f32 * 256.0;
        let i = (m as usize).min(255);
        let f = m - i as f32;
        let (a, b) = (self.table.get(i).copied().unwrap_or(0.0), self.table.get(i + 1).copied().unwrap_or(1.0));
        e as f32 + a + (b - a) * f
    }

    /// `−log10(x)`, `x` floored at [`FLOOR`] (NaN counts as the floor).
    pub(crate) fn density(&self, x: f32) -> f32 {
        let x = if x >= FLOOR && x.is_finite() {
            x
        } else if x > FLOOR {
            1e30
        } else {
            FLOOR
        };
        -self.log2(x) * std::f32::consts::LOG10_2
    }
}

/// A 1-D function sampled on `lo..=hi` and read back with linear interpolation.
pub(crate) struct Lut {
    pub(crate) lo: f32,
    pub(crate) scale: f32,
    pub(crate) v: Vec<f32>,
}

impl Lut {
    pub(crate) fn new(lo: f32, hi: f32, n: usize, f: impl Fn(f32) -> f32) -> Lut {
        let v = (0..=n).map(|i| f(lo + (hi - lo) * i as f32 / n as f32)).collect();
        Lut { lo, scale: n as f32 / (hi - lo), v }
    }

    pub(crate) fn get(&self, x: f32) -> f32 {
        let last = self.v.len().saturating_sub(1);
        let p = ((x - self.lo) * self.scale).clamp(0.0, last as f32);
        let i = (p as usize).min(last.saturating_sub(1));
        let f = p - i as f32;
        let (a, b) = (self.v.get(i).copied().unwrap_or(0.0), self.v.get(i + 1).copied().unwrap_or(0.0));
        a + (b - a) * f
    }
}

/// The Fade slider at +50: display value (sRGB-encoded) in → out. Fitted to a reference conversion's
/// fade setting: blacks lift to about 0.2, the lower mids dip a little, highlights stay.
pub(crate) const FADE_CURVE: [(f32, f32); 13] = [
    (0.0, 0.196),
    (0.05, 0.204),
    (0.1, 0.208),
    (0.2, 0.227),
    (0.3, 0.263),
    (0.4, 0.322),
    (0.5, 0.392),
    (0.6, 0.486),
    (0.7, 0.627),
    (0.8, 0.792),
    (0.9, 0.91),
    (0.95, 0.961),
    (1.0, 0.98),
];

/// The Glow slider at +50: mids brighten, whites settle near 0.8 (fitted the same way).
pub(crate) const GLOW_CURVE: [(f32, f32); 13] = [
    (0.0, 0.0),
    (0.05, 0.027),
    (0.1, 0.078),
    (0.2, 0.216),
    (0.3, 0.345),
    (0.4, 0.475),
    (0.5, 0.584),
    (0.6, 0.69),
    (0.7, 0.761),
    (0.8, 0.78),
    (0.9, 0.788),
    (0.95, 0.796),
    (1.0, 0.796),
];

/// `curve` applied with strength `s` in −1..=1: `x + s·(curve(x) − x)`; a negative strength pushes
/// the other way (Fade < 0 deepens the blacks, Glow < 0 darkens the mids and opens the highlights).
pub(crate) fn display_curve(x: f32, curve: &[(f32, f32)], s: f32) -> f32 {
    if s == 0.0 {
        return x;
    }
    let x = x.clamp(0.0, 1.0);
    let i = curve.iter().position(|(cx, _)| *cx >= x).unwrap_or(curve.len() - 1).max(1);
    let ((x0, y0), (x1, y1)) = (curve.get(i - 1).copied().unwrap_or((0.0, 0.0)), curve.get(i).copied().unwrap_or((1.0, 1.0)));
    let y = if x1 > x0 { y0 + (y1 - y0) * (x - x0) / (x1 - x0) } else { y1 };
    (x + s * (y - x)).clamp(0.0, 1.0)
}

pub(crate) fn srgb_encode(l: f32) -> f32 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

pub(crate) fn srgb_decode(v: f32) -> f32 {
    if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// The highlight roll-off on linear light: unchanged up to 0.9, then easing toward 1.
pub(crate) fn shoulder(l: f32) -> f32 {
    if l > 0.9 { 0.9 + 0.1 * ((l - 0.9) / 0.1).tanh() } else { l }
}
