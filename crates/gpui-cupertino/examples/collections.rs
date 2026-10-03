//! Searchable virtual list, sortable table, and selected-record detail composition.

use gpui::{
    App, Bounds, Context, Entity, KeyBinding, Render, Role, SharedString, Subscription, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_cupertino::{
    components::{
        Button, CollectionEvent, CollectionRow, CollectionView, ReorderDirection, SelectionMode,
        TableColumn, TextInput, TextInputEvent,
    },
    theme::Theme,
};

actions!(
    collection_showcase,
    [
        /// Focus the next control.
        Next,
        /// Focus the previous control.
        Previous,
        /// Close the showcase.
        Quit,
    ]
);

struct Showcase {
    search: Entity<TextInput>,
    list: Entity<CollectionView>,
    table: Entity<CollectionView>,
    detail: SharedString,
    status: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl Showcase {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let rows: Vec<_> = (0..10_000)
            .map(|index| {
                CollectionRow::new(format!("record-{index}"), format!("Record {index:05}"))
                    .cell(if index % 2 == 0 { "Document" } else { "Image" })
            })
            .collect();
        let list = cx.new(|cx| {
            CollectionView::new("Search results", rows.clone(), cx)
                .expect("generated row identities are unique")
                .selection_mode(SelectionMode::Multiple)
        });
        let table = cx.new(|cx| {
            CollectionView::new("Record table", rows, cx)
                .expect("generated row identities are unique")
                .selection_mode(SelectionMode::Multiple)
                .with_columns([
                    TableColumn::new("name", "Name", px(260.0)),
                    TableColumn::new("kind", "Kind", px(160.0)),
                ])
                .expect("each generated row has two cells and valid column widths")
        });
        let search = cx.new(|cx| {
            TextInput::new("Search records", window, cx).placeholder("Filter 10,000 records")
        });
        let search_subscription = cx.subscribe(&search, |this, _, event, cx| {
            if let TextInputEvent::Changed(query) = event {
                this.list.update(cx, |view, cx| view.set_filter(query, cx));
                this.table.update(cx, |view, cx| view.set_filter(query, cx));
                cx.notify();
            }
        });
        let list_subscription = cx.subscribe(&list, Self::changed);
        let table_subscription = cx.subscribe(&table, Self::changed);
        Self {
            search,
            list,
            table,
            detail: "Select a record to inspect its identity and cells.".into(),
            status: "Shift selects a range. Command on macOS or Ctrl elsewhere toggles rows."
                .into(),
            _subscriptions: vec![search_subscription, list_subscription, table_subscription],
        }
    }

    fn changed(
        &mut self,
        collection: Entity<CollectionView>,
        event: &CollectionEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            CollectionEvent::SelectionChanged(ids) => {
                let view = collection.read(cx);
                self.status = format!("{} selected, including hidden records", ids.len()).into();
                self.detail = ids
                    .first()
                    .and_then(|id| view.rows().iter().find(|row| row.id() == id))
                    .map(|row| {
                        format!(
                            "{}\n{}",
                            row.id(),
                            row.cells()
                                .iter()
                                .map(|cell| cell.as_ref())
                                .collect::<Vec<_>>()
                                .join(" · ")
                        )
                    })
                    .unwrap_or_else(|| "No record selected".to_owned())
                    .into();
            }
            CollectionEvent::Activated(id) => self.status = format!("Activated {id}").into(),
            CollectionEvent::Reordered(_) => {
                self.status =
                    "Moved the visible selected rows. Hidden selections stay in place.".into()
            }
            CollectionEvent::SortChanged(Some(_)) => {
                self.status = "Sorted view: restore source order to reorder rows.".into()
            }
            CollectionEvent::SortChanged(None) => {
                self.status =
                    "Source order restored. Drag rows or use Option-Up/Down to reorder.".into()
            }
            CollectionEvent::ColumnResized { .. } => {}
        }
        cx.notify();
    }
}

impl Render for Showcase {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        div()
            .key_context("CupertinoCollectionsShowcase")
            .size_full()
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_action(|_: &Next, window, cx| window.focus_next(cx))
            .on_action(|_: &Previous, window, cx| window.focus_prev(cx))
            .child(div().text_size(px(24.0)).child("Collections"))
            .child(self.search.clone())
            .child(
                div()
                    .h(px(240.0))
                    .flex()
                    .gap(px(16.0))
                    .child(div().w(px(360.0)).h_full().child(self.list.clone()))
                    .child(
                        div()
                            .id("record-detail")
                            .role(Role::Group)
                            .aria_label("Record details")
                            .aria_description(self.detail.clone())
                            .flex_1()
                            .p(px(16.0))
                            .bg(theme.control_background)
                            .rounded(px(8.0))
                            .child(self.detail.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        Button::new("move-up", "Move list selection up")
                            .disabled(self.list.read(cx).selected_ids().is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.list.update(cx, |view, cx| {
                                    view.move_selected(ReorderDirection::Up, cx);
                                });
                            })),
                    )
                    .child(
                        Button::new("move-down", "Move list selection down")
                            .disabled(self.list.read(cx).selected_ids().is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.list.update(cx, |view, cx| {
                                    view.move_selected(ReorderDirection::Down, cx);
                                });
                            })),
                    )
                    .child(
                        Button::new("source-order", "Restore table source order")
                            .disabled(self.table.read(cx).sort().is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.table.update(cx, |view, cx| view.clear_sort(cx))
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().child(self.table.clone()))
            .child(
                div()
                    .id("status")
                    .role(Role::Status)
                    .aria_label(self.status.clone())
                    .a11y_synthetic_children({
                        let status = self.status.clone();
                        move |builder| {
                            let node = builder.parent_node();
                            node.set_value(status.to_string());
                            node.set_live(gpui::accesskit::Live::Polite);
                        }
                    })
                    .text_size(px(12.0))
                    .text_color(theme.secondary_foreground)
                    .child(self.status.clone()),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        gpui_cupertino::init(cx);
        cx.bind_keys([
            KeyBinding::new("tab", Next, Some("CupertinoCollectionsShowcase")),
            KeyBinding::new("shift-tab", Previous, Some("CupertinoCollectionsShowcase")),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(900.0), px(740.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Cupertino Collections");
                cx.new(|cx| Showcase::new(window, cx))
            },
        )
        .expect("the collections showcase requires a graphical session");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        AccessibleAction, TestAppContext,
        accesskit::{ActionData, ActionRequest, TreeId},
    };

    #[gpui::test]
    fn search_input_filters_both_views_and_list_selection_updates_detail(cx: &mut TestAppContext) {
        cx.update(gpui_cupertino::init);
        let (view, cx) = cx.add_window_view(Showcase::new);
        let handle = cx.update(|window, _| window.window_handle());
        let platform = cx.test_window(handle);
        platform.activate_accessibility();
        cx.run_until_parked();
        let action = |label: &str, action, data| {
            let id = platform
                .accessibility_tree_update()
                .unwrap()
                .nodes
                .into_iter()
                .find(|(_, node)| node.label() == Some(label))
                .unwrap()
                .0;
            platform.simulate_accessibility_action(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: id,
                data,
            });
        };
        action(
            "Search records",
            AccessibleAction::SetValue,
            Some(ActionData::Value("09999".into())),
        );
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.list.read(cx).visible_ids().count(), 1);
            assert_eq!(view.table.read(cx).visible_ids().count(), 1);
        });
        action("Record 09999", AccessibleAction::Click, None);
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(view.detail.contains("record-9999"));
            assert!(view.detail.contains("Image"));
            assert!(view.status.starts_with("1 selected"));
        });
        let nodes = platform.accessibility_tree_update().unwrap().nodes;
        assert!(nodes.iter().any(|(_, node)| {
            node.label() == Some("Record details")
                && node
                    .description()
                    .is_some_and(|text| text.contains("record-9999") && text.contains("Image"))
        }));
        assert!(nodes.iter().any(|(_, node)| node.role() == Role::Status
            && node.value() == Some("1 selected, including hidden records")
            && node.live() == Some(gpui::accesskit::Live::Polite)));
    }
}
