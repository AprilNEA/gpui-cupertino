# Foundation validation

Measured on macOS 26.4 (25E246), Apple M5 Max, on 2026-09-28. These are
independent numerical and behavioral acceptance checks. They do not establish
visual equivalence with Apple's native material styles.

## Independent rendering reference

The CPU reference integrates a continuous Gaussian using Simpson quadrature.
It shares no kernel weights, sampling strides, scale interpolation, or filtering
implementation with the Metal path. Captured pixels are constant unit cells;
convolution is sampled at destination pixel centers. The production filter uses
`libm::erf`, normalized four-sigma support, and paired linear texture sampling.

Coverage is checked against intersected pixel-cell area on straight edges.
Composition is checked analytically in linear RGB, including saturation, tint,
brightness, and ancestor opacity. Thresholds were fixed before the initial run
and were retained when the initial implementation failed.

| Worst observed metric | Initial implementation | Corrected implementation | Acceptance limit |
| --- | ---: | ---: | ---: |
| Gaussian energy error | 0.425% | 0.238% | 1.5% |
| Gaussian centroid drift | 0 px | 0 px | 0.05 px |
| Gaussian variance error | 21.894% | 0.694% | 5% |
| Normalized kernel L1 distance | 0.38404 | 0.01691 | 0.10 |
| Step maximum linear-light error | 0.08247 | 0.00435 | 0.015 |
| Fractional-edge error, encoded channel levels | 188 | 0 | 1 |
| Color/opacity error, encoded channel levels | 0 | 0 | 1 |

Variance is compared with the integrated reference's discrete second moment,
which includes the pixel-cell convention; it is not compared directly with
sigma squared. The matrix covers sigma 0.5/1/2/4/8 logical pixels, 1×/2×,
both axes, two odd extents, and three translated impulses. All 197 reported
metric lines matched across three independent runs. The exported six profiles
contained 1,542 rows and were byte-identical across those runs.

A separate refraction-coordinate oracle uses quantized linear-light ramps.
It checks four straight-edge normals, signed displacements, two optical widths,
and 1×/2× scaling. All 160 sampled coordinates agree within one encoded channel
level, below the preselected two-level limit. This verifies the coordinate
mapping, not whether its profile matches an undocumented Apple style.

The comparisons exposed and fixed:

- Small-radius and kernel-shape errors from blending Gaussian scale levels.
  Uniform blur now evaluates its requested kernel directly.
- Lost fractional-pixel coverage caused by rasterizing only the geometric
  rectangle. Raster and scene bounds now include the AA fringe while preserving
  output clipping and rejecting empty geometry.
- A spring clock bypass: the retained spring element used wall time instead of
  GPUI's scheduler clock, preventing deterministic frame progression in tests.

## Actual element and motion checks

The adapter tests execute GPUI layout, prepaint, and paint. They verify that
background precedes capture and content follows it, and exercise parent clipping
and opacity, all three outlines, both scales, and opaque/contrast fallbacks.
System preferences are injected into the cached application state for these
checks; the tests do not mutate macOS settings or claim to test delivery of a
real AppKit notification.

An additional full-image check exercises two overlapping tinted materials with
ancestor opacity, in both paint orders at 1×/2×. It changes the background A→B→A
while reusing the renderer and cached paint operations. Actual `Scene::replay`
through nested layers must match fresh rendering byte-for-byte, and each pixel
must agree with independently calculated sequential linear-light composition
within two encoded levels.

Retained GPUI spring tests use scheduler seeds 0, 1, and 42. They check position
continuity and forward momentum during reversal, stopping frame requests after
settling, mid-flight reduced-motion changes, and resuming with no stale velocity.
Common-time positions at 60/120 Hz, including a dropped frame, differ by less
than 0.05 logical pixels. The resumed trajectory agrees with a fresh spring
within 0.01 logical pixels.

## Performance sample

An optimized build rendered 20 measured frames after three warmups. Temporary
instrumentation read public Metal command-buffer GPU start/end timestamps after
completion; it was removed after collection. Times include every pass in that
command buffer, not just the glass fragment shader. Initialization took 21.862 ms
and may benefit from driver caches.

| Device size / sigma | Effects | GPU median / p95 | Submission + GPU + readback median / p95 |
| --- | ---: | ---: | ---: |
| 1024×768 / 6 px | 0 | 0.098 / 0.131 ms | 1.112 / 1.291 ms |
| 1024×768 / 6 px | 1 | 0.688 / 1.079 ms | 2.079 / 2.473 ms |
| 1024×768 / 6 px | 2 | 1.780 / 2.774 ms | 3.297 / 4.337 ms |
| 2048×1536 / 12 px | 0 | 0.228 / 0.942 ms | 4.490 / 5.374 ms |
| 2048×1536 / 12 px | 1 | 3.527 / 4.098 ms | 8.320 / 9.089 ms |
| 2048×1536 / 12 px | 2 | 6.043 / 8.291 ms | 12.777 / 23.440 ms |

This short sample is not a sustained frame-rate guarantee. Two Retina effects
already approach the 8.33 ms budget for 120 Hz before other UI and display work.
Full-window filtering remains the primary optimization target. Cropping or
sharing captures must preserve the current numerical and paint-order checks.

## Reproduction and native comparison

```sh
devenv test
devenv shell -- cargo test -p gpui-cupertino --test numeric_reference --locked -- --nocapture
devenv shell -- cargo bench -p gpui-cupertino --bench backdrop --locked
```

Set `CUPERTINO_REFERENCE_OUTPUT` to an absolute CSV filename with an existing
parent directory to export the measured Gaussian curves from the numerical
check. The benchmark prints synchronous render/readback timing; the isolated
GPU measurements above used the temporary diagnostic instrumentation described
above.

On macOS 26 or later, with an **unlocked, visible desktop session** and screen
recording permission:

```sh
devenv shell -- cargo run -p gpui-cupertino --example native_compare --locked -- work/comparison
```

This creates an owned AppKit window: public `NSGlassEffectView` on the left,
Metal readback using demo calibration parameters on the right. Both receive the
same background, logical bounds, and corner-radius inputs. AppKit does not
promise the same contour or antialiasing as our analytic outlines. The example
captures light/dark, regular/clear, two background phases, and two repeated
snapshots of each case. It checks before and after every capture that the window
is visible, inactive, non-key, and at the original scale. Reduce Transparency
and Increase Contrast must both be disabled; the example checks these settings
without changing them. A failed state check invalidates that capture run.

## Native comparison results

Three independent process runs produced 48 captures at 2× on the system listed
above. All state checks passed. The initial exploratory run had a focus
transition and a background mismatch, so it is excluded from these results.
After fixing the capture state, all 252,232 far-background control pixels in
every capture matched exactly between the two halves.

The Metal half was pixel-identical in all 24 within-run repeat pairs and all 48
cross-run comparisons of matching cases/snapshots. Native halves were identical
in 19/24 and 30/48 comparisons respectively; every remaining channel difference
was at most one encoded level. Worst native mean difference was 0.029712 over the
content region and 0.083116 over the combined shape interiors. Thus the measured
style differences are substantially larger than the observed capture variation.

Each screenshot is 1536×696 pixels, with a 56-pixel title bar and two 768×640
content regions. Registration was checked against the displayed Metal source.
Screenshots contain the display ICC profile; source PNGs are untagged. Metrics
therefore compare left and right within the same screenshot, without treating
raw source-to-screenshot differences as shader errors.

The following ranges cover both background phases, both snapshots, and all three
runs. Values are mean absolute encoded RGB channel differences on a 0–255 scale,
averaged over pixels at least two device pixels inside each analytic shape.
These are diagnostic differences, not perceptual similarity scores or a newly
chosen pass threshold.

| Appearance / style | Rounded rectangle | Capsule | Circle |
| --- | ---: | ---: | ---: |
| Light / regular | 11.69–12.61 | 12.02–12.37 | 5.47–5.73 |
| Light / clear | 10.59–10.84 | 11.94–13.65 | 13.19–13.44 |
| Dark / regular | 6.47–6.63 | 6.53–7.05 | 5.87–5.87 |
| Dark / clear | 10.47–10.57 | 11.06–11.55 | 11.35–11.45 |

Visual inspection confirms different filtering and color response, with an
excessively bright directional highlight in the dark Metal examples. The demo
parameters have not been fitted to Apple's styles. These results **do not pass
native visual-equivalence acceptance**; the mathematical reference checks above
verify our documented primitives, not Apple's undocumented style parameters.

Native evidence is limited to stationary, inactive windows at 2×. It does not
cover active windows, 1× displays, hover/press, animation, or fusion. Three
neighboring shapes also do not isolate Apple's sampling/grouping behavior.
Calibration needs isolated-shape captures and separate fitting/validation
backgrounds before changing style presets; arbitrary tuning against one image
would not establish a reusable match.

Local captures and logs are in `work/comparison-inactive-{1,2,3}` and
`work/native-inactive-{1,2,3}.log`; diagnostic JSON files sit beside those
directories. `work/` is ignored and is not part of the distributed library.

## Isolated calibration probes

The comparison example also places a single native shape beside the same input
without glass. This separates color response, filtering, and displacement:

```sh
devenv shell -- cargo run -p gpui-cupertino --example native_compare --locked -- \
  work/probes probe --background step:v --shape roundrect --appearance light --style regular
```

All four probe options are required. Backgrounds are `solid:RRGGBB`, `step:h[:phase]`,
`step:v[:phase]`, `ramp`, or `checker:N[:phase]`. Shapes are `roundrect`, `capsule`, and
`circle`; appearances are `light`/`dark`; styles are `regular`/`clear`. The ramp
encodes x in red, y in green, and constant 128 in blue. Checker cells and phase
use integer logical pixels. Step phase is a signed offset from the panel center,
in half-logical-pixel increments; positive moves right/down. A step must fall
strictly inside the panel and exactly on a device-pixel boundary at the capture
scale. Optional `--size WIDTHxHEIGHT` centers integer dimensions within the
384×320 panel. Rounded rectangles use radius `min(20, short_side / 2)`, capsules
use `short_side / 2`, and circles require equal dimensions. Defaults remain
256×128 for rectangles/capsules and 128×128 for circles. Nondefault dimensions
and nonzero phases distinguish output names; equivalent default inputs retain
the original names.

Each case writes the exact input PNG, two captures, and JSON
containing geometry, scale, and the measured title-bar offset. State checks apply
to every capture. The example runs AppKit's application event loop; its delegate
schedules preparation, deactivation, and both captures on separate turns.
Deactivation has a five-second deadline. Checks before and after each screenshot
are never bypassed when the state is wrong.

The first isolation matrix contains 68 cases and 136 successful captures:
five grays, three primaries, two appearance-specific checker colors, two step
directions, a coordinate ramp, and two checker phases for each style/appearance;
capsule and circle ramps supplement the rounded rectangle. Phase 7 and the
checker colors must remain independent validation inputs when parameters are
identified from pure colors and steps.

The calibration target is matched-state output within the native repeatability
envelope (currently at most one encoded level per channel), including edges,
across independent inputs. The current renderer has not met that target. A
smaller average error on a training checkerboard is not sufficient acceptance.

Local read-only research also inspected actual objects attached to an owned
native view, rather than assuming that an on-disk recipe was selected. For an
inactive 256×128 logical-pixel rectangle with radius 20, the observed backdrop
capture scales were 0.25 for regular and 0.5 for clear. Their filter blur-radius
inputs were approximately 3.428571 and 10 respectively. These values are **not
Gaussian sigma**, and do not establish the final GPU buffer or sampler mapping.

Six geometry configurations showed that changing radius 20→64 at a fixed
256×128 size did not change those inputs. Changing the short dimension did:

| Short dimension | Regular blur-radius input | Clear blur-radius input |
| --- | ---: | ---: |
| 64 | 2.285714 | 6.111111 |
| 72 | 2.428571 | 6.666667 |
| 96 | 2.857143 | 8.333333 |
| 128 | 3.428571 | 10 |

A subsequent 32-case sweep included the transposed pair 128×96 and 96×128
with radius 20. Both produced regular blur-radius 2.857143 and clear 8.333333
in both appearances. This rejects a height-only rule and supports minimum-
dimension dependence in the sampled range; it does not establish a universal
interpolation rule. The inactive
key/fill highlight layer had zero opacity and zero color alpha, whereas the
demo used a nonzero directional highlight. The observed refraction-opacity
input of zero does not establish that all inner refraction is disabled: the
archived shader has a separate branch flag that has not been traced to final
uniforms.

Color-managed measurements found zero difference in every far-background
control region, and at most one encoded level of repeat noise. A proposed
per-channel formula using the observed white/black/fill inputs reproduced all
five grays within one level, but failed every colored validation input. Worst
channel errors were 43.125 (light regular), 85.55 (dark regular), 18.40 (light
clear), and 48.45 (dark clear). Applying that same formula in linear RGB was
worse. For example, light regular over pure green predicted [172,247,172] but
measured [129,255,139]. No preset was accepted from this falsified model.

Further measurements separated the transform from the output color conversion.
A fixed luminance/chroma formula, using the observed filter inputs and Rec.709
weights `(0.2126, 0.7152, 0.0722)`, predicted all 64 solid-color cases and both
captures within one encoded level. This includes 24 additional midtone cases;
no coefficients were fitted:

```text
luma = dot(weights, input)
face = input + ((white - black - 1) * luma + black)
output = clamp((1 - fill_alpha) * face + fill_alpha * fill_color, 0, 1)
```

The scalar adjustment in `face` is added to all three channels, preserving
color differences before fill. The inputs and outputs here are the original
captured **display-profile RGB code values**, with each capture's no-glass
control as input. Applying the earlier scalar contrast model after conversion
to sRGB tests a different hypothesis. Maximum channel errors for light regular,
dark regular, light clear, and dark clear were 0.565, 0.538408, 0.475, and
0.533513 levels respectively. Clamping `face` before fill was falsified by the
dark regular blue sample (12.25 levels).

This establishes a color-response model for the measured inactive state and
display profile. It does not establish the compositor's texture format, a
portable sRGB implementation, blur behavior, or a complete native preset.
AppKit independently decoded the untagged input PNGs as sRGB, matching explicit
sRGB `CGColor` backgrounds; the screen captures use the display's ICC profile.

Using independently captured black/white outputs as threshold endpoints,
the step probes gave the following 10–90% output linear-luminance widths. Both
axes and both repeats agreed at the displayed precision. These widths include
the native color response and must not be labeled Gaussian sigma.

| Appearance / style | Transition width, logical pixels |
| --- | ---: |
| Light / regular | 21.228 |
| Dark / regular | 18.912 |
| Light / clear | 47.858 |
| Dark / clear | 44.727 |

Repeating the thresholds in encoded RGB, which removes the measured gray
transfer from the width statistic, gave 10–90% widths of 21.125 / 21.250 logical
pixels for light/dark regular and 48.200 / 47.850 for light/dark clear. Horizontal
and vertical widths and repeats agreed. The ratio of 10–90% to 25–75% width was
2.055–2.099 for regular and 1.740–1.763 for clear, compared with approximately
1.900 for a Gaussian step response. These measurements do not establish a
Gaussian kernel; an equivalent sigma from one width alone would miss the
observed transition shape.

With the analysis environment available, a bounded least-squares comparison
trained one Gaussian CDF and one generalized-normal CDF per material on only
light/vertical/capture0. The window was fixed at ±40 logical pixels from the
step, at least 24 pixels from the nearest contour. Color coefficients remained
fixed. All other axis/appearance/repeat combinations were held out, with no
parameter changes after validation:

| Material | Response model | Holdout maximum code error | Holdout MAE |
| --- | --- | ---: | ---: |
| Regular | Gaussian | 1.735 | 0.484 |
| Regular | Generalized normal | 0.878 | 0.302 |
| Clear | Gaussian | 2.553 | 0.805 |
| Clear | Generalized normal | 2.489 | 0.807 |

Regular's generalized-normal response (`beta=1.454379`, `scale=9.697687`
logical pixels) stayed within one raw Display RGB code value on all seven
holdout profiles. Clear did not improve meaningfully; its residual structure
persisted while the sampled step strips repeated exactly. This evaluates the
fitted baselines, not the minimax feasibility of every Gaussian parameter set.
It is a one-dimensional inactive step-response model, not a recovered native
blur implementation or validation on other shapes, settings, or full images.

Clear's remaining residual led to a prospective phase/size experiment. All
models were frozen before new pixels were inspected: six vertical phases
(`0`, `0.5`, `1`, `1.5`, `2`, `3` logical pixels) at 256×128, horizontal phase 1
in light and dark appearance, and vertical phases 0/1 at 256×96. Each condition
was captured twice. The new phase-zero capture checks run consistency; it is
not a previously unseen condition. The complete batch used the corrected
AppKit application lifecycle described above. Forty window-state checks passed;
input/control step boundaries, capture dimensions, and ICC profiles agreed.
All ten repeated center strips were pixel-identical.

The fixed recursive filter candidate uses the axis projection of a 13-point
downsample kernel. Its five weights are
`[0.05633544921875, 0.244384765625, 0.3985595703125, 0.244384765625, 0.05633544921875]`
at offsets `[-4, -2, 0, 2, 4]` in the preceding level's texel coordinates.
Each destination sample is centered at source index `2*k + 0.5`, with linear
interpolation, implementing 2× downsampling. This candidate has **no fitted
parameters**. For one interior level-four output node, the recovered source-domain
kernel has unit mass, zero center offset, and variance 340.6640625 logical pixels
squared, matching an independent moment calculation.

The predeclared size rule uses effective pitch `1.6 * radius`: pitch 16 for
the measured radius input 10, and pitches 8/16 with upper weight 0.7369655942
for input 25/3 at height 96. The internal surface scale and actual mip index
remain unidentified; this is a tested logical-coordinate model. A single-grid
Gaussian and a Gaussian sampled at these mip levels were also frozen on the
old light/vertical/phase-zero capture. The baseline and single-grid sigma scale
with the radius input; the mip Gaussian's sigma scales with each level's pitch.
Color coefficients remained fixed:

| Response model | Maximum error, 256×128 | Maximum error, 256×96 | Profiles within one code |
| --- | ---: | ---: | ---: |
| Continuous Gaussian baseline | 2.532 | 3.353 | 0 / 20 |
| Single-grid Gaussian | 1.325 | 3.038 | 4 / 20 |
| Mip-level Gaussian | 1.325 | 1.139 | 6 / 20 |
| Fixed recursive filter | **0.946** | **0.870** | **20 / 20** |

Errors are maximum absolute single-channel Display RGB code differences;
raw-pixel maxima equal the mean-strip maxima. No model or threshold changed
after validation. The result supports this one-dimensional inactive Clear
response across the measured phases, axes, appearances, and two sizes. It does
not establish actual compute dispatch, a portable working color space,
two-dimensional separability, contour optics, or active-state equivalence. The
library's Gaussian contract is unchanged. Frozen models, prospective results,
and plots are in `work/clear-grid-models-v2-2026-09-29`; independent capture
checks are in `work/probes-phase-size-2026-09-29-validation.json`.

Coordinate ramps also showed a slope reversal in the inner 2–10 logical-pixel
edge band, while the central slope remained positive. The reversed region moved
with the circle boundary; this cannot be explained by one position-independent
color mapping. It is evidence of spatial optical behavior. The now-verified
color response permits an inverse check, but blur, clipped channels, and the
source/display conversion must still be accounted for before attributing a
pixel displacement to a particular optical parameter.

An inverse check of the fixed color formula across 600 ramp samples supports
the observed inner-refraction amount/height `-60 / 20` in logical pixels with
the candidate profile `1 - sqrt(t * (2 - t))`. At depths 2, 4, and 8 logical
pixels, predicted inward offsets were 33.90, 24.02, and 12.01; measured ranges
were 32.33–35.34, 22.56–25.57, and 10.53–14.29 across appearances and shapes.
No sampled output channel was clipped, and repeats were identical. Maximum
residuals were 2.542 logical pixels for regular and 3.295 for clear. Perturbing
the captured channels by one encoded level produced coordinate spans of
6.02–12.78 logical pixels after inversion and 8-bit ICC conversion. This is
support for the sign, scale, and decay of the candidate, not proof of exact
curve equality or an attribution of the remaining residual to one cause.

A same-view gray→green→gray sweep produced 12 valid inactive observations.
None of the 163 inspected filter/effect/layer fields changed in any of the four
style/appearance combinations. This supports stable host-side inputs for that
experiment; it does not prove that later GPU processing ignores the background.

The comparison's display path is also part of the calibration boundary. The
headless renderer exports untagged RGB PNG bytes, which AppKit interprets as
sRGB before display. GPUI creates a `BGRA8Unorm` `CAMetalLayer` without explicitly
setting its color space. Although Apple's documented creation default is `nil`
(no color matching), the **actual attached GPUIView layer** reported
`kCGColorSpaceSRGB` in the running example. The two measured display paths thus
both specify sRGB on this machine. Creation defaults alone would have led to
an incorrect conclusion; no renderer color-space change was needed.

The live inspection is repeatable and exits after reading the owned window:

```sh
devenv shell -- cargo run -p gpui-cupertino --example glass --locked -- --inspect-color-space
```

This inspects the renderer-owning `GPUIView` inside AppKit's content view,
checks that its layer is a `CAMetalLayer`, and prints its actual color-space
name. It neither sets the property nor assumes an expected value. A formula
verified in display RGB still cannot be copied directly into the renderer's
linear RGB stage: the source/display transform remains part of the model.
See [Apple's colorspace contract](https://developer.apple.com/documentation/quartzcore/cametallayer/colorspace).

No private filter calls, system shader binaries, or host recipes are used by the
library. Local inspection tools, raw logs, and provenance remain in the ignored
research directory. The repository's devenv includes Python with NumPy, SciPy,
Pillow/ImageCms, and Matplotlib for measurement, fitting, and plotting. Run local
analysis scripts with `devenv shell -- python3 work/<script>.py`; package versions
come from the pinned Nix inputs. The recorded fixed-formula results did not use
an optimizer; nonlinear fitting is no longer blocked on the analysis environment.
Active-state parameters were subsequently obtained with `active=true`,
`key=true`, and `visible=true` for all four styles/appearances at 256×128/r20.
Three valid cases came from one run; a separate single-case run supplied dark
clear after the earlier run lost focus. Failed state samples were excluded.
The differences are material: active regular enables refraction opacity 0.3,
bleed opacity approximately 0.3333333 (light) / 0.5333334 (dark), and shadow
opacity approximately 0.3214286 / 0.2928571. Active clear changes the blur-radius
input from 10 to 1, white/black/saturation to 1.15 / 0.075 / 1.06, and fill alpha
to zero; a holding-tone stage is also enabled. All active key/fill highlight
layers have opacity 1 and color alpha 1. The inactive color-model verification
must not be generalized to these active recipes without new pixel probes.
