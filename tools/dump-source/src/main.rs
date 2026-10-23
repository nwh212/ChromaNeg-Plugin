//! Writes a photo as the app's Source stage sees it, the way Measure gets it: decoded, framed by its
//! develop geometry (orientation; no crop on a fresh import), in linear sRGB primaries, as
//! interleaved little-endian `f32` RGB, for the bench (`tools/bench`).
//!
//! ```text
//! cargo run --release --manifest-path tools/dump-source/Cargo.toml -- scan.ARW scan.f32 [long_edge=1200]
//! ```
//!
//! Prints `width height`, which go into the bench's `refs.toml`.

use lcc_engine::{Session, SourceLevel};
use serde_json::json;

fn main() {
    if let Err(e) = run() {
        eprintln!("dump-source: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    let (input, out) = (a.get(1).ok_or("usage: dump-source IN OUT.f32 [long_edge]")?, a.get(2).ok_or("missing OUT.f32")?);
    let edge: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1200);
    let mut s = Session::new().with_fs().with_system_clock();
    let abs = std::fs::canonicalize(input).map_err(|e| format!("{input}: {e}"))?;
    let r = s.execute("library.import", &json!({"paths": [abs]})).map_err(|e| e.to_string())?;
    let id = lcc_engine::catalog::PhotoId(r["imported"][0].as_u64().ok_or_else(|| format!("{input}: not a readable photo"))?);
    let photo = s.catalog.photo(id).cloned().ok_or("the photo vanished")?;
    let develop = s.develop_of(id).ok_or("no develop settings")?;
    let src = s.media.source_ref(&photo, SourceLevel::Full).load_source()?;
    let (data, w, h) = lcc_pipeline::plugin_filters::analysis_input_framed(&src.image, &develop, edge);
    let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(out, bytes).map_err(|e| format!("{out}: {e}"))?;
    println!("{w} {h}");
    Ok(())
}
