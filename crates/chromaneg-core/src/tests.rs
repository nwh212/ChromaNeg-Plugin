//! The conversion's tests, natively.

use crate::{analysis::*, convert::*, math::*, overlay::*, params::*, plan::*, presets::*};
use serde_json::{Value, json};

/// A synthetic colour negative: an orange mask (per-channel base density) plus a scene whose
/// log exposure is spread over the dye layers alike; a clear film border around it.
fn negative(w: usize, h: usize) -> Vec<f32> {
    negative_with(w, h, [0.65; 3])
}

/// [`negative`] with each dye layer's own contrast.
fn negative_with(w: usize, h: usize, gamma: [f32; 3]) -> Vec<f32> {
    let base = [0.25f32, 0.55, 0.85];
    let mut px = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let border = x < w / 20 || y < h / 20 || x >= w - w / 20 || y >= h - h / 20;
            // grey: a ramp of log exposure 0..3 with a stripe pattern for edges
            let e = if border { 0.0 } else { 3.0 * x as f32 / w as f32 + if (y / 4) % 2 == 0 { 0.1 } else { 0.0 } };
            for c in 0..3 {
                px.push(10f32.powf(-(base[c] + gamma[c] * e)));
            }
        }
    }
    px
}

fn measured(px: &[f32], w: usize, h: usize, json: Value) -> Params {
    let mut p = Params::from_json(&json);
    p.analysis = Some(analyze(px, w, h, &p));
    p
}

fn spread(c: &[f32]) -> f32 {
    c.iter().cloned().fold(f32::MIN, f32::max) - c.iter().cloned().fold(f32::MAX, f32::min)
}

#[test]
fn a_grey_negative_comes_out_neutral_and_increasing_in_every_wb_mode() {
    let (w, h) = (200, 40);
    // the shared range keeps layers of unequal contrast unequal, as the film has them; the
    // per-channel method stretches each layer to its own range in density
    let cases =
        [("Shared range", "Auto-Neutral", [0.65; 3]), ("Shared range", "Auto-Color", [0.65; 3]), ("Per channel", "Auto-Neutral", [0.6, 0.65, 0.7])];
    for (method, wb, gamma) in cases {
        let mut px = negative_with(w, h, gamma);
        let wb = wb.to_string() + " / " + method;
        let p = measured(&px, w, h, json!({"stock": "None (as scanned)", "look": "Neutral", "wbMode": wb.split(" / ").next(), "method": method}));
        convert(&mut px, &p);
        let row = &px[(h / 2) * w * 3..(h / 2 + 1) * w * 3];
        let mut last = -1.0;
        for c in row.as_chunks::<3>().0.iter().skip(w / 20 + 1).take(w - w / 10 - 2) {
            assert!(spread(c) < 0.02, "{wb}: not neutral: {c:?}");
            assert!(c[1] >= last - 1e-3, "{wb}: not monotonic");
            last = c[1];
        }
        assert!(last > 0.8, "{wb}: brightest end should be near white, got {last}");
    }
}

#[test]
fn a_small_saturated_object_doesnt_tint_the_neutrals() {
    // the grey negative plus a strongly red-dense patch (a sign) in one corner
    let (w, h) = (200, 80);
    let mut px = negative(w, h);
    for y in 10..20 {
        for x in 150..160 {
            px[(y * w + x) * 3] = 10f32.powf(-3.0);
        }
    }
    let p = measured(&px, w, h, json!({"stock": "None (as scanned)", "look": "Neutral"}));
    convert(&mut px, &p);
    let c = &px[(60 * w + 100) * 3..(60 * w + 100) * 3 + 3];
    assert!(spread(c) < 0.03, "mid grey tinted: {c:?}");
}

#[test]
fn the_tone_tables_match_the_functions() {
    let log = FastLog::new();
    for x in [1e-6f32, 0.001, 0.0371, 0.5, 0.999, 1.0, 3.7] {
        assert!((log.density(x) - density(x)).abs() < 1e-4, "{x}");
    }
    for pr in PROFILES {
        let mut last = -1.0;
        for i in 0..=100 {
            let y = profile_curve(i as f32 / 100.0, &pr.slopes);
            assert!(y >= last - 1e-6, "{}: not monotonic", pr.name);
            last = y;
        }
        assert!(profile_curve(0.0, &pr.slopes).abs() < 1e-6 && (profile_curve(1.0, &pr.slopes) - 1.0).abs() < 1e-6, "{}", pr.name);
        assert!((profile_curve(0.5, &pr.slopes) - 0.5).abs() < 1e-6, "{}", pr.name);
    }
}

#[test]
fn analysis_round_trips_through_json() {
    let (w, h) = (64, 32);
    let px = negative(w, h);
    let a = analyze(&px, w, h, &Params::default());
    assert_eq!(Analysis::from_json(&a.to_json()), Some(a));
}

#[test]
fn every_look_and_profile_renders_finite_values() {
    let (w, h) = (64, 32);
    let px0 = negative(w, h);
    for l in LOOKS {
        for pr in PROFILES {
            let mut px = px0.clone();
            let p = measured(&px, w, h, json!({"look": l.name, "toneProfile": pr.name}));
            convert(&mut px, &p);
            assert!(px.iter().all(|v| v.is_finite() && (-0.01..=1.01).contains(v)), "{} / {}", l.name, pr.name);
        }
    }
}

#[test]
fn every_stock_renders_finite_values_and_none_matches_the_old_none() {
    let (w, h) = (64, 32);
    let px0 = negative(w, h);
    for st in STOCKS {
        let mut px = px0.clone();
        let p = measured(&px, w, h, json!({"stock": st.name}));
        convert(&mut px, &p);
        assert!(px.iter().all(|v| v.is_finite() && (-0.01..=1.01).contains(v)), "{}", st.name);
    }
    assert_eq!(Params::from_json(&json!({"colorModel": "None"})).stock.name, "None (as scanned)");
    assert_eq!(Params::from_json(&json!({"colorModel": "Basic"})).stock.name, "Generic");
}

#[test]
fn hue_sectors_land_where_they_should() {
    let hs = [0.5, 1.0, 1.0, 1.0, 2.0, 1.0];
    assert!((hue_factor(&[0.2, -0.1, -0.1], &hs) - 0.5).abs() < 1e-3, "red");
    assert!((hue_factor(&[-0.1, -0.1, 0.2], &hs) - 2.0).abs() < 1e-3, "blue");
    assert_eq!(hue_factor(&[0.0; 3], &hs), 1.0, "grey");
}

#[test]
fn every_stock_keeps_its_tone_changes_gentle() {
    // Measure already maps each frame's black and white, so a stock must not re-shape the tone
    // much: on a grey ramp every stock stays within a small distance of Generic
    let (w, h) = (200, 40);
    let render = |stock: &str| {
        let mut px = negative(w, h);
        let p = measured(&px, w, h, json!({"stock": stock}));
        convert(&mut px, &p);
        px
    };
    let generic = render("Generic");
    for st in STOCKS {
        let px = render(st.name);
        let row = (h / 2) * w * 3;
        for x in (w / 10..w - w / 10).step_by(10) {
            let (a, b) = (generic[row + x * 3 + 1], px[row + x * 3 + 1]);
            assert!((a - b).abs() < 0.06, "{}: green {b} vs Generic {a} at x={x}", st.name);
        }
    }
}

#[test]
fn fade_and_glow_are_monotonic_and_do_what_they_say() {
    for s in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
        for curve in [&FADE_CURVE[..], &GLOW_CURVE[..]] {
            let mut last = -1.0;
            for i in 0..=200 {
                let y = display_curve(i as f32 / 200.0, curve, s);
                assert!(y >= last - 1e-6, "not monotonic at strength {s}");
                last = y;
            }
        }
    }
    assert!(display_curve(0.0, &FADE_CURVE, 1.0) > 0.15, "fade lifts black");
    assert!(display_curve(0.1, &FADE_CURVE, -1.0) < 0.1, "negative fade deepens the shadows");
    assert!(display_curve(0.5, &GLOW_CURVE, 1.0) > 0.55 && display_curve(1.0, &GLOW_CURVE, 1.0) < 0.85, "glow: brighter mids, softer whites");
    assert_eq!(display_curve(0.37, &GLOW_CURVE, 0.0), 0.37);
    assert!((srgb_decode(srgb_encode(0.18)) - 0.18).abs() < 1e-5);
}

#[test]
fn the_border_preview_dims_exactly_the_unmeasured_frame() {
    // a 100 × 50 frame drawn as one band; a 10 % buffer leaves 10..90 × 5..45 measured
    let image = json!({"x": 0, "y": 0, "width": 100, "height": 50, "canvasWidth": 100, "canvasHeight": 50});
    let o = BorderOverlay::new(Some(&image), 100, 10.0);
    let mut px = vec![0.5f32; 100 * 50 * 3];
    o.apply(&mut px, 0);
    let at = |x: usize, y: usize| px[(y * 100 + x) * 3 + 1];
    assert_eq!(at(50, 25), 0.5, "inside stays");
    assert_eq!(at(10, 5), 0.5, "the inner corner stays");
    assert!(at(9, 25) < 0.2 && at(50, 4) < 0.2 && at(90, 25) < 0.2 && at(50, 45) < 0.2, "outside dims");
    // a lower band of the same frame (rows 25..50), started mid-band: positions follow `_image`
    let lower = json!({"x": 0, "y": 25, "width": 100, "height": 25, "canvasWidth": 100, "canvasHeight": 50});
    let o = BorderOverlay::new(Some(&lower), 100, 10.0);
    let mut px = vec![0.5f32; 100 * 3];
    o.apply(&mut px, 19 * 100); // band row 19 = frame row 44 (inside); the next would be 45
    assert_eq!(px[50 * 3 + 1], 0.5);
    let mut px = vec![0.5f32; 100 * 3];
    o.apply(&mut px, 20 * 100);
    assert!(px[50 * 3 + 1] < 0.2, "frame row 45 is outside");
    // no `_image`: the band is the frame's width, nothing breaks
    let o = BorderOverlay::new(None, 10, 10.0);
    let mut px = vec![0.5f32; 30];
    o.apply(&mut px, 0);
    assert!(px.iter().all(|v| v.is_finite()));
}

#[test]
fn the_border_preview_shows_until_measured_with_that_border_buffer() {
    let (w, h) = (200, 40);
    let px = negative(w, h);
    let show = |border: f64, measured_with: Option<f64>| {
        let mut json = json!({"showBorder": true, "borderBuffer": border});
        if let Some(b) = measured_with {
            json["_analysis"] = analyze(&px, w, h, &Params::from_json(&json!({"borderBuffer": b}))).to_json();
        }
        Params::from_json(&json).shows_border()
    };
    assert!(show(10.0, None), "before Measure");
    assert!(!show(10.0, Some(10.0)), "measured: exports stay clean");
    assert!(show(15.0, Some(10.0)), "the border buffer changed since");
    assert!(!Params::from_json(&json!({"borderBuffer": 10})).shows_border(), "unticked");
}

#[test]
fn sprocket_holes_in_the_measured_area_are_ignored() {
    // the grey negative, then the same film with rows of bare light (transmission 1) added below
    let (w, h) = (200, 100);
    let clean = negative(w, h);
    let mut holes = clean.clone();
    holes.extend(std::iter::repeat_n(1.0f32, w * 6 * 3));
    let p = Params::from_json(&json!({"borderBuffer": 0}));
    let (a, b) = (analyze(&clean, w, h, &p), analyze(&holes, w, h + 6, &p));
    for c in 0..3 {
        assert!((a.ge_lo[c][BLACK_I as usize] - b.ge_lo[c][BLACK_I as usize]).abs() < 0.01, "black {c}");
        assert!((a.lo[c][BASE_I] - b.lo[c][BASE_I]).abs() < 0.01, "base {c}");
        assert!((a.gain[c] - b.gain[c]).abs() < 0.05, "gain {c}");
    }
    // a frame whose thin end runs continuously into the film keeps everything
    let ramp: Vec<f32> = (0..w * h).flat_map(|i| [10f32.powf(-(0.01 * (i % w) as f32)); 3]).collect();
    let mut g: Vec<[f32; 3]> = ramp.as_chunks::<3>().0.iter().map(|c| [density(c[0]), density(c[1]), density(c[2])]).collect();
    exclude_bare_light(&mut g);
    assert!(g.iter().all(|d| d[0].is_finite()), "nothing excluded without a gap");
}

#[test]
fn hostile_input_never_panics_or_goes_non_finite() {
    let mut px = vec![f32::NAN, f32::INFINITY, -1.0, 0.0, 1e30, -0.0];
    let a = analyze(&px, 2, 1, &Params::default());
    let mut p = Params::from_json(&json!({"_analysis": a.to_json(), "exposure": 5, "contrast": 50, "softHighs": true}));
    convert(&mut px, &p);
    assert!(px.iter().all(|v| v.is_finite()));
    p.analysis = Analysis::from_json(&json!({"version": 2, "lo": [], "hi": [[1]], "gain": [0, 1, 1e9]}));
    assert!(p.analysis.is_none());
    assert_eq!(analyze(&[], 0, 0, &p), Analysis::unmeasured());
    let _ = analyze(&[0.5; 3], 5, 5, &p); // short buffer
}

#[test]
fn auto_exposure_lifts_a_dark_frame_and_leaves_a_normal_one() {
    // a frame that is mostly deep shadow (low density on the negative) with a few bright pixels
    let (w, h) = (100, 100);
    let base = [0.25f32, 0.55, 0.85];
    let dark: Vec<f32> = (0..w * h)
        .flat_map(|i| {
            let e = if i % 10 == 0 { 3.0 } else { 0.3 + 0.2 * ((i % 7) as f32 / 7.0) };
            (0..3).map(move |c| 10f32.powf(-(base[c] + 0.65 * e)))
        })
        .collect();
    let median = |auto: bool, px: &[f32], border: f32| {
        let mut px = px.to_vec();
        let p = measured(&px, w, h, json!({"autoExposure": auto, "borderBuffer": border}));
        convert(&mut px, &p);
        let mut g: Vec<f32> = px.as_chunks::<3>().0.iter().map(|c| c[1]).collect();
        g.sort_by(f32::total_cmp);
        g[g.len() / 2]
    };
    assert!(median(true, &dark, 0.0) > median(false, &dark, 0.0) * 1.5, "a dark frame should be lifted");
    // the grey ramp, its clear film border outside the border buffer
    let normal = negative(w, h);
    assert!((median(true, &normal, 10.0) - median(false, &normal, 10.0)).abs() < 1e-4, "a normal frame should be left alone");
}

#[test]
fn a_slide_keeps_its_orientation() {
    // a positive: bright on the left, dark on the right
    let (w, h) = (100, 10);
    let mut px: Vec<f32> = (0..w * h).flat_map(|i| [1.0 - (i % w) as f32 / w as f32 * 0.99; 3]).collect();
    let p = measured(&px, w, h, json!({"filmType": "Positive (Slide)", "borderBuffer": 0}));
    convert(&mut px, &p);
    assert!(px[3] > px[(w - 2) * 3]);
}
