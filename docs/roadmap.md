# Repository goal

Build an Apple-inspired component library for GPUI, with macOS desktop behavior as the primary acceptance target. Keep renderer-independent values and motion parameters in `cupertino`. Keep rendering, retained component state, input, focus, accessibility, and public platform integration in `gpui-cupertino`.

The completed library must provide a consistent theme; controls; text input; navigation and layout; lists and tables; menus and floating panels; materials; and continuous motion. Reuse GPUI primitives and public platform capabilities. Keep the measured material regressions intact and document the actual support of each platform and material.

Acceptance must include three runnable compositions: a settings form, a searchable list with multiple selection, and a detail panel. Verify keyboard operation, Chinese input, focus, drag and drop, accessibility, rendering, and performance. Provide API documentation and a reproducible development check. Native behavior must have native evidence; simulated tests alone do not establish platform parity.

## Milestones

| Milestone | Scope | Status |
| --- | --- | --- |
| Rendering and motion foundations | Gaussian materials, inactive Clear on Apple GPUs, continuous outlines, accessibility fallbacks, GPUI spring integration | Implemented with scoped material and motion regressions. |
| First component foundation | Semantic light/dark palette, Button, single-line TextInput, nonmodal Popover, settings showcase, accessibility test support | Implemented; native Chinese IME and VoiceOver acceptance remains. |
| Form controls | Toggle, Checkbox, RadioGroup, Slider, Stepper, SegmentedControl, progress and loading states | Planned. Reuse the theme, focus, input, and event contracts. |
| Navigation and layout | Form groups, toolbar, sidebar, tabs, split view, scrolling, empty states | Form groups, toolbar, scrolling, and empty states are implemented. Sidebar, tabs, and split view are in progress. |
| Collections | Virtual lists, selection, search, tables, sorting, resizing, drag and drop | Planned. Reuse GPUI list and drag primitives. |
| Menus and panels | Menus, tooltips, dialogs, sheets, nested floating panels | Planned. Resolve dismissal, focus, and stacking ownership first. |
| Integrated acceptance | Settings form, searchable multiple-selection list, detail panel, native input/accessibility checks, performance baselines | Initial settings showcase exists. The remaining scenarios and release checks are pending. |

The first component milestone does not complete the repository goal. Password fields, rich text, nested popovers, and unmeasured cross-platform parity are not implied by the current APIs.

## Parallel development

Use one integration owner and separate worktrees for component owners. Assign each owner a concrete behavior, owned paths, shared API dependencies, and a passing check. Freeze shared APIs before consumers implement against those APIs. Share implementation through reviewed commits.

Keep one owner for shared theme, focus, overlay, or input changes during each milestone. Component owners must request the shared change instead of introducing a second mechanism. The integration owner must resolve shared-file conflicts, compile the combined showcase, run `devenv test`, and update the public contracts.

Run independent code inspection and component simulation tests in parallel. Share the Cargo target cache when disk use matters; Cargo serializes competing builds. Reserve one owner for native windows, screenshots, GPU calibration, or input-device automation. Avoid simultaneous edits in the integration worktree.

Use the existing formatter, Clippy configuration, devenv environment, and workspace tests. Do not add a second task framework or separate test runner. Add a new shared facility only when a component check demonstrates the need. The accessibility TestWindow hooks and pending-key cancellation regression are examples from the first milestone.
