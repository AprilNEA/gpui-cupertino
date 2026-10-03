//! Verify splitter geometry, controlled resizing, focus, and accessibility dispatch.

use gpui::{
    AccessibleAction, Bounds, Context, FocusHandle, InputEvent, IntoElement, KeyBinding,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, Render, Role, TestAppContext, Window, WindowHandle,
    accesskit::{ActionData, ActionRequest, Node, NodeId, Orientation, TreeId},
    actions, div, point,
    prelude::*,
    px, size,
};
use gpui_cupertino::components::{Button, SplitView, ValueRange};

actions!(
    split_regression,
    [
        /// Consumes right-arrow navigation in the containing view.
        GlobalRight
    ]
);

struct Probe {
    ratio: f64,
    requests: Vec<f64>,
    accept: bool,
    vertical: bool,
    disabled: bool,
    width: f32,
    height: f32,
    first: FocusHandle,
    divider: FocusHandle,
    second: FocusHandle,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let first = Button::new("first", "First pane")
            .size_full()
            .min_w(px(600.0))
            .track_focus(&self.first);
        let second = Button::new("second", "Second pane")
            .size_full()
            .track_focus(&self.second);
        let split = SplitView::new("split", "Pane size", first, second)
            .range(ValueRange::new(0.0, 1.0, 0.1).unwrap())
            .unwrap()
            .ratio(self.ratio)
            .unwrap()
            .track_focus(&self.divider)
            .disabled(self.disabled)
            .w(px(self.width))
            .h(px(self.height))
            .on_change(cx.listener(|view, ratio, _, cx| {
                view.requests.push(*ratio);
                if view.accept {
                    view.ratio = *ratio;
                }
                cx.notify();
            }));
        div()
            .key_context("SplitHost")
            .on_action(|_: &GlobalRight, _, _| {})
            .p(px(16.0))
            .child(if self.vertical {
                split.vertical()
            } else {
                split
            })
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Probe> {
    cx.update(gpui_cupertino::init);
    let window = cx.open_window(size(px(700.0), px(400.0)), |_, cx| Probe {
        ratio: 0.5,
        requests: Vec::new(),
        accept: true,
        vertical: false,
        disabled: false,
        width: 400.0,
        height: 200.0,
        first: cx.focus_handle(),
        divider: cx.focus_handle(),
        second: cx.focus_handle(),
    });
    window
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.test_window(window.into()).activate_accessibility();
    cx.run_until_parked();
    window
}

#[gpui::test]
fn splitter_key_scope_precedes_a_conflicting_global_action(cx: &mut TestAppContext) {
    let window = open(cx);
    cx.update(|cx| cx.bind_keys([KeyBinding::new("right", GlobalRight, Some("SplitHost"))]));
    action(window, AccessibleAction::Focus, None, cx);
    key(window, "right", cx);
    close(ratio(window, cx), 0.6);
}

fn node(window: WindowHandle<Probe>, label: &str, cx: &TestAppContext) -> Option<(NodeId, Node)> {
    cx.test_window(window.into())
        .accessibility_tree_update()
        .unwrap()
        .nodes
        .into_iter()
        .find(|(_, node)| node.label() == Some(label))
}

fn divider(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> Bounds<Pixels> {
    let rect = node(window, "Pane size", cx).unwrap().1.bounds().unwrap();
    let scale = window
        .update(cx, |_, window, _| window.scale_factor())
        .unwrap() as f64;
    Bounds::new(
        point(px((rect.x0 / scale) as f32), px((rect.y0 / scale) as f32)),
        size(
            px((rect.width() / scale) as f32),
            px((rect.height() / scale) as f32),
        ),
    )
}

fn event(window: WindowHandle<Probe>, input: impl InputEvent, cx: &mut TestAppContext) {
    cx.test_window(window.into())
        .simulate_input(input.to_platform_input());
    cx.run_until_parked();
}

fn mouse_down(window: WindowHandle<Probe>, position: Point<Pixels>, cx: &mut TestAppContext) {
    event(
        window,
        MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
        cx,
    );
}

fn mouse_move(window: WindowHandle<Probe>, position: Point<Pixels>, cx: &mut TestAppContext) {
    event(
        window,
        MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        },
        cx,
    );
}

fn mouse_up(window: WindowHandle<Probe>, position: Point<Pixels>, cx: &mut TestAppContext) {
    event(
        window,
        MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 1,
        },
        cx,
    );
}

fn action(
    window: WindowHandle<Probe>,
    action: AccessibleAction,
    data: Option<ActionData>,
    cx: &mut TestAppContext,
) {
    let target_node = node(window, "Pane size", cx).unwrap().0;
    cx.test_window(window.into())
        .simulate_accessibility_action(ActionRequest {
            action,
            data,
            target_node,
            target_tree: TreeId::ROOT,
        });
    cx.run_until_parked();
}

fn key(window: WindowHandle<Probe>, key: &str, cx: &mut TestAppContext) {
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

fn ratio(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> f64 {
    window.update(cx, |view, _, _| view.ratio).unwrap()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.01, "{a} != {b}");
}

fn layout_position(
    window: WindowHandle<Probe>,
    actual: Pixels,
    expected: f64,
    cx: &mut TestAppContext,
) {
    let scale = window
        .update(cx, |_, window, _| window.scale_factor())
        .unwrap() as f64;
    // GPUI snaps resolved flex edges to device pixels; these cases avoid midpoint ties.
    close(actual.as_f32() as f64, (expected * scale).round() / scale);
}

#[gpui::test]
fn divider_uses_remaining_extent_and_resizes_outside_both_orientations(cx: &mut TestAppContext) {
    let window = open(cx);
    let bounds = divider(window, cx);
    layout_position(window, bounds.left(), 16.0 + 196.0, cx);
    layout_position(window, bounds.top(), 16.0, cx);
    assert_eq!(bounds.size, size(px(8.0), px(200.0)));
    assert_eq!(
        node(window, "Pane size", cx).unwrap().1.orientation(),
        Some(Orientation::Vertical)
    );
    let start = bounds.center();
    mouse_down(window, start, cx);
    assert!(
        window
            .update(cx, |view, _, _| view.requests.is_empty())
            .unwrap()
    );
    let destination = start + point(px(39.2), px(250.0));
    mouse_move(window, destination, cx);
    close(ratio(window, cx), 0.6);
    layout_position(window, divider(window, cx).left(), 16.0 + 392.0 * 0.6, cx);
    mouse_up(window, destination, cx);
    mouse_move(window, start, cx);
    close(ratio(window, cx), 0.6);

    window
        .update(cx, |view, _, cx| {
            view.vertical = true;
            view.ratio = 0.5;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let bounds = divider(window, cx);
    assert_eq!(bounds.size, size(px(400.0), px(8.0)));
    layout_position(window, bounds.top(), 16.0 + 96.0, cx);
    assert_eq!(
        node(window, "Pane size", cx).unwrap().1.orientation(),
        Some(Orientation::Horizontal)
    );
    let start = bounds.center();
    mouse_down(window, start, cx);
    let destination = start + point(px(350.0), px(-38.4));
    mouse_move(window, destination, cx);
    close(ratio(window, cx), 0.3);
    mouse_up(window, destination, cx);
    window
        .update(cx, |view, _, cx| {
            view.width = 600.0;
            view.height = 300.0;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let bounds = divider(window, cx);
    layout_position(window, bounds.top(), 16.0 + 292.0 * 0.3, cx);
    assert_eq!(bounds.size, size(px(600.0), px(8.0)));
}

#[gpui::test]
fn keyboard_and_accessibility_resize_and_disabled_drag_cannot_resume(cx: &mut TestAppContext) {
    let window = open(cx);
    let (_, splitter) = node(window, "Pane size", cx).unwrap();
    assert_eq!(splitter.role(), Role::Splitter);
    assert_eq!(splitter.numeric_value(), Some(0.5));
    assert_eq!(splitter.min_numeric_value(), Some(0.0));
    assert_eq!(splitter.max_numeric_value(), Some(1.0));
    assert_eq!(splitter.numeric_value_step(), Some(0.1));
    action(window, AccessibleAction::Focus, None, cx);
    key(window, "right", cx);
    close(ratio(window, cx), 0.6);
    key(window, "home", cx);
    close(ratio(window, cx), 0.0);
    key(window, "end", cx);
    close(ratio(window, cx), 1.0);
    action(window, AccessibleAction::Decrement, None, cx);
    close(ratio(window, cx), 0.9);
    action(
        window,
        AccessibleAction::SetValue,
        Some(ActionData::NumericValue(0.34)),
        cx,
    );
    close(ratio(window, cx), 0.3);
    action(
        window,
        AccessibleAction::SetValue,
        Some(ActionData::NumericValue(f64::NAN)),
        cx,
    );
    close(ratio(window, cx), 0.3);
    let start = divider(window, cx).center();
    mouse_down(window, start, cx);
    window
        .update(cx, |view, _, cx| {
            view.disabled = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    mouse_move(window, start + point(px(100.0), px(0.0)), cx);
    key(window, "home", cx);
    close(ratio(window, cx), 0.3);
    let (_, splitter) = node(window, "Pane size", cx).unwrap();
    assert!(splitter.is_disabled());
    for action in [
        AccessibleAction::Focus,
        AccessibleAction::Increment,
        AccessibleAction::Decrement,
        AccessibleAction::SetValue,
    ] {
        assert!(!splitter.supports_action(action));
    }
    window
        .update(cx, |view, window, cx| {
            assert!(!view.divider.is_focused(window));
            view.disabled = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    mouse_move(window, start + point(px(100.0), px(0.0)), cx);
    close(ratio(window, cx), 0.3);
    let start = divider(window, cx).center();
    mouse_down(window, start, cx);
    window
        .update(cx, |view, _, cx| {
            view.vertical = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let destination = divider(window, cx).center() + point(px(0.0), px(100.0));
    mouse_move(window, destination, cx);
    mouse_up(window, destination, cx);
    close(ratio(window, cx), 0.3);
}

#[gpui::test]
fn collapsed_panes_leave_focus_and_accessibility_and_requests_remain_controlled(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.first.focus(window, cx);
            window.focus_next(cx);
            assert!(view.divider.is_focused(window));
            window.focus_next(cx);
            assert!(view.second.is_focused(window));
            view.first.focus(window, cx);
            view.ratio = 0.0;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert!(node(window, "First pane", cx).is_none());
    assert!(node(window, "Second pane", cx).is_some());
    window
        .update(cx, |view, window, cx| {
            assert!(view.divider.is_focused(window));
            view.second.focus(window, cx);
            view.ratio = 1.0;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert!(node(window, "First pane", cx).is_some());
    assert!(node(window, "Second pane", cx).is_none());
    window
        .update(cx, |view, window, cx| {
            assert!(view.divider.is_focused(window));
            view.ratio = 0.5;
            view.accept = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    action(window, AccessibleAction::Increment, None, cx);
    close(ratio(window, cx), 0.5);
    close(
        window
            .update(cx, |view, _, _| *view.requests.last().unwrap())
            .unwrap(),
        0.6,
    );
    assert_eq!(
        node(window, "Pane size", cx).unwrap().1.numeric_value(),
        Some(0.5)
    );
}
