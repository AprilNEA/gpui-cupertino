# Split view

Call `gpui_cupertino::init(cx)` once before creating application windows to register component key bindings. `SplitView::new(id, label, first, second)` renders two panes with a movable divider. Panes are horizontal by default. Call `.vertical()` to place the first pane above the second. The accessible label names the divider.

The controlled ratio describes the first pane's share of the space remaining after the `8 px` divider. The default ratio is `0.5`, with bounds `0.1..=0.9` and a `0.01` step. Set `.ratio(value)` and retain `on_change` requests in the parent. The view does not accept a request until the parent renders the requested ratio.

Pass a validated `ValueRange` to `.range` to change the bounds and keyboard step. Fractional bounds must be within `0..=1`; invalid bounds return `ValueRangeError::FractionBounds`. `.ratio` rejects nonfinite values and snaps finite values to the configured bounds and step. An explicit zero or one endpoint allows a pane to collapse.

Pane wrappers have zero minimum size and clip overflowing content. Children retain their own minimum sizes inside those wrappers. A collapsed pane does not render its child content or register child focus and accessibility nodes. Keep stateful child entities in the parent to preserve their values while collapsed. If a programmatic collapse removes the focused pane, focus moves to the divider, or clears when the divider is disabled.

The divider supports pointer dragging outside its bounds, arrow stepping, Home and End, and accessibility increment, decrement, and numeric `SetValue` actions. Divider orientation describes the separator: horizontal panes expose a vertical splitter. Disabling or changing orientation cancels a drag. Disabling the divider leaves pane content interactive.

Run `cargo test -p gpui-cupertino --test split_view --test value_controls --lib --locked` in the existing devenv shell. Tests verify divider geometry after orientation and size changes, clipping layout under an oversized child, outside dragging, controlled callbacks, keyboard and accessibility operations, disabled cancellation, and collapsed-pane focus removal. Native VoiceOver and pointer-device acceptance remain separate checks.
