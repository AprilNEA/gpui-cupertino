# Repository layout

`gpui-cupertino` depends on `cupertino`. The core crate stays independent of GPUI,
window systems, GPU resources, and platform APIs.

| Path | Responsibility |
| --- | --- |
| `crates/cupertino/src/materials.rs` | Validated, renderer-independent material descriptions. |
| `crates/cupertino/src/motion.rs` | Validated spring parameters and presets; no animation scheduler. |
| `crates/gpui-cupertino/src/materials.rs` | Glass element, GPUI paint boundary, opaque accessibility fallback. |
| `crates/gpui-cupertino/src/motion.rs` | Convert spring parameters to GPUI's existing spring animation. |
| `crates/gpui-cupertino/src/theme.rs` | Semantic light and dark colors shared by controls. |
| `crates/gpui-cupertino/src/components/` | Reusable controls with GPUI input, focus, and accessibility behavior. |
| `crates/gpui-cupertino/src/platform.rs` | Public AppKit accessibility preference observation. |
| `crates/gpui-cupertino/examples/` | Runnable interaction and material checks. |
| `crates/gpui-cupertino/tests/` | Scene ordering and actual Metal output regression tests. |
| `vendor/crates/gpui/src/scene/` | Ordered backdrop primitives and batching. |
| `vendor/crates/gpui_apple/src/metal_renderer/` | Independent material shader and Metal passes. |
| `docs/` | Public architecture and implementation contracts. |
| `internal-docs/` | Ignored local research; never part of the source distribution. |

The remaining core `design/` placeholder does not expose APIs.
Add shared modules when actual components need them. Keep component state,
input, focus, and accessibility behavior beside the component.

The vendored GPUI dependency closure is a separate workspace, pinned to the
revision recorded in `vendor/README.md`. Only the backdrop integration changes
its behavior. Cupertino's root checks exercise this integration without running
unrelated upstream application examples and tests.

Use `name.rs` for implementations and `name/mod.rs` for namespace shells. Put
meaningful unit tests beside their logic and integration checks under `tests/`.
Run `devenv test` before committing; CI uses the same command.
