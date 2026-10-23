//! Degrains a raw linear `f32` image, for tuning by eye and timing:
//! `cargo run --release -p degrain-core --example degrain -- in.f32 width height '{"fine": 60}' out.f32`
fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    let [_, input, w, h, params, output] = a.as_slice() else { return Err("usage: degrain in.f32 width height params.json out.f32".into()) };
    let (w, h): (usize, usize) = (w.parse().map_err(|_| "width")?, h.parse().map_err(|_| "height")?);
    let bytes = std::fs::read(input).map_err(|e| e.to_string())?;
    let mut px: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
    let p = degrain_core::Params::from_json(&serde_json::from_str(params).map_err(|e| e.to_string())?);
    let t = std::time::Instant::now();
    degrain_core::degrain(&mut px, w, h, &p);
    eprintln!("degrain: {:?} for {} px", t.elapsed(), w * h);
    std::fs::write(output, px.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).map_err(|e| e.to_string())
}
