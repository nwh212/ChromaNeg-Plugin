//! Converting pixels with a plan.

use crate::{math::*, params::*, plan::*};

/// Converts interleaved linear RGB in place.
pub fn convert(px: &mut [f32], p: &Params) {
    Converter::new(p).run(px);
}

/// A conversion prepared once (its lookup tables built) and run on any number of chunks.
pub struct Converter {
    pl: Plan,
    log: FastLog,
}

impl Converter {
    pub fn new(p: &Params) -> Converter {
        Converter { pl: plan(p), log: FastLog::new() }
    }

    /// Converts interleaved linear RGB in place.
    pub fn run(&self, px: &mut [f32]) {
        let (pl, log) = (&self.pl, &self.log);
        convert_with(px, pl, log);
    }
}

pub(crate) fn convert_with(px: &mut [f32], pl: &Plan, log: &FastLog) {
    let (sh, hl, tinted) = (pl.shadows, pl.highlights, pl.tinted);
    for c in px.as_chunks_mut::<3>().0 {
        let mut d = [log.density(c[0]), log.density(c[1]), log.density(c[2])];
        if pl.bw {
            d = [(d[0] + d[1] + d[2]) / 3.0; 3];
        }
        let mut t = [0.0f32; 3];
        for k in 0..3 {
            t[k] = match &pl.cube {
                Some(cube) => (pl.origin[k] - pl.mul[k] * cube.get(d[k])) * pl.scale[k],
                None => {
                    let v = (d[k] - pl.origin[k]) * pl.scale[k];
                    if pl.flip { 1.0 - v } else { v }
                }
            };
        }
        if pl.crosstalk != 0.0 {
            // dye cross-talk correction; rows sum to 1 so neutrals stay neutral
            let s = pl.crosstalk;
            t = [
                t[0] + s * (0.08 * t[0] - 0.06 * t[1] - 0.02 * t[2]),
                t[1] + s * (-0.04 * t[0] + 0.08 * t[1] - 0.04 * t[2]),
                t[2] + s * (-0.02 * t[0] - 0.06 * t[1] + 0.08 * t[2]),
            ];
        }
        let m = (t[0] + t[1] + t[2]) / 3.0;
        let sat = match &pl.hue_sat {
            Some(hs) => pl.sat * hue_factor(&t, hs),
            None => pl.sat,
        };
        let mut o = [0.0f32; 3];
        for k in 0..3 {
            o[k] = pl.tone.get(m + (t[k] - m) * sat + pl.offset[k]);
        }
        if tinted {
            let l = (o[0] + o[1] + o[2]) / 3.0;
            let (ws, wh) = (bell(l, 0.12, 0.22), bell(l, 0.88, 0.22));
            for k in 0..3 {
                o[k] += sh[k] * ws * 4.0 + hl[k] * wh * 4.0;
            }
        }
        for k in 0..3 {
            c[k] = pl.linear.get(o[k]);
        }
    }
}
