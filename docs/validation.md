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

All four probe options are required. Backgrounds are `solid:RRGGBB`, `step:h`,
`step:v`, `ramp`, or `checker:N[:phase]`. Shapes are `roundrect`, `capsule`, and
`circle`; appearances are `light`/`dark`; styles are `regular`/`clear`. The ramp
encodes x in red, y in green, and constant 128 in blue. Checker cells and phase
use logical pixels. Each case writes the exact input PNG, two captures, and JSON
containing geometry, scale, and the measured title-bar offset. State checks apply
to every capture. Launch activation is drained before setting the inactive
measurement state; checks are never bypassed when the state is wrong.

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

All sampled widths were at least their heights; these observations cannot yet
distinguish height from minimum-dimension dependence. This is a measured range,
not a universal interpolation rule. The inactive
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

Coordinate ramps also showed a slope reversal in the inner 2–10 logical-pixel
edge band, while the central slope remained positive. The reversed region moved
with the circle boundary; this cannot be explained by one position-independent
color mapping. It is evidence of spatial optical behavior, but the unresolved
RGB response prevents a reliable conversion to source displacement in pixels.

No private filter calls, system shader binaries, or host recipes are used by the
library. Local inspection tools, raw logs, and provenance remain in the ignored
research directory. Mathematical fitting is pending a SciPy-enabled analysis
environment; acquisition and fixed-formula verification are independent of it.
Active-window inspection was attempted but rejected by its state assertions;
the graphical session was independently confirmed locked. No active-state
parameter result is claimed from those attempts.
