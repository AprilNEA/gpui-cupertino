//! Virtual collections with stable row identity, selection, sorting, and reordering.

use std::{
    collections::{BTreeSet, HashSet},
    error::Error,
    fmt,
};

use gpui::{
    Context, EventEmitter, FocusHandle, Focusable, Pixels, SharedString, UniformListScrollHandle,
    px,
};

mod render;
mod selection;
mod table;

use selection::Selection;

pub(crate) fn init(cx: &mut gpui::App) {
    selection::init(cx);
    table::init(cx);
}

/// The selection policy for a collection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// Select at most one row.
    #[default]
    Single,
    /// Support modifier toggles, ranges, and Select All.
    Multiple,
}

/// A row whose identity remains stable when its position or cell values change.
#[derive(Clone, Debug)]
pub struct CollectionRow {
    id: SharedString,
    cells: Vec<SharedString>,
}

impl CollectionRow {
    /// Creates a row with its first cell, also used as the list label.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            cells: vec![label.into()],
        }
    }

    /// Appends a table cell in column order.
    pub fn cell(mut self, value: impl Into<SharedString>) -> Self {
        self.cells.push(value.into());
        self
    }

    /// Returns the stable identity.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns cells in column order. The first cell is the list label.
    pub fn cells(&self) -> &[SharedString] {
        &self.cells
    }
}

/// A table column. Widths must be finite and between 48 and 640 logical pixels.
#[derive(Clone, Debug)]
pub struct TableColumn {
    id: SharedString,
    label: SharedString,
    width: Pixels,
}

impl TableColumn {
    /// Describes a column. `CollectionView::with_columns` validates the width and ID.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, width: Pixels) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            width,
        }
    }

    /// Returns the column identity.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the current width in logical pixels.
    pub fn width(&self) -> Pixels {
        self.width
    }
}

/// The direction of a lexical table sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// Sort from the smallest cell string to the largest.
    Ascending,
    /// Sort from the largest cell string to the smallest.
    Descending,
}

/// A column identity and its current sort direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableSort {
    /// The sorted column's stable identity.
    pub column: SharedString,
    /// The string comparison direction.
    pub direction: SortDirection,
}

/// A one-step move for the visible selected rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReorderDirection {
    /// Move the selection before the preceding visible row.
    Up,
    /// Move the selection after the following visible row.
    Down,
}

/// Invalid row or table data. A failed replacement leaves the collection unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectionError {
    /// Row identities must be unique.
    DuplicateRow(SharedString),
    /// Column identities must be unique.
    DuplicateColumn(SharedString),
    /// Table rows must have one cell per column.
    CellCount {
        /// The invalid row identity.
        row: SharedString,
        /// The required cell count.
        expected: usize,
        /// The supplied cell count.
        actual: usize,
    },
    /// Column width must be finite and within the supported range.
    ColumnWidth(SharedString),
    /// A requested sort column does not exist.
    UnknownColumn(SharedString),
}

impl fmt::Display for CollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRow(id) => write!(f, "duplicate collection row ID: {id}"),
            Self::DuplicateColumn(id) => write!(f, "duplicate table column ID: {id}"),
            Self::CellCount {
                row,
                expected,
                actual,
            } => write!(f, "row {row} has {actual} cells; table requires {expected}"),
            Self::ColumnWidth(id) => write!(
                f,
                "column {id} width must be finite and between 48 and 640 logical pixels"
            ),
            Self::UnknownColumn(id) => write!(f, "unknown table column ID: {id}"),
        }
    }
}
impl Error for CollectionError {}

/// Changes emitted by a collection entity.
#[derive(Clone, Debug, PartialEq)]
pub enum CollectionEvent {
    /// The selected IDs changed. IDs are in lexical order and include hidden selections.
    SelectionChanged(Vec<SharedString>),
    /// Enter or a double click activated the focused row.
    Activated(SharedString),
    /// Row order changed. The payload contains every row ID in source order.
    Reordered(Vec<SharedString>),
    /// The table's sort changed. `None` restores source order and enables reordering.
    SortChanged(Option<TableSort>),
    /// A column's width changed.
    ColumnResized {
        /// The resized column identity.
        column: SharedString,
        /// The new logical width.
        width: Pixels,
    },
}

/// A retained virtual list or table with fixed 32-pixel rows.
///
/// Retain this entity in the parent view and give the parent a bounded height.
/// `with_columns` enables table presentation. Filtering retains hidden selection IDs.
pub struct CollectionView {
    label: SharedString,
    rows: Vec<CollectionRow>,
    columns: Vec<TableColumn>,
    visible: Vec<usize>,
    query: String,
    selection: Selection,
    sort: Option<TableSort>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    resize: Option<(usize, Pixels, Pixels)>,
}

impl EventEmitter<CollectionEvent> for CollectionView {}

impl CollectionView {
    /// Creates a single-selection list and validates row identities.
    pub fn new(
        label: impl Into<SharedString>,
        rows: impl IntoIterator<Item = CollectionRow>,
        cx: &mut Context<Self>,
    ) -> Result<Self, CollectionError> {
        let rows: Vec<_> = rows.into_iter().collect();
        validate_rows(&rows, 0)?;
        let mut view = Self {
            label: label.into(),
            rows,
            columns: Vec::new(),
            visible: Vec::new(),
            query: String::new(),
            selection: Selection::default(),
            sort: None,
            focus: cx.focus_handle().tab_stop(true),
            scroll: UniformListScrollHandle::new(),
            resize: None,
        };
        view.rebuild_visible();
        Ok(view)
    }

    /// Sets the initial selection policy before the entity is rendered.
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection.mode = mode;
        self
    }

    /// Enables table presentation and validates columns and row cell counts.
    pub fn with_columns(
        mut self,
        columns: impl IntoIterator<Item = TableColumn>,
    ) -> Result<Self, CollectionError> {
        let columns: Vec<_> = columns.into_iter().collect();
        let mut ids = HashSet::with_capacity(columns.len());
        for column in &columns {
            if !ids.insert(&column.id) {
                return Err(CollectionError::DuplicateColumn(column.id.clone()));
            }
            if !column.width.as_f32().is_finite() || !(px(48.0)..=px(640.0)).contains(&column.width)
            {
                return Err(CollectionError::ColumnWidth(column.id.clone()));
            }
        }
        validate_rows(&self.rows, columns.len())?;
        self.columns = columns;
        Ok(self)
    }

    /// Replaces rows atomically. Selection survives for identities that still exist.
    pub fn set_rows(
        &mut self,
        rows: impl IntoIterator<Item = CollectionRow>,
        cx: &mut Context<Self>,
    ) -> Result<(), CollectionError> {
        let rows: Vec<_> = rows.into_iter().collect();
        validate_rows(&rows, self.columns.len())?;
        self.rows = rows;
        let before = self.selection.selected.clone();
        let ids: HashSet<_> = self.rows.iter().map(|row| &row.id).collect();
        self.selection.selected.retain(|id| ids.contains(id));
        if self
            .selection
            .anchor
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            self.selection.anchor = None;
        }
        self.rebuild_visible();
        self.scroll_to_focus();
        self.selection_changed(before, cx);
        cx.notify();
        Ok(())
    }

    /// Filters all cells with a case-insensitive substring. Hidden selection IDs remain selected.
    pub fn set_filter(&mut self, query: impl AsRef<str>, cx: &mut Context<Self>) {
        self.query = query.as_ref().to_lowercase();
        self.rebuild_visible();
        self.scroll_to_focus();
        cx.notify();
    }

    /// Returns all selected IDs, including rows hidden by the filter.
    pub fn selected_ids(&self) -> &BTreeSet<SharedString> {
        &self.selection.selected
    }

    /// Returns the focused visible row identity, if any.
    pub fn focused_id(&self) -> Option<&SharedString> {
        self.selection.focused.as_ref()
    }

    /// Returns all rows in source order, independent of filtering and sorting.
    pub fn rows(&self) -> &[CollectionRow] {
        &self.rows
    }

    /// Returns the current table columns and widths.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Returns visible row IDs in display order.
    pub fn visible_ids(&self) -> impl Iterator<Item = &SharedString> {
        self.visible.iter().map(|index| &self.rows[*index].id)
    }

    /// Returns the current sort, if table sorting is active.
    pub fn sort(&self) -> Option<&TableSort> {
        self.sort.as_ref()
    }

    /// Toggles a column between ascending and descending lexical order.
    pub fn toggle_sort(
        &mut self,
        column: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), CollectionError> {
        let column = self
            .columns
            .iter()
            .find(|item| item.id.as_ref() == column)
            .ok_or_else(|| CollectionError::UnknownColumn(column.to_owned().into()))?;
        let direction = if self.sort.as_ref().is_some_and(|sort| {
            sort.column == column.id && sort.direction == SortDirection::Ascending
        }) {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        };
        self.sort = Some(TableSort {
            column: column.id.clone(),
            direction,
        });
        self.rebuild_visible();
        self.scroll_to_focus();
        cx.emit(CollectionEvent::SortChanged(self.sort.clone()));
        cx.notify();
        Ok(())
    }

    /// Restores source order and enables drag and keyboard reordering.
    pub fn clear_sort(&mut self, cx: &mut Context<Self>) {
        if self.sort.take().is_some() {
            self.rebuild_visible();
            self.scroll_to_focus();
            cx.emit(CollectionEvent::SortChanged(None));
            cx.notify();
        }
    }

    /// Moves visible selected rows one step. Sorted views and boundary moves return false.
    pub fn move_selected(&mut self, direction: ReorderDirection, cx: &mut Context<Self>) -> bool {
        if self.sort.is_some() {
            return false;
        }
        let visible: Vec<_> = self.visible_ids().cloned().collect();
        let selected: Vec<_> = visible
            .iter()
            .filter(|id| self.selection.selected.contains(*id))
            .cloned()
            .collect();
        let Some(first) = visible
            .iter()
            .position(|id| self.selection.selected.contains(id))
        else {
            return false;
        };
        let last = visible
            .iter()
            .rposition(|id| self.selection.selected.contains(id))
            .expect("the selection has a first visible item");
        let target = match direction {
            ReorderDirection::Up if first > 0 => Some(visible[first - 1].clone()),
            ReorderDirection::Down if last + 1 < visible.len() => visible.get(last + 2).cloned(),
            _ => return false,
        };
        self.reorder(&selected, target.as_ref(), cx)
    }

    fn rebuild_visible(&mut self) {
        self.visible = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                self.query.is_empty()
                    || row
                        .cells
                        .iter()
                        .any(|cell| cell.to_lowercase().contains(&self.query))
            })
            .map(|(index, _)| index)
            .collect();
        if let Some(sort) = &self.sort {
            let column = self
                .columns
                .iter()
                .position(|column| column.id == sort.column)
                .expect("sort column was validated");
            self.visible.sort_by(|a, b| {
                let order = self.rows[*a].cells[column].cmp(&self.rows[*b].cells[column]);
                if sort.direction == SortDirection::Ascending {
                    order
                } else {
                    order.reverse()
                }
            });
        }
        if !self
            .visible
            .iter()
            .any(|index| self.selection.focused.as_ref() == Some(&self.rows[*index].id))
        {
            self.selection.focused = self
                .visible
                .first()
                .map(|index| self.rows[*index].id.clone());
        }
    }

    fn scroll_to_focus(&self) {
        if let Some(index) = self
            .visible_ids()
            .position(|id| Some(id) == self.selection.focused.as_ref())
        {
            self.scroll
                .scroll_to_item(index, gpui::ScrollStrategy::Nearest);
        }
    }

    fn selection_changed(&self, before: BTreeSet<SharedString>, cx: &mut Context<Self>) {
        if before != self.selection.selected {
            cx.emit(CollectionEvent::SelectionChanged(
                self.selection.selected.iter().cloned().collect(),
            ));
        }
    }

    fn reorder(
        &mut self,
        moving: &[SharedString],
        before: Option<&SharedString>,
        cx: &mut Context<Self>,
    ) -> bool {
        let moving: HashSet<_> = moving.iter().collect();
        if self.sort.is_some() || moving.is_empty() || before.is_some_and(|id| moving.contains(id))
        {
            return false;
        }
        if before.is_some_and(|id| !self.rows.iter().any(|row| &row.id == id)) {
            return false;
        }
        let original: Vec<_> = self.rows.iter().map(|row| row.id.clone()).collect();
        let (moved, mut remaining): (Vec<_>, Vec<_>) = std::mem::take(&mut self.rows)
            .into_iter()
            .partition(|row| moving.contains(&row.id));
        let index = before
            .and_then(|id| remaining.iter().position(|row| row.id == *id))
            .unwrap_or(remaining.len());
        remaining.splice(index..index, moved);
        self.rows = remaining;
        let reordered: Vec<_> = self.rows.iter().map(|row| row.id.clone()).collect();
        if original == reordered {
            return false;
        }
        self.rebuild_visible();
        self.scroll_to_focus();
        cx.emit(CollectionEvent::Reordered(reordered));
        cx.notify();
        true
    }
}

impl Focusable for CollectionView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

fn validate_rows(rows: &[CollectionRow], columns: usize) -> Result<(), CollectionError> {
    let mut ids = HashSet::with_capacity(rows.len());
    for row in rows {
        if !ids.insert(&row.id) {
            return Err(CollectionError::DuplicateRow(row.id.clone()));
        }
        if columns > 0 && row.cells.len() != columns {
            return Err(CollectionError::CellCount {
                row: row.id.clone(),
                expected: columns,
                actual: row.cells.len(),
            });
        }
    }
    Ok(())
}
