# Navigation

Call `gpui_cupertino::init(cx)` before creating windows. Navigation components use the shared `ChoiceOption` model and GPUI focus handles. Keep each option ID unique and stable when labels or order change.

`Tabs::new(id, label, selected, tabs)` renders a tab list and the selected panel. Construct each item with `Tab::new(id, label, content)`. Only selected content is mounted. Retain stateful entities in the parent when their state must survive unmounting. Each tab supplies a separate element identity for local child state, so equal child IDs in different tabs cannot share offsets or input state.

Selection is controlled: update the selected ID in `on_change` and notify the parent view. An unknown selected ID renders no panel. `Tab::disabled(true)` prevents user selection; a caller may still display that tab by selecting its ID programmatically.

Left and Right wrap through enabled tabs. Home and End select the first and last enabled tabs. The tab list has one Tab stop, and each enabled option supports accessibility activation. Selected content follows the tab list in ordinary focus traversal.

`Sidebar::new(id, label, options).selected(id)` provides single-selection navigation. Up and Down wrap through enabled items; Home and End reach the endpoints. The sidebar exposes a list box and selected options. Render its associated content separately and update the selected identity in `on_change`.

Use an ancestor key context for application navigation defaults. Component contexts take precedence over ancestor bindings. GPUI preserves explicit context-free user overrides registered after initialization.

See [split views](split-view.md) for resizable panes and [forms and scrolling](layout.md) for bounded content. Run `devenv shell -- cargo test -p gpui-cupertino --test navigation --locked` for selection, disabled items, unknown IDs, and tab state isolation.
