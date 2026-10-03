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
check support adapter acceptance.

`TestWindow` also retains accessibility callbacks and tree updates. Component tests use these callbacks to activate accessibility and dispatch actions through GPUI's platform path.

GPUI clears pending keyboard activation when a control removes its primary click listeners. GPUI cancels a pending pointer press when no click, auxiliary click, or drag handler remains. This prevents a press from surviving a disabled interval and activating after the control is enabled again.

`gpui::inert` keeps a subtree's layout and painting while excluding input handlers, focus targets, platform text input, and accessibility nodes. Modal panels use this boundary for background content. Deferred descendants preserve the boundary. Cached views rebuild registrations when the boundary changes. Application-global actions and persistent subscriptions remain outside the boundary.

`Div::autoscroll_on_focus` lets ScrollArea reveal newly focused descendants through a tracked ScrollHandle. A private scoped target keeps nested offsets consistent without consuming List's existing autoscroll requests. Geometry settles on the next frame, and wheel scrolling remains free until focus changes again. Inert and deferred descendants do not scroll the background.

Keystroke interception stops dispatch before raw capture handlers run. Tooltip dismissal uses this boundary when the pointer hovers a hint whose trigger does not own keyboard focus.

Other source files are kept at the pinned revision apart from whitespace cleanup. This source workspace is excluded from Cupertino's workspace so its upstream examples and tests are not library release targets.

The source is included because the background effect requires renderer and
scene changes not exposed by the upstream public API. Do not substitute an
unpatched GPUI release. No Apple private resources or shaders are included.
