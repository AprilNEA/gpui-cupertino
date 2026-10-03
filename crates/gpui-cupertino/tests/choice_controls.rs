//! Exercise controlled choices through GPUI input, focus, accessibility, and frame scheduling.

use std::time::Duration;

use gpui::{
    AccessibleAction, Context, FocusHandle, InputEvent, IntoElement, KeyDownEvent, KeyUpEvent,
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Render, Role, SharedString,
    TestAppContext, Window, WindowHandle,
    accesskit::{ActionRequest, Node, NodeId, Toggled, TreeId},
    div, point,
    prelude::*,
    px, size,
};
use gpui_cupertino::components::{Button, CheckState, Checkbox, ChoiceOption, RadioGroup, Toggle};

struct Probe {
    checked: bool,
    check_state: CheckState,
    changes: usize,
    selected: Option<SharedString>,
    selections: Vec<SharedString>,
    options: Vec<(&'static str, bool)>,
    disabled: bool,
    before: FocusHandle,
    toggle: FocusHandle,
    checkbox: FocusHandle,
    after: FocusHandle,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(200.0))
            .child(Button::new("before", "Before").track_focus(&self.before))
            .child(
                Toggle::new("toggle", "Wireless", self.checked)
                    .track_focus(&self.toggle)
                    .disabled(self.disabled)
                    .on_change(cx.listener(|view, value, _, cx| {
                        view.checked = *value;
                        view.changes += 1;
                        cx.notify();
                    })),
            )
            .child(
                Checkbox::new("checkbox", "All folders", self.check_state)
                    .track_focus(&self.checkbox)
                    .disabled(self.disabled)
                    .on_change(cx.listener(|view, value, _, cx| {
                        view.check_state = *value;
                        view.changes += 1;
                        cx.notify();
                    })),
            )
            .child(
                RadioGroup::new(
                    "radios",
                    "Destination",
                    self.options
                        .iter()
                        .map(|(id, disabled)| ChoiceOption::new(*id, *id).disabled(*disabled)),
                )
                .when_some(self.selected.clone(), |group, selected| {
                    group.selected(selected)
                })
                .disabled(self.disabled)
                .on_change(cx.listener(|view, id: &SharedString, _, cx| {
                    view.selected = Some(id.clone());
                    view.selections.push(id.clone());
                    cx.notify();
                })),
            )
            .child(Button::new("after", "After").track_focus(&self.after))
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Probe> {
    cx.update(gpui_cupertino::init);
    let window = cx.open_window(size(px(400.0), px(400.0)), |_, cx| Probe {
        checked: false,
        check_state: CheckState::Mixed,
        changes: 0,
        selected: Some("A".into()),
        selections: Vec::new(),
        options: vec![("A", false), ("B", true), ("C", false), ("D", false)],
        disabled: false,
        before: cx.focus_handle(),
        toggle: cx.focus_handle(),
        checkbox: cx.focus_handle(),
        after: cx.focus_handle(),
    });
    cx.test_window(window.into()).activate_accessibility();
    cx.run_until_parked();
    window
}

fn node(window: WindowHandle<Probe>, label: &str, cx: &TestAppContext) -> (NodeId, Node) {
    cx.test_window(window.into())
        .accessibility_tree_update()
        .unwrap()
        .nodes
        .into_iter()
        .find(|(_, node)| node.label() == Some(label))
        .unwrap()
}

fn focused(window: WindowHandle<Probe>, label: &str, cx: &TestAppContext) -> bool {
    let tree = cx
        .test_window(window.into())
        .accessibility_tree_update()
        .unwrap();
    tree.focus == node(window, label, cx).0
}

fn event(window: WindowHandle<Probe>, event: impl InputEvent, cx: &mut TestAppContext) {
    cx.test_window(window.into())
        .simulate_input(event.to_platform_input());
    cx.run_until_parked();
}

fn down(window: WindowHandle<Probe>, key: &str, cx: &mut TestAppContext) {
    event(
        window,
        KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        },
        cx,
    );
}

fn key(window: WindowHandle<Probe>, key: &str, cx: &mut TestAppContext) {
    down(window, key, cx);
    event(
        window,
        KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        },
        cx,
    );
}

fn pointer(window: WindowHandle<Probe>, label: &str, pressed: bool, cx: &mut TestAppContext) {
    let bounds = node(window, label, cx).1.bounds().unwrap();
    let scale = window
        .update(cx, |_, window, _| f64::from(window.scale_factor()))
        .unwrap();
    let position = point(
        px(((bounds.x0 + bounds.x1) / (2.0 * scale)) as f32),
        px(((bounds.y0 + bounds.y1) / (2.0 * scale)) as f32),
    );
    if pressed {
        event(
            window,
            MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            },
            cx,
        );
    } else {
        event(
            window,
            MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::none(),
                click_count: 1,
            },
            cx,
        );
    }
}

fn click(window: WindowHandle<Probe>, label: &str, cx: &mut TestAppContext) {
    pointer(window, label, true, cx);
    pointer(window, label, false, cx);
}

fn action(
    window: WindowHandle<Probe>,
    label: &str,
    action: AccessibleAction,
    cx: &mut TestAppContext,
) {
    cx.test_window(window.into())
        .simulate_accessibility_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node(window, label, cx).0,
            data: None,
        });
    cx.run_until_parked();
}

fn update(
    window: WindowHandle<Probe>,
    cx: &mut TestAppContext,
    f: impl FnOnce(&mut Probe, &mut Window, &mut Context<Probe>),
) {
    window
        .update(cx, |view, window, cx| {
            f(view, window, cx);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
}

#[gpui::test]
fn pointer_keyboard_and_accessibility_preserve_boolean_and_mixed_values(cx: &mut TestAppContext) {
    let window = open(cx);
    let switch = node(window, "Wireless", cx).1;
    assert_eq!(switch.role(), Role::Switch);
    assert_eq!(switch.toggled(), Some(Toggled::False));
    assert!(switch.children().is_empty());
    assert_eq!(
        node(window, "All folders", cx).1.toggled(),
        Some(Toggled::Mixed)
    );

    pointer(window, "Wireless", true, cx);
    assert_eq!(
        node(window, "Wireless", cx).1.toggled(),
        Some(Toggled::False)
    );
    pointer(window, "Wireless", false, cx);
    assert_eq!(
        node(window, "Wireless", cx).1.toggled(),
        Some(Toggled::True)
    );
    key(window, "space", cx);
    assert_eq!(
        node(window, "Wireless", cx).1.toggled(),
        Some(Toggled::False)
    );
    action(window, "Wireless", AccessibleAction::Click, cx);
    assert_eq!(
        node(window, "Wireless", cx).1.toggled(),
        Some(Toggled::True)
    );
    key(window, "cmd-space", cx);
    key(window, "shift-enter", cx);
    window
        .update(cx, |view, _, _| assert_eq!(view.changes, 3))
        .unwrap();

    action(window, "All folders", AccessibleAction::Click, cx);
    let checkbox = node(window, "All folders", cx).1;
    assert_eq!(checkbox.role(), Role::CheckBox);
    assert_eq!(checkbox.toggled(), Some(Toggled::True));
    assert!(checkbox.children().is_empty());
    action(window, "All folders", AccessibleAction::Focus, cx);
    key(window, "space", cx);
    assert_eq!(
        node(window, "All folders", cx).1.toggled(),
        Some(Toggled::False)
    );
    window
        .update(cx, |view, _, _| assert_eq!(view.changes, 5))
        .unwrap();
}

#[gpui::test]
fn disabling_choices_cancels_pending_activation_and_removes_tab_stops(cx: &mut TestAppContext) {
    let window = open(cx);
    pointer(window, "Wireless", true, cx);
    update(window, cx, |view, _, _| view.disabled = true);
    pointer(window, "Wireless", false, cx);
    for label in ["Wireless", "All folders", "A", "C"] {
        let disabled = node(window, label, cx).1;
        assert!(disabled.is_disabled());
        assert!(!disabled.supports_action(AccessibleAction::Click));
        assert!(!disabled.supports_action(AccessibleAction::Focus));
        click(window, label, cx);
        action(window, label, AccessibleAction::Click, cx);
    }
    update(window, cx, |view, window, cx| {
        view.before.focus(window, cx);
        window.focus_next(cx);
        assert!(view.after.is_focused(window));
    });
    update(window, cx, |view, _, _| view.disabled = false);
    pointer(window, "Wireless", false, cx);
    action(window, "All folders", AccessibleAction::Focus, cx);
    down(window, "space", cx);
    update(window, cx, |view, _, _| view.disabled = true);
    event(
        window,
        KeyUpEvent {
            keystroke: Keystroke::parse("space").unwrap(),
        },
        cx,
    );
    update(window, cx, |view, _, _| view.disabled = false);
    event(
        window,
        KeyUpEvent {
            keystroke: Keystroke::parse("space").unwrap(),
        },
        cx,
    );
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.changes, 0);
            assert!(view.selections.is_empty());
        })
        .unwrap();
}

#[gpui::test]
fn radio_navigation_wraps_skips_disabled_and_exposes_one_tab_stop(cx: &mut TestAppContext) {
    let window = open(cx);
    update(window, cx, |view, window, cx| {
        view.checkbox.focus(window, cx);
        window.focus_next(cx);
    });
    assert!(focused(window, "A", cx));
    assert_eq!(node(window, "Destination", cx).1.role(), Role::RadioGroup);
    assert_eq!(node(window, "A", cx).1.role(), Role::RadioButton);
    for (key_name, destination) in [
        ("right", "C"),
        ("down", "D"),
        ("right", "A"),
        ("left", "D"),
        ("home", "A"),
        ("end", "D"),
        ("up", "C"),
    ] {
        key(window, key_name, cx);
        assert!(focused(window, destination, cx), "{key_name}");
        assert_eq!(
            node(window, destination, cx).1.toggled(),
            Some(Toggled::True)
        );
        assert_eq!(node(window, "B", cx).1.toggled(), Some(Toggled::False));
    }
    key(window, "cmd-left", cx);
    key(window, "space", cx);
    window
        .update(cx, |view, _, _| assert_eq!(view.selections.len(), 7))
        .unwrap();
    update(window, cx, |view, window, cx| {
        window.focus_next(cx);
        assert!(view.after.is_focused(window));
    });
    update(window, cx, |_, window, cx| window.focus_prev(cx));
    assert!(focused(window, "C", cx));
    click(window, "D", cx);
    assert!(focused(window, "D", cx));
    action(window, "A", AccessibleAction::Click, cx);
    assert!(focused(window, "A", cx));
    window
        .update(cx, |view, _, _| assert_eq!(view.selections.len(), 9))
        .unwrap();
}

#[gpui::test]
fn radio_identity_survives_reorder_and_removed_focus_moves_without_changing_value(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    action(window, "C", AccessibleAction::Click, cx);
    let original = node(window, "C", cx).0;
    update(window, cx, |view, _, _| {
        view.options = vec![("D", false), ("C", false), ("B", true), ("A", false)]
    });
    assert_eq!(node(window, "C", cx).0, original);
    assert!(focused(window, "C", cx));
    key(window, "right", cx);
    assert!(focused(window, "A", cx));
    update(window, cx, |view, _, _| {
        view.options.retain(|(id, _)| *id != "A")
    });
    assert!(focused(window, "D", cx));
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.selected.as_deref(), Some("A"));
            assert_eq!(view.selections.len(), 2);
        })
        .unwrap();
    assert_eq!(node(window, "D", cx).1.toggled(), Some(Toggled::False));
    key(window, "space", cx);
    assert_eq!(node(window, "D", cx).1.toggled(), Some(Toggled::True));
    update(window, cx, |view, _, _| view.options[0].1 = true);
    assert!(focused(window, "C", cx));
}

#[gpui::test]
fn unselected_and_all_disabled_radio_groups_do_not_trap_tab(cx: &mut TestAppContext) {
    let window = open(cx);
    update(window, cx, |view, window, cx| {
        view.selected = None;
        view.checkbox.focus(window, cx);
    });
    update(window, cx, |_, window, cx| window.focus_next(cx));
    assert!(focused(window, "A", cx));
    update(window, cx, |view, _, _| {
        for (_, disabled) in &mut view.options {
            *disabled = true;
        }
    });
    for label in ["A", "B", "C", "D"] {
        assert!(
            !node(window, label, cx)
                .1
                .supports_action(AccessibleAction::Focus)
        );
    }
    update(window, cx, |view, window, cx| {
        view.checkbox.focus(window, cx);
        window.focus_next(cx);
        assert!(view.after.is_focused(window));
    });
    update(window, cx, |view, _, _| view.options.clear());
    update(window, cx, |view, window, cx| {
        view.checkbox.focus(window, cx);
        window.focus_next(cx);
        assert!(view.after.is_focused(window));
    });
}

fn frame(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> usize {
    cx.executor().advance_clock(Duration::from_millis(50));
    let scheduled = window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap();
    cx.run_until_parked();
    scheduled
}

#[gpui::test]
fn toggle_animates_value_changes_and_obeys_reduced_motion_in_flight(cx: &mut TestAppContext) {
    let window = open(cx);
    assert_eq!(
        frame(window, cx),
        0,
        "mounting must not animate from a false value"
    );
    update(window, cx, |view, _, _| view.checked = true);
    assert!(frame(window, cx) > 0);
    update(window, cx, |view, _, _| view.checked = false);
    assert!(frame(window, cx) > 0, "reversing must retain the spring");
    cx.update(|cx| cx.set_reduce_motion(true));
    cx.run_until_parked();
    frame(window, cx);
    assert_eq!(frame(window, cx), 0);
    update(window, cx, |view, _, _| view.checked = true);
    assert_eq!(
        frame(window, cx),
        0,
        "reduced motion must not schedule a transition"
    );
    assert_eq!(
        node(window, "Wireless", cx).1.toggled(),
        Some(Toggled::True)
    );
}
