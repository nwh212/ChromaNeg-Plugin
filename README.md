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
| Convert | Type (Color Negative, B&W Negative, Positive), Look, Film stock, Pre-saturation 1–5, Border buffer % |
| Tone | Tone profile, Exposure, Brightness, Contrast, Lights, Darks, Whites, Blacks, Fade, Soft highs, Soft lows, Auto exposure, White clip, Black clip |
| Colour | WB (Auto-Neutral, Auto-Color, Auto-Warm, Auto-Cool, None), Temp, Tint |

**Auto exposure** (off by default) lifts a dark frame's median brightness toward mid grey, by at most
2 stops, never darker, and rolls the highlights off so a bright sky doesn't clip. It needs a measurement.

**WB modes.** *Auto-Neutral* (the default) balances the dye layers from the edges in the frame and
neutralises each channel's black (see below). *Auto-Color* stretches each channel between its own darkest and brightest values:
stronger, but a large or very saturated object (blue sky, a red sign) can tilt the result.
*Warm* and *Cool* are Auto-Neutral with a bias. *None* only removes the film base.

**Film stocks** gently correct a stock's known tendencies. Auto-Neutral already removes a frame's
overall cast, so a stock corrects what it can't: dye cross-talk (stronger for films without an
orange mask), contrast and the shadow toe, casts that differ between shadows and highlights, and
saturation by hue. *Generic* is the former "Basic" colour model and *None (as scanned)* the former "None".
Stocks are corrections; Looks are creative, and the two combine. Measure already maps each frame's
own black and white, so stocks barely touch contrast: a "contrasty" or "flat" stock is mostly
corrected by the measurement (v0.4's larger contrast values double-corrected, washing Phoenix out).

| Stock | Reported tendency | Correction |
|---|---|---|
| Kodak Portra 160 | flat, pastel, low saturation; slight magenta-blue bias | a little more contrast and saturation, tint toward green |
| Kodak Portra 400 | natural, low saturation; slight magenta-blue bias | tint toward green |
| Kodak Portra 800 | warmer, more saturated and contrasty than 160/400 | softer, cooler highlights |
| Kodak Ektar 100 | the most saturated C-41 stock; warm orange lean; reddish skin | less red and orange, slightly less saturation and warmth |
| Kodak Gold 200 | warm, reds and yellows pushed, contrasty | less red and yellow, cooler, softer |
| Kodak UltraMax 400 | saturated, punchier than Portra | slightly less saturation and contrast |
| Kodak ColorPlus 200 | soft colour, flat in dull light | a little more contrast and saturation |
| Kodak Vision3 50D / 250D / 500T | cine stocks, flat by design (made to be graded) | more contrast and saturation |
| CineStill 50D / 800T | Vision3 without remjet: flat-ish, red halation | more contrast (halation is optical and can't be undone per pixel) |
| CineStill 400D | medium contrast, occasional mint-green cast | tint away from green, mostly in the shadows |
| Fujifilm Superia X-TRA 400 | magenta casts reported in scans | tint toward green, less magenta |
| Fujicolor C200 | cool, green-leaning | slightly warmer, tint away from green |
| Fujifilm 200 (2022 on) | datasheet matches Kodak Gold 200 | corrected like Gold |
| Fujifilm 400 (US, 2023 on) | widely thought to be UltraMax-like (unconfirmed) | corrected like UltraMax |
| Fujifilm Pro 400H | fourth colour layer; pastel, greenish-blue cast | tint away from green, cooler greens and cyans |
| Harman Phoenix 200 | **no orange mask**, limited antihalation; high contrast, shadows fall off fast; bold reds and blues, weak yellows and skin; warm highlights over cold greenish shadows | stronger cross-talk correction, a little less red/blue and more yellow, a slight opposite split cast, cooler (its contrast is handled by Measure) |
| Harman Phoenix II | **no orange mask**; more normal contrast and colour than Phoenix I, cooler blues and greens, shadows still fall off fast | stronger cross-talk correction, slightly warmer |
| Lomography 400 / 800 | 400: balanced, slightly muted; 800: medium-high saturation | 400: a little more saturation; 800: slightly less |

These are starting points from published descriptions, not measurements: the datasheets' curves
and spectral dye densities are figures, and reviews depend on lab and scanner. Tune them against
your own scans. Sources:
- Masking chemistry: [C-41 process](https://en.wikipedia.org/wiki/C-41_process); Brian Pritchard,
  [Why colour negatives are orange](https://brianpritchard.com/wp/motion-picture-technology/__trashed-6/).
- Kodak: [technical documents index](https://125px.com/techdocs/kodak/) (Portra 400 E-4050, Ektar
  100 E-4046, Gold 200 E-7022, UltraMax 400 E-7023); [Ektar vs Portra](https://gridfiti.com/ektar-100-vs-portra);
  [Ektar 100 test](https://www.adorama.com/alc/product-review-kodak-ektar-100-120-roll-film-test);
  [Portra 160/400/800](https://www.l-camera-forum.com/topic/335509-portra-160-400-800-differences);
  [Gold 200](https://www.focusnordic.com/product-reviews/kodak-gold-200-review-by-bjorn-joachimsen/);
  [UltraMax 400](https://www.digitalcameraworld.com/cameras/film-cameras/kodak-ultramax-400-film-review);
  [UltraMax vs Gold](https://shootitwithfilm.com/film-comparison-kodak-ultramax-400-vs-kodak-gold-200-pushed-one-stop/);
  [ColorPlus 200](https://www.analog.cafe/r/kodak-colorplus-200-film-review-fygr);
  [Vision3 250D](https://www.35mmc.com/03/07/2019/c41-or-ecn2-to-process-kodak-vision-3-250d-movie-colour-negative-a-quick-comparison-by-phil-harrison).
- CineStill: [800T](https://brooklynfilmcamera.com/blogs/guides/cinestill-800t-review-the-night-photography-film),
  [400D](https://www.japancamerahunter.com/2022/10/film-review-cinestill-400d/).
- Fujifilm: [Superia](https://en.wikipedia.org/wiki/Fujifilm_Superia);
  [Superia X-TRA 400](https://www.analog.cafe/r/fujifilm-superia-x-tra-400-film-review-zc0y);
  [Pro 400H](https://analog.cafe/r/fujifilm-fujicolor-pro-400h-film-review-rau5);
  [Fujicolor 200 and Gold 200](https://petapixel.com/2022/01/10/fujifilms-new-fujicolor-200-looks-to-be-kodak-gold-200-in-disguise);
  [Fujifilm 400 and UltraMax](https://www.35mmc.com/11/07/2023/fujifilm-400-made-in-the-usa-vs-kodak-ultramax-400-the-same-or-different/).
- Harman: [Phoenix 200 store page](https://cinestillfilm.com/products/phoenix200-color-negative-film-35mm);
  [Phoenix 200 review](https://casualphotophile.com/2023/12/01/harman-phoenix-200-the-review/);
  [Phoenix 200 slide test](https://www.photrio.com/forum/threads/slide-with-harman-phoenix-200.212735/post-2882422);
  [Phoenix II release](https://kosmofoto.com/2025/07/harman-technology-releases-phoenix-ii-colour-negative-film/);
  [Phoenix II review](https://bluemooncameracodex.com/film-fridays/harman-phoenix-ii).
- Lomography: [Color 400](https://parallaxphotographic.coop/lomography-color-negative-400-film-review/),
  [Color 800](https://www.analog.cafe/r/lomography-color-negative-800-film-review-y45b).

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

**Tone profiles** turn the normalised value `t` into light. *Standard* was fitted to a reference
conversion of the same scan (Negative Lab Pro, its standard settings): linear light ≈ `t`^1.5, with the
top rolled off instead of clipped. The others vary it: Soft, Hard; Highlight Soft / Hard and Shadow
Soft / Hard (shoulder or toe only); Cine Log (flat and lifted, for grading), Cine Rich, Cine Flat;
Linear (light = `t`), Linear Gamma (brighter mids), Linear Deep (darker), Linear Flat (brighter).

**Sharpening** is not part of this filter. A filter that reads neighbouring pixels makes the host
run it on the whole frame at full resolution after every parameter change, which is slow in the
plug-in interpreter. Use Develop ▸ Detail ▸ Sharpening on the converted image instead.

## How it works

Sources: own design on textbook film sensitometry; colour balance from the grey-edge hypothesis
(van de Weijer, Gevers, Gijsenij, "Edge-Based Color Constancy", IEEE TIP 2007), applied to densities.
Details are in `src/convert.rs`. In short, everything works in density (`−log10` of the linear sample):

1. **Base:** the film base (orange mask plus fog) is removed as a per-channel density offset.
2. **Balance:** most density changes across edges are changes of light, not of colour, so the
   channels' edge strengths give each dye layer's contrast relative to green. Only edges between
   mid-tone pixels count (green density between its 10th and 80th percentiles): a large bright wall
   or a dark area, where a scan's densities are least reliable, would otherwise tilt the balance.
3. **Ends:** each channel's black is its own darkest balanced density, so the shadows come out
   neutral. White comes from the brightness histogram, shared by the channels, so no single channel
   or object sets it. The brightest 5 % of the frame is then neutralised when it's nearly neutral
   already (a white wall), and left alone when it's clearly coloured, with a fade between. White /
   Black clip choose which stored percentile is the end: at 0, 5 % (white) and 3 % (black); each
   step moves along 0, 0.1, 0.25, 0.5, 1, 1.5, 2, 3, 5, 8, 12 %.
4. The normalised value goes through the look and the tone profile, then into light.

**Checked against references.** Two scans also converted with Negative Lab Pro, compared over the whole
frame (mean difference in levels of 255, from 8-bit exports of the negatives):

| Frame | v0.4 | v0.5 | v0.6 |
|---|---|---|---|
| Colour negative portrait, indoors | 19.6 | 9.9 | 8.9 |
| Harman Phoenix 200 landscape, flatbed scan (Generic) | — | 14.7 (in the app) | 9.1 |
| the same with the Phoenix 200 stock | — | 35.6 (in the app) | 9.3 |

A third frame (street, daylight) has no reference; v0.6 renders it within a few levels of v0.5.

Speed: the logarithm and the tone curve are lookup tables. In the app's interpreter, a
2.7 MP render takes about 1–2 s (v0.1: 3.6 s), and Measure about 0.5 s: it reads at most 300 000
pixels, with each density computed once.

## Not done yet

- **Tuning on real RAW captures.** The fit used an 8-bit export of the negative, whose display tone
  curve bends the densities, so the in-app result (from the linear RAW) can differ slightly. More
  reference pairs (your scan + a conversion you like) will tighten it.
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
