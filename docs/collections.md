# Virtual collections

Call `gpui_cupertino::init(cx)` before creating windows. Retain an `Entity<CollectionView>` and give its parent a bounded height. `CollectionView` uses GPUI `uniform_list`; only the visible range is rendered. Rows have a fixed height of 32 logical pixels.

```rust
use gpui::{Context, px};
use gpui_cupertino::components::{CollectionError, CollectionRow, CollectionView, SelectionMode, TableColumn};

fn records(cx: &mut Context<CollectionView>) -> Result<CollectionView, CollectionError> {
    CollectionView::new("Files", [
        CollectionRow::new("a", "Archive").cell("Folder"),
        CollectionRow::new("b", "Photo").cell("Image"),
    ], cx)?
        .selection_mode(SelectionMode::Multiple)
        .with_columns([
            TableColumn::new("name", "Name", px(240.0)),
            TableColumn::new("kind", "Kind", px(120.0)),
        ])
}
```

Omit `with_columns` for a list. The first cell supplies the visible list label; the filter searches every cell. `with_columns` enables a table with a fixed header, vertical row virtualization, and shared horizontal scrolling for the header and cells.

## Data and identity

Row IDs must be unique. Column IDs must be unique. Every table row must contain one cell per column. Column widths must be finite and between 48 and 640 logical pixels. The constructor, `with_columns`, and `set_rows` return `CollectionError` when these conditions fail. `set_rows` validates the replacement before changing the current rows, selection, or sort.

`rows()` reports source order. `visible_ids()` reports filtered and sorted display order. `selected_ids()` includes selected rows hidden by the filter. Replacing rows retains selection for IDs that still exist and removes IDs that no longer exist. Changing a row's text or position does not change its identity.

`set_filter` uses a lowercase substring match across all cells. Filtering preserves selection and the range anchor. A focused row that remains visible retains focus and scrolls into view. If the focused row becomes hidden or is removed, focus moves to the first visible row without selecting that row. An empty result has no active row.

## Selection and keyboard operation

Single selection is the default. `SelectionMode::Multiple` adds these behaviors:

- A plain click or unmodified movement selects one row and establishes the range anchor.
- Command-click on macOS, or Ctrl-click on other targets, toggles a row and establishes a new anchor.
- Shift-click or Shift-movement selects a range in current display order. The range replaces visible selections and retains hidden selections.
- Adding the platform modifier to Shift extends the selected set without removing previous selections.
- If the anchor is hidden, the current visible focused row becomes the new anchor before range selection.
- Command-A on macOS, or Ctrl-A on other targets, selects every visible row and retains hidden selections.

Up, Down, Home, and End move through the current display order. The platform modifier moves focus without changing selection. Space selects the focused row; platform-modified Space toggles the focused row in multiple-selection mode. Escape clears selection, including hidden selections when the filtered result is empty. Enter or a double click emits `CollectionEvent::Activated`.

Navigation and column resizing use private scoped GPUI actions. Bind application navigation to an ancestor `key_context`. A context-free binding registered after library initialization is an explicit application override and can replace the component binding. The application owns Tab and Shift-Tab traversal.

## Sorting and resizing

Click a table header or activate the header through accessibility to toggle ascending and descending lexical string order. Equal cell values retain source order. Sorting preserves selected IDs and focused identity. `sort()` reports the column and direction. `clear_sort` restores source order and enables reordering.

Drag a column's right edge to resize the column. Pointer capture continues the resize outside the header and window bounds until the left button is released. Focus the resize handle to use Left and Right in 8-pixel steps; Home and End select the minimum and maximum widths. Accessibility exposes the width, range, Increment, Decrement, and SetValue. Finite requested widths are clamped to the supported range; nonfinite values are rejected.

## Reordering

Unsorted collections support internal row drag and drop. Dragging an unselected row first selects that row. A drop moves the source row, or the selected visible rows when the source remains selected, as one block. The block retains source order and is inserted before the target row. The bottom drop strip appends the block. Hidden selected rows stay selected and do not move. Cross-collection drops and a source or target that disappeared are rejected.

`move_selected(ReorderDirection::Up, cx)` and `Down` move the visible selected block one step. Option-Up and Option-Down provide the same keyboard operation. Accessibility exposes `Move selected rows up` and `Move selected rows down` custom actions on the collection. Sorted views disable all reorder operations. Boundary moves return `false` and do not emit a reorder event.

`CollectionEvent` reports selection changes, activation, source-order changes, sorting, and column resizing. Selection events contain lexically ordered IDs; reorder events contain all IDs in source order. The caller can persist reorder events without inferring order from the filtered viewport.

## Accessibility

A list exposes `ListBox` and `ListBoxOption` roles. A table exposes `Grid`, `Row`, `ColumnHeader`, and `Cell` roles. The table row count includes its header. Row and column indices are zero-based; list positions are one-based. Counts describe the complete filtered result, while nodes describe the rendered viewport.

The collection holds one keyboard focus handle and reports the active rendered row through GPUI's active-descendant mechanism. Keyboard movement and accessibility ScrollUp or ScrollDown bring the destination into the rendered viewport. Rows expose Focus, Click, ScrollIntoView, and selected state. Multiple-selection rows also expose a `Toggle selection` custom action.

## Example and verification

Run `cargo run -p gpui-cupertino --example collections --locked` in the existing devenv shell. The example combines a TextInput search field, a virtual multiple-selection list, a detail panel, and a sortable, resizable table. Both collections contain 10,000 rows and share the search query. Buttons exercise the public reorder and clear-sort APIs.

Run these CPU checks:

```sh
cargo test -p gpui-cupertino --test collections --test choice_navigation --example collections --locked
cargo bench -p gpui-cupertino --bench collections --locked
```

The integration checks dispatch real GPUI pointer, key, drag, and accessibility events. They verify viewport node counts, offscreen focus, selection identity, filter and replacement behavior, sorting, resize capture, reorder, validation, and the search/detail composition. The benchmark reports median and P95 CPU times for 1,000, 10,000, and 50,000 rows. The default benchmark profile is optimized; add `--profile dev` for a separate debug measurement. Measurements include GPUI layout and event processing in TestAppContext, not native drawing or GPU time.

Native VoiceOver navigation, native text composition in search, pointer feel, display appearance, and native frame timing require separate acceptance. This version uses text cells and fixed row height. Variable-height rows, custom cell editors, hierarchical rows, external drag payloads, and automatic edge scrolling during a drag are not implemented.
