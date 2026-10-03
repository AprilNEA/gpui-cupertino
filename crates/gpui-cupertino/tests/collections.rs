//! CPU integration checks for virtual rendering, identity, selection, and table input.

use gpui::{
    AccessibleAction, Context, Focusable, InputEvent, KeyDownEvent, KeyUpEvent, Keystroke,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Role,
    TestAppContext, Window, WindowHandle,
    accesskit::{ActionData, ActionRequest, Node, NodeId, TreeId},
    point, px, size,
};
use gpui_cupertino::components::{
    CollectionError, CollectionRow, CollectionView, ReorderDirection, SelectionMode, SortDirection,
    TableColumn,
};

gpui::actions!(
    collection_conflict,
    [
        /// Record an ancestor navigation action.
        OuterNavigation
    ]
);

fn open(
    cx: &mut TestAppContext,
    count: usize,
    multiple: bool,
    table: bool,
) -> WindowHandle<CollectionView> {
    cx.update(gpui_cupertino::init);
    let window = cx.open_window(size(px(420.0), px(240.0)), |_, cx| {
        let rows = (0..count).map(|index| {
            let row = CollectionRow::new(format!("id-{index}"), format!("Item {index:05}"));
            if table {
                row.cell(format!("{}", (index * 17) % 13))
            } else {
                row
            }
        });
        let collection = CollectionView::new("Records", rows, cx)
            .unwrap()
            .selection_mode(if multiple {
                SelectionMode::Multiple
            } else {
                SelectionMode::Single
            });
        if table {
            collection
                .with_columns([
                    TableColumn::new("name", "Name", px(180.0)),
                    TableColumn::new("value", "Value", px(140.0)),
                ])
                .unwrap()
        } else {
            collection
        }
    });
    cx.test_window(window.into()).activate_accessibility();
    cx.run_until_parked();
    window
}

fn update(
    window: WindowHandle<CollectionView>,
    cx: &mut TestAppContext,
    f: impl FnOnce(&mut CollectionView, &mut Window, &mut Context<CollectionView>),
) {
    window
        .update(cx, |view, window, cx| {
            f(view, window, cx);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
}

fn node(window: WindowHandle<CollectionView>, label: &str, cx: &TestAppContext) -> (NodeId, Node) {
    cx.test_window(window.into())
        .accessibility_tree_update()
        .unwrap()
        .nodes
        .into_iter()
        .find(|(_, node)| node.label() == Some(label))
        .unwrap()
}

fn event(window: WindowHandle<CollectionView>, event: impl InputEvent, cx: &mut TestAppContext) {
    cx.test_window(window.into())
        .simulate_input(event.to_platform_input());
    cx.run_until_parked();
}

fn key(window: WindowHandle<CollectionView>, key: &str, cx: &mut TestAppContext) {
    event(
        window,
        KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        },
        cx,
    );
    event(
        window,
        KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        },
        cx,
    );
}

fn action(
    window: WindowHandle<CollectionView>,
    label: &str,
    action: AccessibleAction,
    data: Option<ActionData>,
    cx: &mut TestAppContext,
) {
    cx.test_window(window.into())
        .simulate_accessibility_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node(window, label, cx).0,
            data,
        });
    cx.run_until_parked();
}

fn center(
    window: WindowHandle<CollectionView>,
    label: &str,
    cx: &mut TestAppContext,
) -> Point<Pixels> {
    let rect = node(window, label, cx).1.bounds().unwrap();
    let scale = window
        .update(cx, |_, window, _| f64::from(window.scale_factor()))
        .unwrap();
    point(
        px(((rect.x0 + rect.x1) / (2.0 * scale)) as f32),
        px(((rect.y0 + rect.y1) / (2.0 * scale)) as f32),
    )
}

fn mouse_down(
    window: WindowHandle<CollectionView>,
    position: Point<Pixels>,
    modifiers: Modifiers,
    cx: &mut TestAppContext,
) {
    event(
        window,
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        },
        cx,
    );
}

fn mouse_up(
    window: WindowHandle<CollectionView>,
    position: Point<Pixels>,
    modifiers: Modifiers,
    cx: &mut TestAppContext,
) {
    event(
        window,
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
        },
        cx,
    );
}

fn click(
    window: WindowHandle<CollectionView>,
    label: &str,
    modifiers: Modifiers,
    cx: &mut TestAppContext,
) {
    let position = center(window, label, cx);
    mouse_down(window, position, modifiers, cx);
    mouse_up(window, position, modifiers, cx);
}

fn selected(window: WindowHandle<CollectionView>, cx: &mut TestAppContext) -> Vec<String> {
    window
        .update(cx, |view, _, _| {
            view.selected_ids()
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .unwrap()
}

fn primary(key: &str) -> String {
    format!(
        "{}-{key}",
        if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        }
    )
}

#[gpui::test]
fn ten_thousand_rows_are_virtual_and_offscreen_keyboard_focus_scrolls_into_view(
    cx: &mut TestAppContext,
) {
    let window = open(cx, 10_000, true, false);
    let tree = cx
        .test_window(window.into())
        .accessibility_tree_update()
        .unwrap();
    let rows = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::ListBoxOption)
        .count();
    assert!(
        (3..20).contains(&rows),
        "a 240-pixel viewport rendered {rows} rows"
    );
    assert_eq!(node(window, "Records", cx).1.row_count(), Some(10_000));
    assert!(node(window, "Records", cx).1.is_multiselectable());
    update(window, cx, |view, window, cx| {
        view.focus_handle(cx).focus(window, cx)
    });
    key(window, "end", cx);
    assert_eq!(selected(window, cx), ["id-9999"]);
    let (last, last_node) = node(window, "Item 09999", cx);
    assert_eq!(last_node.position_in_set(), Some(10_000));
    assert_eq!(last_node.size_of_set(), Some(10_000));
    assert_eq!(
        cx.test_window(window.into())
            .accessibility_tree_update()
            .unwrap()
            .focus,
        last
    );
    update(window, cx, |view, _, cx| view.set_filter("Item", cx));
    assert_eq!(node(window, "Item 09999", cx).0, last);
    assert_eq!(
        cx.test_window(window.into())
            .accessibility_tree_update()
            .unwrap()
            .focus,
        last
    );
    key(window, "home", cx);
    assert_eq!(selected(window, cx), ["id-0"]);
    action(window, "Records", AccessibleAction::ScrollDown, None, cx);
    window
        .update(cx, |view, _, _| {
            assert_ne!(view.focused_id().map(|id| id.as_ref()), Some("id-0"))
        })
        .unwrap();
    key(window, "end", cx);
    update(window, cx, |view, _, cx| {
        let rows = view
            .rows()
            .iter()
            .filter(|row| row.id().as_ref() != "id-9999")
            .cloned()
            .collect::<Vec<_>>();
        view.set_rows(rows, cx).unwrap();
    });
    let first = node(window, "Item 00000", cx).0;
    assert_eq!(
        cx.test_window(window.into())
            .accessibility_tree_update()
            .unwrap()
            .focus,
        first
    );
}

#[gpui::test]
fn pointer_ranges_toggles_filtering_and_identity_replacement_preserve_selection(
    cx: &mut TestAppContext,
) {
    let window = open(cx, 6, true, false);
    click(window, "Item 00001", Modifiers::none(), cx);
    click(
        window,
        "Item 00003",
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
        cx,
    );
    assert_eq!(selected(window, cx), ["id-1", "id-2", "id-3"]);
    click(window, "Item 00002", Modifiers::secondary_key(), cx);
    assert_eq!(selected(window, cx), ["id-1", "id-3"]);
    update(window, cx, |view, _, cx| view.set_filter("00005", cx));
    assert_eq!(selected(window, cx), ["id-1", "id-3"]);
    key(window, "shift-space", cx);
    assert_eq!(selected(window, cx), ["id-1", "id-3", "id-5"]);
    update(window, cx, |view, _, cx| view.set_filter("", cx));
    action(
        window,
        "Item 00003",
        AccessibleAction::CustomAction,
        Some(ActionData::CustomAction(0)),
        cx,
    );
    assert_eq!(selected(window, cx), ["id-1", "id-5"]);
    update(window, cx, |view, _, cx| {
        view.set_rows(
            [
                CollectionRow::new("id-5", "Renamed five"),
                CollectionRow::new("id-1", "Renamed one"),
                CollectionRow::new("new", "New record"),
            ],
            cx,
        )
        .unwrap();
    });
    assert_eq!(selected(window, cx), ["id-1", "id-5"]);
    key(window, &primary("a"), cx);
    assert_eq!(selected(window, cx), ["id-1", "id-5", "new"]);
    key(window, "escape", cx);
    assert!(selected(window, cx).is_empty());
}

#[gpui::test]
fn single_selection_ignores_toggle_and_range_modifiers(cx: &mut TestAppContext) {
    let window = open(cx, 5, false, false);
    click(window, "Item 00001", Modifiers::none(), cx);
    click(window, "Item 00003", Modifiers::secondary_key(), cx);
    assert_eq!(selected(window, cx), ["id-3"]);
    key(window, "shift-home", cx);
    assert_eq!(selected(window, cx), ["id-0"]);
    key(window, &primary("a"), cx);
    assert_eq!(selected(window, cx), ["id-0"]);
}

#[gpui::test]
fn table_sort_is_stable_and_resizing_handles_pointer_keyboard_and_accessibility(
    cx: &mut TestAppContext,
) {
    let window = open(cx, 4, true, true);
    update(window, cx, |view, _, cx| {
        view.set_rows(
            [
                CollectionRow::new("a", "Alfa").cell("20"),
                CollectionRow::new("b", "Beta").cell("10"),
                CollectionRow::new("c", "Gamma").cell("20"),
                CollectionRow::new("d", "Delta").cell("2"),
            ],
            cx,
        )
        .unwrap()
    });
    click(window, "Value", Modifiers::none(), cx);
    window
        .update(cx, |view, _, _| {
            assert_eq!(
                view.visible_ids().map(|id| id.as_ref()).collect::<Vec<_>>(),
                ["b", "d", "a", "c"]
            );
            assert_eq!(view.sort().unwrap().direction, SortDirection::Ascending);
        })
        .unwrap();
    action(window, "Value", AccessibleAction::Click, None, cx);
    window
        .update(cx, |view, _, cx| {
            assert_eq!(
                view.visible_ids().map(|id| id.as_ref()).collect::<Vec<_>>(),
                ["a", "c", "d", "b"]
            );
            assert!(!view.move_selected(ReorderDirection::Down, cx));
        })
        .unwrap();
    assert_eq!(node(window, "Records", cx).1.column_count(), Some(2));
    assert_eq!(node(window, "Records", cx).1.row_count(), Some(5));

    let start = center(window, "Resize Name", cx);
    mouse_down(window, start, Modifiers::none(), cx);
    let end = start + point(px(50.0), px(260.0));
    event(
        window,
        MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        },
        cx,
    );
    mouse_up(window, end, Modifiers::none(), cx);
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.columns()[0].width(), px(230.0))
        })
        .unwrap();
    action(window, "Resize Name", AccessibleAction::Focus, None, cx);
    key(window, "right", cx);
    assert_eq!(
        node(window, "Resize Name", cx).1.numeric_value(),
        Some(238.0)
    );
    action(window, "Resize Name", AccessibleAction::Decrement, None, cx);
    assert_eq!(
        node(window, "Resize Name", cx).1.numeric_value(),
        Some(230.0)
    );
    action(
        window,
        "Resize Name",
        AccessibleAction::SetValue,
        Some(ActionData::NumericValue(2000.0)),
        cx,
    );
    assert_eq!(
        node(window, "Resize Name", cx).1.numeric_value(),
        Some(640.0)
    );
    action(
        window,
        "Resize Name",
        AccessibleAction::SetValue,
        Some(ActionData::NumericValue(f64::NAN)),
        cx,
    );
    assert_eq!(
        node(window, "Resize Name", cx).1.numeric_value(),
        Some(640.0)
    );
    key(window, "home", cx);
    assert_eq!(
        node(window, "Resize Name", cx).1.numeric_value(),
        Some(48.0)
    );
}

#[gpui::test]
fn drag_drop_and_accessible_reorder_move_selected_visible_rows_only(cx: &mut TestAppContext) {
    let window = open(cx, 6, true, false);
    click(window, "Item 00001", Modifiers::none(), cx);
    click(window, "Item 00002", Modifiers::secondary_key(), cx);
    let start = center(window, "Item 00001", cx);
    let target = center(window, "Item 00000", cx);
    mouse_down(window, start, Modifiers::none(), cx);
    event(
        window,
        MouseMoveEvent {
            position: start + point(px(10.0), px(0.0)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        },
        cx,
    );
    event(
        window,
        MouseMoveEvent {
            position: target,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        },
        cx,
    );
    mouse_up(window, target, Modifiers::none(), cx);
    window
        .update(cx, |view, _, _| {
            assert_eq!(
                view.rows()
                    .iter()
                    .map(|row| row.id().as_ref())
                    .collect::<Vec<_>>(),
                ["id-1", "id-2", "id-0", "id-3", "id-4", "id-5"]
            )
        })
        .unwrap();
    assert_eq!(selected(window, cx), ["id-1", "id-2"]);
    action(
        window,
        "Records",
        AccessibleAction::CustomAction,
        Some(ActionData::CustomAction(2)),
        cx,
    );
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.rows()[0].id().as_ref(), "id-0")
        })
        .unwrap();
    update(window, cx, |view, _, cx| view.set_filter("00001", cx));
    action(
        window,
        "Records",
        AccessibleAction::CustomAction,
        Some(ActionData::CustomAction(2)),
        cx,
    );
    assert_eq!(selected(window, cx), ["id-1", "id-2"]);
    update(window, cx, |view, _, cx| view.set_filter("", cx));
    key(window, "alt-up", cx);
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.rows()[0].id().as_ref(), "id-1")
        })
        .unwrap();
}

#[gpui::test]
fn invalid_replacements_leave_rows_selection_and_sort_unchanged(cx: &mut TestAppContext) {
    let window = open(cx, 3, false, true);
    update(window, cx, |view, window, cx| {
        view.focus_handle(cx).focus(window, cx);
    });
    key(window, "space", cx);
    update(window, cx, |view, _, cx| {
        view.toggle_sort("value", cx).unwrap();
        let duplicate = view.set_rows(
            [
                CollectionRow::new("x", "X").cell("1"),
                CollectionRow::new("x", "Y").cell("2"),
            ],
            cx,
        );
        assert!(matches!(duplicate, Err(CollectionError::DuplicateRow(_))));
        assert!(matches!(
            view.set_rows([CollectionRow::new("x", "X")], cx),
            Err(CollectionError::CellCount {
                expected: 2,
                actual: 1,
                ..
            })
        ));
        assert!(matches!(
            view.toggle_sort("missing", cx),
            Err(CollectionError::UnknownColumn(_))
        ));
        assert_eq!(view.rows().len(), 3);
        assert_eq!(view.sort().unwrap().column.as_ref(), "value");
    });
    assert_eq!(selected(window, cx), ["id-0"]);
}

#[gpui::test]
fn collection_and_resize_actions_override_ancestor_navigation_only_when_focused(
    cx: &mut TestAppContext,
) {
    use gpui::{Entity, FocusHandle, IntoElement, Render, div, prelude::*};
    use gpui_cupertino::components::Button;

    struct Host {
        collection: Entity<CollectionView>,
        outside: FocusHandle,
        outer: usize,
    }
    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .key_context("CollectionHost")
                .size_full()
                .flex()
                .flex_col()
                .on_action(cx.listener(|view, _: &OuterNavigation, _, _| view.outer += 1))
                .child(div().h(px(200.0)).child(self.collection.clone()))
                .child(Button::new("outside", "Outside").track_focus(&self.outside))
        }
    }
    cx.update(|cx| {
        gpui_cupertino::init(cx);
        cx.bind_keys(
            ["down", "end", "right"]
                .into_iter()
                .map(|key| gpui::KeyBinding::new(key, OuterNavigation, Some("CollectionHost"))),
        );
    });
    let window = cx.open_window(size(px(400.0), px(240.0)), |_, cx| Host {
        collection: cx.new(|cx| {
            CollectionView::new(
                "Records",
                (0..40).map(|index| {
                    CollectionRow::new(format!("id-{index}"), format!("Item {index}"))
                }),
                cx,
            )
            .unwrap()
            .with_columns([TableColumn::new("name", "Name", px(180.0))])
            .unwrap()
        }),
        outside: cx.focus_handle(),
        outer: 0,
    });
    let platform = cx.test_window(window.into());
    platform.activate_accessibility();
    cx.run_until_parked();
    let mut sender = platform.clone();
    let mut send = |key: &str, cx: &mut TestAppContext| {
        sender.simulate_input(
            KeyDownEvent {
                keystroke: Keystroke::parse(key).unwrap(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
        );
        cx.run_until_parked();
        sender.simulate_input(
            KeyUpEvent {
                keystroke: Keystroke::parse(key).unwrap(),
            }
            .to_platform_input(),
        );
        cx.run_until_parked();
    };
    window
        .update(cx, |view, window, cx| {
            view.collection.focus_handle(cx).focus(window, cx)
        })
        .unwrap();
    cx.run_until_parked();
    send("end", cx);
    window
        .update(cx, |view, _, cx| {
            assert!(view.collection.read(cx).selected_ids().contains("id-39"));
            assert_eq!(view.outer, 0);
        })
        .unwrap();
    let resize = platform
        .accessibility_tree_update()
        .unwrap()
        .nodes
        .into_iter()
        .find(|(_, node)| node.label() == Some("Resize Name"))
        .unwrap()
        .0;
    platform.simulate_accessibility_action(ActionRequest {
        action: AccessibleAction::Focus,
        target_tree: TreeId::ROOT,
        target_node: resize,
        data: None,
    });
    cx.run_until_parked();
    send("right", cx);
    window
        .update(cx, |view, window, cx| {
            assert_eq!(view.collection.read(cx).columns()[0].width(), px(188.0));
            assert_eq!(view.outer, 0);
            view.outside.focus(window, cx);
        })
        .unwrap();
    cx.run_until_parked();
    send("down", cx);
    window
        .update(cx, |view, _, _| assert_eq!(view.outer, 1))
        .unwrap();
}

#[gpui::test]
fn filtered_reorder_preserves_hidden_rows_and_empty_results_allow_clearing_selection(
    cx: &mut TestAppContext,
) {
    let window = open(cx, 6, true, false);
    update(window, cx, |view, _, cx| {
        view.set_rows(
            [
                CollectionRow::new("visible-a", "Visible A"),
                CollectionRow::new("hidden", "Hidden selection"),
                CollectionRow::new("visible-b", "Visible B"),
                CollectionRow::new("visible-c", "Visible C"),
            ],
            cx,
        )
        .unwrap();
    });
    click(window, "Visible A", Modifiers::none(), cx);
    click(window, "Hidden selection", Modifiers::secondary_key(), cx);
    update(window, cx, |view, _, cx| view.set_filter("Visible", cx));
    action(
        window,
        "Records",
        AccessibleAction::CustomAction,
        Some(ActionData::CustomAction(2)),
        cx,
    );
    window
        .update(cx, |view, _, _| {
            assert_eq!(
                view.rows()
                    .iter()
                    .map(|row| row.id().as_ref())
                    .collect::<Vec<_>>(),
                ["hidden", "visible-b", "visible-a", "visible-c"]
            );
        })
        .unwrap();
    assert_eq!(selected(window, cx), ["hidden", "visible-a"]);
    update(window, cx, |view, _, cx| {
        view.set_filter("No matching row", cx)
    });
    assert_eq!(node(window, "Records", cx).1.row_count(), Some(0));
    key(window, "down", cx);
    assert_eq!(selected(window, cx), ["hidden", "visible-a"]);
    key(window, "escape", cx);
    assert!(selected(window, cx).is_empty());
}
