# ChromaNeg: film negative conversion filter

A pixel-filter plug-in (PhotoCraft ABI v1 + the `pc_analyze` v1.1 proposal; see `docs/plugins.md`
▸ *Pixel filters*) that turns camera or scanner captures of film into positives. It follows the
usual two-step negative-conversion workflow: **analyze** the frame once, then every render
**converts** with that measurement.

## Build and install

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
# → target/wasm32-unknown-unknown/release/chromaneg.wasm (≈110 KB, no imports)
```

Install it from File ▸ Plug-ins ▸ Plug-in Manager… (or `<cli> commands plugin.install '{"path": "…/chromaneg.wasm"}'`).
Filters need no permissions.

## Use

1. **Capture:** expose so that no channel clips. The film base transmits a lot of red, so red clips
   first, and a clipped red channel comes out starved and blue. White-balance on the clear film
   border if you like, though the conversion removes the mask either way.
2. In Develop ▸ Plug-in Filters, add **ChromaNeg**. It asks for the **Source** stage, so it runs on
   the linear capture before every other develop step.
3. Set **Type** and **Border buffer %**, so the film holder, sprocket holes and edge print lie
   outside the measured area. Then press **Measure**. Until you measure, the filter only inverts
   the image, and the orange cast stays.
4. Pick a **Look** and a **Tone profile**, then fine-tune with the sliders or with the normal develop
   sliders, which act on the converted positive.
5. **A whole roll:** set up one frame (Measure, Look, sliders), then Copy and Paste (or Sync) its
   settings to the other frames. The filter, its settings *and its measurement* go along, so the roll
   converts consistently, like a lab's roll analysis. A frame that needs its own colour can be
   measured on its own afterwards (select it, Measure).

## Controls

| Group | Parameters |
|---|---|
| Convert | Type (Color Negative, B&W Negative, Positive), Look, Color model (Basic, None), Pre-saturation 1–5, Border buffer % |
| Tone | Tone profile, Exposure, Brightness, Contrast, Lights, Darks, Whites, Blacks, Fade, Soft highs, Soft lows, Auto exposure, White clip, Black clip |
| Colour | WB (Auto-Neutral, Auto-Color, Auto-Warm, Auto-Cool, None), Temp, Tint |

**Auto exposure** (on by default) lifts a dark frame's median brightness toward mid grey, by at most
about 2 stops, never darker, and rolls the highlights off so a bright sky doesn't clip. It needs a
measurement; turn it off to get the plain conversion.

**WB modes.** *Auto-Neutral* (the default) balances the dye layers from the edges in the frame and
neutralises each channel's black (see below). *Auto-Color* stretches each channel between its own darkest and brightest values:
stronger, but a large or very saturated object (blue sky, a red sign) can tilt the result.
*Warm* and *Cool* are Auto-Neutral with a bias. *None* only removes the film base.

**Looks** are offsets layered on the sliders, so every slider still starts at 0. Each is an own design:

| Look | Aims for |
|---|---|
| Standard | balanced, a touch of density |
| Neutral | as measured: no cross-talk correction, slightly muted |
| Rich | deeper colour and contrast |
| Clean Contrast, Clean Contrast + | clean whites and firm blacks, then more so |
| Golden Hour | consumer gold film: warm, yellow highlights, saturated |
| Portrait Warm | professional portrait film: low contrast, warm skin, gentle colour |
| Portrait Airy | the bright, lifted, pastel version |
| Fashion Cool | high-key fashion: cool, clean, contrasty |
| Crisp Scan | lab scanner, crisp: cyan-leaning blues, firm greens |
| Green Shadows | lab scanner, mellow: green-tinted shadows, warm highlights |
| Vivid Green | consumer film with lively greens and cool shadows |
| Night Cine | tungsten cine stock in daylight: teal shadows, warm highlights |
| Archive Print | optical print on archival paper: deep blacks, rich reds, warm |

**Tone profiles** are curves through black, mid grey and white with chosen slopes at each: Standard,
Soft, Hard; Highlight Soft / Hard and Shadow Soft / Hard (shoulder or toe only); Cine Log (very flat
and lifted, for grading), Cine Rich, Cine Flat; Linear, Linear Gamma (brighter mids), Linear Deep
(more stops, darker), Linear Flat (fewer stops, brighter).

**Sharpening** is not part of this filter. A filter that reads neighbouring pixels makes the host
run it on the whole frame at full resolution after every parameter change, which is slow in the
plug-in interpreter. Use Develop ▸ Detail ▸ Sharpening on the converted image instead.

## How it works

Sources: own design on textbook film sensitometry; colour balance from the grey-edge hypothesis
(van de Weijer, Gevers, Gijsenij, "Edge-Based Color Constancy", IEEE TIP 2007), applied to densities.
Details are in `src/convert.rs`. In short, everything works in density (`−log10` of the linear sample):

1. **Base:** the film base (orange mask plus fog) is removed as a per-channel density offset.
2. **Balance:** most density changes across edges are changes of light, not of colour, so the
   channels' mean edge strengths give each dye layer's contrast relative to green.
3. **Ends:** each channel's black is its own darkest balanced density, so the shadows come out
   neutral. White comes from the brightness histogram, shared by the channels, so no single channel
   or object sets it. White / Black clip choose which stored percentile is the end:
   0 → 0.1 %, +4 → 2 %, −2 → 0 %.
4. The normalised log exposure goes through the look and the tone profile, then back to linear light.

Speed: the logarithm and the tone curve are lookup tables. In the app's interpreter, a
2.7 MP render takes about 2 s (v0.1: 3.6 s), and Measure about 1 s: it reads at most 300 000 pixels, with each density computed once.

## Not done yet

- **Tuning on real RAW captures.** The defaults pass synthetic tests, but they've only been checked
  against an 8-bit export, whose display tone curve hides the red channel's highlights.
- **Roll analysis:** one shared measurement for a whole roll needs `_analysis` from several photos,
  which the ABI doesn't offer yet. The workaround is to measure a typical frame and copy the filter
  with its analysis.
- Scanner (gamma-encoded) input, per-stock film profiles, mids / highs / shadows colour balance, HSL, glow.

## Test

`cargo test` runs the maths natively:
- a synthetic orange-masked grey ramp comes out neutral and monotonic in each balance mode;
- a small saturated patch doesn't tint the neutrals;
- every look × profile renders finite values;
- the lookup tables match the exact functions;
- analysis JSON round-trips, and the manifest's menus match the code;
- hostile input never panics.

`cargo run --release --example develop -- in.f32 w h '{"borderBuffer":20}' out.f32` runs Measure and
a conversion on a raw linear `f32` image, for tuning by eye.
