# Repository layout

```text
.
├── crates/
│   ├── cupertino/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── design/
│   │       ├── materials/
│   │       └── motion/
│   └── gpui-cupertino/
│       ├── examples/
│       └── src/
│           ├── lib.rs
│           ├── components/
│           ├── theme/
│           ├── materials/
│           ├── motion/
│           └── platform/
├── docs/
├── .github/workflows/
├── Cargo.toml
└── devenv.nix
```

The feature directories are tracked with `.gitkeep` until their first
implementation. They are not yet Rust modules or public APIs. Remove each
placeholder when adding source files to its directory.

## Crate boundaries

`gpui-cupertino` depends on `cupertino`. The core crate must remain independent
of GPUI, window systems, GPU resources, and platform APIs.

| Directory | Responsibility |
| --- | --- |
| `cupertino/src/design/` | Framework-independent color, typography, spacing, and geometry values. |
| `cupertino/src/materials/` | Material descriptions and parameters; no rendering or texture ownership. |
| `cupertino/src/motion/` | Time-based interpolation, easing, and spring calculations; no frame scheduling. |
| `gpui-cupertino/src/components/` | Controls and containers, with their state, focus, input, and accessibility behavior. |
| `gpui-cupertino/src/theme/` | Resolve core design values into GPUI styles for appearance and control states. |
| `gpui-cupertino/src/materials/` | Material rendering and compositing through GPUI. |
| `gpui-cupertino/src/motion/` | Drive animations through GPUI updates and invalidation. |
| `gpui-cupertino/src/platform/` | Native integrations, isolated behind target-specific compilation. |
| `gpui-cupertino/examples/` | Runnable Cargo examples, including the component gallery when implemented. |
| `docs/` | Architecture and contributor-facing design documentation. |

Keep component-specific behavior beside its component. Move code into a shared
module when multiple implementations actually need it. Platform-specific code
does not belong in the core crate.

## Rust modules and checks

- Add module declarations with their first implementation, documenting public items.
- Use `name.rs` for implementations; use `name/mod.rs` for namespace shells.
- Keep unit tests beside meaningful logic. Add crate-local `tests/` for public API
  integration tests and `benches/` for implemented hot paths when needed.
- Add example assets beside the example that uses them.
- Run `devenv test` before committing; CI uses the same command.
