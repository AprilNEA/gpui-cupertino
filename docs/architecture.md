# Repository layout

`gpui-cupertino` depends on `cupertino`. The core crate stays independent of GPUI,
window systems, GPU resources, and platform APIs.

| Path | Responsibility |
| --- | --- |
| `crates/cupertino/src/materials.rs` | Validated, renderer-independent material descriptions. |
| `crates/cupertino/src/motion.rs` | Validated spring parameters and presets; no animation scheduler. |
| `crates/gpui-cupertino/src/theme.rs` | Semantic colors resolved from the GPUI window appearance. |
| `crates/gpui-cupertino/src/components/` | Controls with retained input, focus, and accessibility behavior. |
| `crates/gpui-cupertino/src/components/layout.rs` | Form composition, toolbars, and empty states. |
| `crates/gpui-cupertino/src/components/scroll_area.rs` | Scroll viewport, retained offset, and keyboard/accessibility scrolling. |
| `crates/gpui-cupertino/src/components/choice_group.rs` | Shared option identity and keyboard selection for radio groups, segments, tabs, and sidebars. |
| `crates/gpui-cupertino/src/components/numeric_controls.rs` | Shared bounded numeric input and pointer capture for sliders, steppers, and split dividers. |
| `crates/gpui-cupertino/src/components/panel_state.rs` | Window-local panel ordering, dismissal, and focus restoration. |
| `crates/gpui-cupertino/src/components/collections.rs` | Stable row data, selection, filtering, sorting, and reordering for virtual lists and tables. |
| `crates/gpui-cupertino/src/materials.rs` | Glass element, GPUI paint boundary, opaque accessibility fallback. |
| `crates/gpui-cupertino/src/motion.rs` | Convert spring parameters to GPUI's existing spring animation. |
| `crates/gpui-cupertino/src/platform.rs` | Public AppKit accessibility preference observation. |
| `crates/gpui-cupertino/examples/` | Runnable interaction and material checks. |
| `crates/gpui-cupertino/tests/` | Interaction, accessibility, scene ordering, and actual Metal output regressions. |
| `vendor/crates/gpui/src/platform/test/` | Simulated window input and accessibility dispatch for component checks. |
| `vendor/crates/gpui/src/scene/` | Ordered backdrop primitives and batching. |
| `vendor/crates/gpui_apple/src/metal_renderer/` | Independent material shader and Metal passes. |
| `docs/` | Public architecture and implementation contracts. |
| `internal-docs/` | Ignored local research; never part of the source distribution. |

Components share the GPUI theme and reuse GPUI event dispatch, focus handles,
anchored layout, deferred painting, and platform input handlers. Add core design values only when a
framework-independent consumer needs them. Keep component state, input, focus,
and accessibility behavior beside the component. See
[component contracts](components.md) and [the roadmap](roadmap.md).

The vendored GPUI dependency closure is a separate workspace, pinned to the
revision recorded in `vendor/README.md`. Alloy patches cover the rendering and
motion integration, component test hooks, and cancellation of pending keyboard
activation when click listeners are removed. The inert subtree boundary preserves layout and paint while excluding background input and accessibility for modal panels. Cupertino's root checks exercise
these integrations without running
unrelated upstream application examples and tests.

Keep the generated vendor sources unchanged. Follow the [Alloy maintenance workflow](upstream-maintenance.md) before importing a new snapshot.

Use `name.rs` for implementations and `name/mod.rs` for namespace shells. Put
meaningful unit tests beside their logic and integration checks under `tests/`.
Run `devenv test` before committing; CI uses the same command.
