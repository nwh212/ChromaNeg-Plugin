//! Runs the filter on a file the way the host does, for tuning by eye:
//! `cargo run --release --example develop -- in.f32 width height '{"borderBuffer":20}' out.f32`
//! (interleaved little-endian linear RGB `f32`). Measure runs on a copy with a long edge of 1024, as the host's does.

use chromaneg_core::{Params, analyze, convert};

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    let [_, input, w, h, params, output] = a.as_slice() else {
        return Err("usage: develop in.f32 width height params.json out.f32".into());
    };
    let (w, h): (usize, usize) = (w.parse().map_err(|_| "width")?, h.parse().map_err(|_| "height")?);
    let bytes = std::fs::read(input).map_err(|e| e.to_string())?;
    let mut px: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
    let json: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
    let p = Params::from_json(&json);
    // the measurement copy: nearest-neighbour, long edge 1024
    let step = (w.max(h) as f32 / 1024.0).max(1.0);
    let (sw, sh) = ((w as f32 / step) as usize, (h as f32 / step) as usize);
    let small: Vec<f32> = (0..sh)
        .flat_map(|y| (0..sw).map(move |x| ((y as f32 * step) as usize * w + (x as f32 * step) as usize) * 3))
        .flat_map(|i| px[i..i + 3].to_vec())
        .collect();
    let an = analyze(&small, sw, sh, &p);
    eprintln!("{}", an.to_json());
    let mut json = json;
    json["_analysis"] = an.to_json();
    let p = Params::from_json(&json);
    let t = std::time::Instant::now();
    convert(&mut px, &p);
    eprintln!("convert: {:?} for {} px (native)", t.elapsed(), w * h);
    std::fs::write(output, px.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).map_err(|e| e.to_string())
}
