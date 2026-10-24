//! The reference bench: converts each scan of a reference set with ChromaLab and scores it against a
//! reference conversion of the same scan (mean difference in levels of 255, and the bias per
//! channel), so every change is checked against all of them at once.
//!
//! ```text
//! cargo run --release -p bench -- REFS_DIR ['{"method": "Per channel"}'] [--save] [--sheet out.png] [--golden save|check]
//! ```
//!
//! `REFS_DIR` holds the set (kept out of this repository): `refs.toml` and the files it names.
//!
//! ```toml
//! [[frame]]
//! label = "lodge, blue sky"          # any name
//! negative = "lodge.f32"             # the scan as the Source stage sees it: linear sRGB f32, interleaved
//! width = 1200
//! height = 800
//! reference = "lodge-reference.png" # the reference conversion (PNG, WebP or JPEG), any size or rotation
//! crop = [0.095, 0.15, 0.87, 0.84]   # the picture, x0 y0 x1 y1 of the frame (default: all of it)
//! params = { stock = "Fujifilm 400 (US)" }   # optional, layered under the command line's
//! # shift = [6, 12]                  # optional: the reference is already the negative's size, offset by dx, dy
//! ```
//!
//! Each frame is cropped, measured inside a 3 % border buffer (as a user crops to the picture) and
//! converted. `--save` keeps the scores as `REFS_DIR/baseline.json`, and later runs print the change
//! from it. `--sheet` writes reference | ChromaLab pairs side by side. `--golden save` hashes the
//! output of every frame under a fixed set of settings, and `--golden check` compares against
//! those hashes: a refactor that must not change the result passes only if every output is
//! bit-identical.

use std::path::{Path, PathBuf};

use chromalab_core::{Params, analyze, convert};
use image::{RgbImage, imageops};
use serde_json::{Value, json};

struct Frame {
    label: String,
    negative: PathBuf,
    width: usize,
    height: usize,
    reference: PathBuf,
    crop: [f64; 4],
    params: Value,
    shift: Option<[usize; 2]>,
}

/// Settings the golden hashes cover: every method, film type, WB mode and a spread of the rest
/// (changing this list means saving the hashes again).
fn golden_params() -> Vec<Value> {
    vec![
        json!({}),
        json!({"method": "Per channel"}),
        json!({"filmType": "B&W Negative"}),
        json!({"filmType": "Positive (Slide)"}),
        json!({"stock": "Creative: Golden Hour"}),
        json!({"stock": "Harman Phoenix 200", "toneProfile": "Cine Log"}),
        json!({"toneProfile": "Hard", "glow": 30, "fade": -20, "exposure": 0.7, "contrast": 12}),
        json!({"wbMode": "Auto-Color"}),
        json!({"wbMode": "None", "temp": 15, "tint": -8}),
        json!({"wbMode": "Auto-Warm", "softHighs": true, "softLows": true}),
        json!({"whiteClip": 3, "blackClip": -2, "lights": 20, "darks": -15, "whites": 10, "blacks": -10, "brightness": 12}),
        json!({"autoExposure": true, "preSaturation": 5}),
        json!({"method": "Per channel", "wbMode": "Auto-Cool", "stock": "Creative: Portrait Airy"}),
        json!({"stock": "CineStill 800T", "toneProfile": "Linear Deep"}),
        json!({"glow": -40, "fade": 45, "toneProfile": "Shadow Soft"}),
        json!({"borderBuffer": 15, "showBorder": true}),
    ]
}

fn main() {
    if let Err(e) = run() {
        eprintln!("bench: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir = None;
    let mut params = json!({});
    let (mut save, mut sheet, mut golden) = (false, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--save" => save = true,
            "--sheet" => sheet = Some(it.next().ok_or("--sheet needs a file")?.clone()),
            "--golden" => golden = Some(it.next().ok_or("--golden needs save or check")?.clone()),
            s if s.starts_with('{') => params = serde_json::from_str(s).map_err(|e| format!("settings: {e}"))?,
            s if dir.is_none() => dir = Some(PathBuf::from(s)),
            s => return Err(format!("unexpected argument {s:?}")),
        }
    }
    let dir = dir.ok_or("usage: bench REFS_DIR ['{settings}'] [--save] [--sheet out.png] [--golden save|check]")?;
    let frames = read_set(&dir)?;
    if let Some(mode) = golden {
        return run_golden(&dir, &frames, &mode);
    }
    let baseline: Value = std::fs::read_to_string(dir.join("baseline.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    let mut scores = serde_json::Map::new();
    let mut pairs = Vec::new();
    let mut total = 0.0;
    for f in &frames {
        let (ours, w, h) = render(f, &params)?;
        let reference = load_reference(f, &ours, w, h)?;
        let (mae, bias) = score(&ours, &reference);
        total += mae;
        let was = baseline[&f.label].as_f64().map(|b| format!("  ({:+.1})", mae - b)).unwrap_or_default();
        println!("{:<34} {mae:5.1}{was:<9} bias R {:+6.1} G {:+6.1} B {:+6.1}", f.label, bias[0], bias[1], bias[2]);
        scores.insert(f.label.clone(), json!((mae * 100.0).round() / 100.0));
        if sheet.is_some() {
            pairs.push((reference, ours, w, h));
        }
    }
    let mean = total / frames.len().max(1) as f64;
    let was = baseline["mean"].as_f64().map(|b| format!("  ({:+.2})", mean - b)).unwrap_or_default();
    println!("{:<34} {mean:5.2}{was}", "mean");
    scores.insert("mean".into(), json!((mean * 100.0).round() / 100.0));
    if save {
        std::fs::write(dir.join("baseline.json"), Value::Object(scores).to_string()).map_err(|e| e.to_string())?;
        println!("saved as the baseline");
    }
    if let Some(path) = sheet {
        write_sheet(&pairs, &path)?;
    }
    Ok(())
}

fn read_set(dir: &Path) -> Result<Vec<Frame>, String> {
    let text = std::fs::read_to_string(dir.join("refs.toml")).map_err(|e| format!("{}: {e}", dir.join("refs.toml").display()))?;
    let doc: toml::Table = text.parse().map_err(|e| format!("refs.toml: {e}"))?;
    let list = doc.get("frame").and_then(toml::Value::as_array).ok_or("refs.toml: no [[frame]] entries")?;
    list.iter()
        .enumerate()
        .map(|(i, f)| {
            let at = format!("refs.toml: [[frame]] #{}", i + 1);
            let s = |k: &str| f.get(k).and_then(toml::Value::as_str).map(str::to_string).ok_or(format!("{at}: needs {k}"));
            let n = |k: &str| f.get(k).and_then(toml::Value::as_integer).and_then(|v| usize::try_from(v).ok()).ok_or(format!("{at}: needs {k}"));
            let nums = |k: &str| -> Option<Vec<f64>> {
                f.get(k)?.as_array()?.iter().map(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).collect()
            };
            let crop = match nums("crop") {
                None => [0.0, 0.0, 1.0, 1.0],
                Some(c) => <[f64; 4]>::try_from(c).map_err(|_| format!("{at}: crop is four numbers"))?,
            };
            let shift = nums("shift").map(|s| [s.first().copied().unwrap_or(0.0) as usize, s.get(1).copied().unwrap_or(0.0) as usize]);
            let params = f.get("params").map(|p| serde_json::to_value(p).map_err(|e| e.to_string())).transpose()?.unwrap_or(json!({}));
            Ok(Frame {
                label: s("label")?,
                negative: dir.join(s("negative")?),
                width: n("width")?,
                height: n("height")?,
                reference: dir.join(s("reference")?),
                crop,
                params,
                shift,
            })
        })
        .collect()
}

/// The crop in pixels of a `w × h` frame.
fn crop_px(f: &Frame, w: usize, h: usize) -> (usize, usize, usize, usize) {
    let at = |v: f64, n: usize| ((v.clamp(0.0, 1.0) * n as f64) as usize).min(n);
    (at(f.crop[0], w), at(f.crop[1], h), at(f.crop[2], w), at(f.crop[3], h))
}

/// Measures and converts the frame's crop with `params` (the frame's own layered under them):
/// display-encoded 0..255 samples and the crop's size.
fn render(f: &Frame, params: &Value) -> Result<(Vec<f32>, usize, usize), String> {
    let bytes = std::fs::read(&f.negative).map_err(|e| format!("{}: {e}", f.negative.display()))?;
    let px: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
    if px.len() != f.width * f.height * 3 {
        return Err(format!("{}: {} samples, not {} × {} × 3", f.negative.display(), px.len(), f.width, f.height));
    }
    let (x0, y0, x1, y1) = crop_px(f, f.width, f.height);
    let (w, h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let mut crop = Vec::with_capacity(w * h * 3);
    for y in y0..y1 {
        crop.extend_from_slice(&px[(y * f.width + x0) * 3..(y * f.width + x1) * 3]);
    }
    let mut json = json!({"borderBuffer": 3});
    for src in [&f.params, params] {
        if let (Some(o), Some(s)) = (json.as_object_mut(), src.as_object()) {
            o.extend(s.clone());
        }
    }
    let mut out = convert_like_the_host(&crop, w, h, json);
    out.iter_mut().for_each(|v| *v = srgb(*v) * 255.0);
    Ok((out, w, h))
}

/// Measure on a copy with a long edge of 1024 (nearest neighbour), then convert at full size, as the host does.
fn convert_like_the_host(px: &[f32], w: usize, h: usize, mut json: Value) -> Vec<f32> {
    let step = (w.max(h) as f32 / 1024.0).max(1.0);
    let (sw, sh) = ((w as f32 / step) as usize, (h as f32 / step) as usize);
    let small: Vec<f32> = (0..sh)
        .flat_map(|y| (0..sw).map(move |x| ((y as f32 * step) as usize * w + (x as f32 * step) as usize) * 3))
        .flat_map(|i| px[i..i + 3].to_vec())
        .collect();
    let an = analyze(&small, sw, sh, &Params::from_json(&json));
    json["_analysis"] = an.to_json();
    let mut out = px.to_vec();
    convert(&mut out, &Params::from_json(&json));
    out
}

fn srgb(l: f32) -> f32 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

/// The reference at the negative's size, turned to match it, cut to the crop.
fn load_reference(f: &Frame, ours: &[f32], w: usize, h: usize) -> Result<Vec<f32>, String> {
    let img = image::open(&f.reference).map_err(|e| format!("{}: {e}", f.reference.display()))?.to_rgb8();
    let (x0, y0, ..) = crop_px(f, f.width, f.height);
    let cut = |full: &RgbImage| -> Vec<f32> {
        let mut out = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let p = full.get_pixel_checked((x0 + x) as u32, (y0 + y) as u32).map(|p| p.0).unwrap_or([0; 3]);
                out.extend(p.iter().map(|v| f32::from(*v)));
            }
        }
        out
    };
    if let Some([dx, dy]) = f.shift {
        let full = imageops::crop_imm(&img, dx as u32, dy as u32, f.width as u32, f.height as u32).to_image();
        return Ok(cut(&full));
    }
    // the rotation whose brightness pattern matches the conversion best
    let mut best: Option<(f64, Vec<f32>)> = None;
    for turn in 0..4 {
        let t = match turn {
            0 => img.clone(),
            1 => imageops::rotate90(&img),
            2 => imageops::rotate180(&img),
            _ => imageops::rotate270(&img),
        };
        if (t.width() > t.height()) != (f.width > f.height) {
            continue;
        }
        let full = imageops::resize(&t, f.width as u32, f.height as u32, imageops::FilterType::Triangle);
        let r = cut(&full);
        let c = correlation(ours, &r);
        if best.as_ref().is_none_or(|b| c > b.0) {
            best = Some((c, r));
        }
    }
    best.map(|b| b.1).ok_or_else(|| format!("{}: no orientation matches the negative", f.reference.display()))
}

fn correlation(a: &[f32], b: &[f32]) -> f64 {
    let lum = |p: &[f32]| p.as_chunks::<3>().0.iter().map(|c| f64::from(c[0] + c[1] + c[2])).collect::<Vec<_>>();
    let (a, b) = (lum(a), lum(b));
    let n = a.len().min(b.len()).max(1) as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut cov, mut va, mut vb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(&b) {
        cov += (x - ma) * (y - mb);
        va += (x - ma) * (x - ma);
        vb += (y - mb) * (y - mb);
    }
    cov / (va * vb).sqrt().max(1e-9)
}

/// Mean absolute difference and the mean signed difference per channel, in levels of 255.
fn score(ours: &[f32], reference: &[f32]) -> (f64, [f64; 3]) {
    let n = (ours.len().min(reference.len()) / 3).max(1) as f64;
    let (mut mae, mut bias) = (0.0, [0.0; 3]);
    for (i, (a, b)) in ours.iter().zip(reference).enumerate() {
        let d = f64::from(a.round() - b);
        mae += d.abs();
        bias[i % 3] += d;
    }
    (mae / (n * 3.0), bias.map(|b| b / n))
}

fn write_sheet(pairs: &[(Vec<f32>, Vec<f32>, usize, usize)], path: &str) -> Result<(), String> {
    const CELL: u32 = 400;
    let rows = pairs.len().div_ceil(2) as u32;
    let mut sheet = RgbImage::from_pixel(4 * CELL + 30, rows * (CELL + 10), image::Rgb([255, 255, 255]));
    for (i, (reference, ours, w, h)) in pairs.iter().enumerate() {
        for (k, px) in [reference, ours].into_iter().enumerate() {
            let img = RgbImage::from_fn(*w as u32, *h as u32, |x, y| {
                let o = (y as usize * w + x as usize) * 3;
                image::Rgb([0, 1, 2].map(|c| px.get(o + c).copied().unwrap_or(0.0).round().clamp(0.0, 255.0) as u8))
            });
            let s = (CELL as f32 / *w.max(h) as f32).min(1.0);
            let small = imageops::resize(&img, (*w as f32 * s) as u32, (*h as f32 * s) as u32, imageops::FilterType::Triangle);
            let x = (i as u32 % 2) * (2 * CELL + 20) + k as u32 * (CELL + 5);
            imageops::overlay(&mut sheet, &small, i64::from(x), i64::from((i as u32 / 2) * (CELL + 10)));
        }
    }
    sheet.save(path).map_err(|e| format!("{path}: {e}"))?;
    println!("wrote {path} (each pair: reference | ChromaLab)");
    Ok(())
}

fn run_golden(dir: &Path, frames: &[Frame], mode: &str) -> Result<(), String> {
    let mut hashes = serde_json::Map::new();
    for f in frames {
        for (i, p) in golden_params().iter().enumerate() {
            let (out, ..) = render(f, p)?;
            // FNV-1a over the exact bits
            let h = out
                .iter()
                .flat_map(|v| v.to_bits().to_le_bytes())
                .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
            hashes.insert(format!("{} #{i}", f.label), json!(format!("{h:016x}")));
        }
    }
    let path = dir.join("golden.json");
    match mode {
        "save" => {
            std::fs::write(&path, Value::Object(hashes.clone()).to_string()).map_err(|e| e.to_string())?;
            println!("saved {} output hashes", hashes.len());
        }
        "check" => {
            let saved: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| e.to_string())?;
            let differ: Vec<&String> = hashes.iter().filter(|(k, v)| saved.get(k.as_str()) != Some(v)).map(|(k, _)| k).collect();
            if differ.is_empty() {
                println!("all {} outputs bit-identical to the saved ones", hashes.len());
            } else {
                return Err(format!("{} of {} outputs differ, e.g. {:?}", differ.len(), hashes.len(), &differ[..differ.len().min(5)]));
            }
        }
        m => return Err(format!("--golden {m}: use save or check")),
    }
    Ok(())
}
