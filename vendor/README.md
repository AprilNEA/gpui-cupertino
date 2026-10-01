# GPUI source

This directory contains the GPUI dependency closure from
`zed-industries/zed` at `4c841aaf1c4fa613e89a5d77096523d0ff593b56`.
Upstream licenses are retained. The workspace manifest retains the inherited
dependency versions and lint settings for these crates.

The local changes add ordered backdrop boundaries to GPUI and an independently
written Metal material implementation to `gpui_apple`. Spring animation uses
GPUI's scheduler clock, enabling deterministic retargeting and frame-cadence
checks. A separate macOS continuous-quad batch shares glass geometry with opaque
accessibility surfaces and preserves the ordinary quad buffer layout. Inactive
Clear uses an encoded-RGB mip filter and public ColorSync parameters for live
layer/display conversion. Test-only scene accessors and a ColorSync reference
check support adapter acceptance. Other source files are
kept at the pinned revision apart from whitespace cleanup. This source workspace is excluded from Cupertino's
workspace so its upstream examples and tests are not library release targets.

The source is included because the background effect requires renderer and
scene changes not exposed by the upstream public API. Do not substitute an
unpatched GPUI release. No Apple private resources or shaders are included.
