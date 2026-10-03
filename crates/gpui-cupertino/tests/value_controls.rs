//! Exercise numeric controls, selection, and progress through GPUI dispatch.

use std::time::Duration;

use gpui::{
    AccessibleAction, Context, FocusHandle, InputEvent, IntoElement, KeyBinding, KeyDownEvent,
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, Role,
    SharedString, TestAppContext, Window, WindowHandle, WindowVisibility,
    accesskit::{ActionData, ActionRequest, Node, NodeId, Toggled, TreeId},
    actions, div, point,
    prelude::*,
    px, size,
};

actions!(
    numeric_regression,
    [
        /// Records navigation handled by the containing view.
        GlobalNavigation
    ]
);
use gpui_cupertino::components::{
    ChoiceOption, Progress, SegmentedControl, Slider, Stepper, ValueRange,
};

struct Probe {
    value: f64,
    changes: usize,
    disabled: bool,
    listening: bool,
    global_actions: usize,
    slider_focus: FocusHandle,
    stepper_focus: FocusHandle,
    selected: SharedString,
    progress: Option<f64>,
    show_progress: bool,
    clip_progress: bool,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let range = ValueRange::new(0.0, 10.0, 3.0).unwrap();
        div()
            .key_context("NumericHost")
            .on_action(cx.listener(|view, _: &GlobalNavigation, _, cx| {
                view.global_actions += 1;
                cx.notify();
            }))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                Slider::new("slider", "Volume", range)
                    .value(self.value)
                    .unwrap()
                    .w(px(200.0))
                    .h(px(28.0))
                    .track_focus(&self.slider_focus)
                    .disabled(self.disabled)
                    .when(self.listening, |slider| {
                        slider.on_change(cx.listener(|view, value, _, cx| {
                            view.value = *value;
                            view.changes += 1;
                            cx.notify();
                        }))
                    }),
            )
            .child(
                Stepper::new("stepper", "Quantity", range)
                    .value(self.value)
                    .unwrap()
                    .track_focus(&self.stepper_focus)
                    .disabled(self.disabled)
                    .on_change(cx.listener(|view, value, _, cx| {
                        view.value = *value;
                        view.changes += 1;
                        cx.notify();
                    })),
            )
            .child(
                SegmentedControl::new(
                    "segments",
                    "View mode",
                    [
                        ChoiceOption::new("list", "List"),
                        ChoiceOption::new("unavailable", "Unavailable").disabled(true),
                        ChoiceOption::new("grid", "Grid"),
                    ],
                )
                .selected(self.selected.clone())
                .disabled(self.disabled)
                .on_change(cx.listener(|view, selected: &SharedString, _, cx| {
                    view.selected = selected.clone();
                    cx.notify();
                })),
            )
            .when(self.show_progress, |root| {
                let progress = Progress::new("download", "Download");
                let progress = if let Some(value) = self.progress {
                    progress.value(value).unwrap()
                } else {
                    progress
                };
                root.child(
                    div()
                        .h(px(if self.clip_progress { 0.0 } else { 8.0 }))
                        .overflow_hidden()
                        .child(progress),
                )
            })
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Probe> {
    cx.update(gpui_cupertino::init);
    let window = cx.open_window(size(px(400.0), px(220.0)), |_, cx| Probe {
        value: 0.0,
        changes: 0,
        disabled: false,
        listening: true,
        global_actions: 0,
        slider_focus: cx.focus_handle(),
        stepper_focus: cx.focus_handle(),
        selected: "list".into(),
        progress: Some(0.25),
        show_progress: true,
        clip_progress: false,
    });
    window
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.test_window(window.into()).activate_accessibility();
    cx.run_until_parked();
    window
}

#[gpui::test]
fn removed_listener_cancels_drag_before_listener_is_restored(cx: &mut TestAppContext) {
    let window = open(cx);
    mouse_down(window, 100.0, cx);
    assert_eq!(value(window, cx), 6.0);
    window
        .update(cx, |view, _, cx| {
            view.listening = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    mouse_up(window, 280.0, cx);
    window
        .update(cx, |view, _, cx| {
            view.listening = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    mouse_move(window, 8.0, cx);
    mouse_up(window, 8.0, cx);
    assert_eq!(value(window, cx), 6.0);
    mouse_down(window, 8.0, cx);
    assert_eq!(value(window, cx), 0.0);
}

#[gpui::test]
fn numeric_key_scope_takes_precedence_over_global_navigation(cx: &mut TestAppContext) {
    let window = open(cx);
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("right", GlobalNavigation, Some("NumericHost")),
            KeyBinding::new("home", GlobalNavigation, Some("NumericHost")),
            KeyBinding::new("end", GlobalNavigation, Some("NumericHost")),
        ])
    });
    action(window, "Volume", AccessibleAction::Focus, None, cx);
    key(window, "right", cx);
    assert_eq!(value(window, cx), 3.0);
    key(window, "end", cx);
    assert_eq!(value(window, cx), 10.0);
    action(window, "Quantity", AccessibleAction::Focus, None, cx);
    key(window, "home", cx);
    assert_eq!(value(window, cx), 0.0);
    key(window, "right", cx);
    assert_eq!(value(window, cx), 3.0);
    assert_eq!(
        window.update(cx, |view, _, _| view.global_actions).unwrap(),
        0
    );
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

fn event(window: WindowHandle<Probe>, input: impl InputEvent, cx: &mut TestAppContext) {
    cx.test_window(window.into())
        .simulate_input(input.to_platform_input());
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

fn action(
    window: WindowHandle<Probe>,
    label: &str,
    action: AccessibleAction,
    data: Option<ActionData>,
    cx: &mut TestAppContext,
) {
    let (target_node, _) = node(window, label, cx);
    cx.test_window(window.into())
        .simulate_accessibility_action(ActionRequest {
            action,
            data,
            target_tree: TreeId::ROOT,
            target_node,
        });
    cx.run_until_parked();
}

fn value(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> f64 {
    window.update(cx, |view, _, _| view.value).unwrap()
}

fn mouse_down(window: WindowHandle<Probe>, x: f32, cx: &mut TestAppContext) {
    event(
        window,
        MouseDownEvent {
            position: point(px(x), px(14.0)),
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
        cx,
    );
}

fn mouse_move(window: WindowHandle<Probe>, x: f32, cx: &mut TestAppContext) {
    event(
        window,
        MouseMoveEvent {
            position: point(px(x), px(14.0)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        },
        cx,
    );
}

fn mouse_up(window: WindowHandle<Probe>, x: f32, cx: &mut TestAppContext) {
    event(
        window,
        MouseUpEvent {
            position: point(px(x), px(14.0)),
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 1,
        },
        cx,
    );
}

#[gpui::test]
fn slider_drag_survives_renders_and_cancels_outside_and_on_disable(cx: &mut TestAppContext) {
    let window = open(cx);
    mouse_down(window, 100.0, cx);
    assert_eq!(value(window, cx), 6.0);
    mouse_move(window, 280.0, cx);
    assert_eq!(value(window, cx), 10.0);
    mouse_up(window, 280.0, cx);
    mouse_move(window, 8.0, cx);
    assert_eq!(value(window, cx), 10.0);

    mouse_down(window, 8.0, cx);
    assert_eq!(value(window, cx), 0.0);
    window
        .update(cx, |view, _, cx| {
            view.disabled = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    mouse_move(window, 192.0, cx);
    assert_eq!(value(window, cx), 0.0);
    window
        .update(cx, |view, window, _| {
            assert!(!view.slider_focus.is_focused(window))
        })
        .unwrap();
    window
        .update(cx, |view, _, cx| {
            view.disabled = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    mouse_move(window, 192.0, cx);
    assert_eq!(value(window, cx), 0.0);
    mouse_down(window, 8.0, cx);
    mouse_up(window, 192.0, cx);
    assert_eq!(value(window, cx), 10.0);
}

#[gpui::test]
fn slider_keyboard_and_accessibility_share_bounds_and_quantization(cx: &mut TestAppContext) {
    let window = open(cx);
    let (_, slider) = node(window, "Volume", cx);
    assert_eq!(slider.role(), Role::Slider);
    assert_eq!(slider.min_numeric_value(), Some(0.0));
    assert_eq!(slider.max_numeric_value(), Some(10.0));
    assert_eq!(slider.numeric_value_step(), Some(3.0));
    for action in [
        AccessibleAction::Increment,
        AccessibleAction::Decrement,
        AccessibleAction::SetValue,
    ] {
        assert!(slider.supports_action(action));
    }
    action(window, "Volume", AccessibleAction::Focus, None, cx);
    for expected in [3.0, 6.0, 9.0, 10.0] {
        key(window, "right", cx);
        assert_eq!(value(window, cx), expected);
    }
    key(window, "up", cx);
    assert_eq!(window.update(cx, |view, _, _| view.changes).unwrap(), 4);
    key(window, "left", cx);
    assert_eq!(value(window, cx), 9.0);
    key(window, "home", cx);
    assert_eq!(value(window, cx), 0.0);
    key(window, "cmd-right", cx);
    assert_eq!(value(window, cx), 0.0);
    action(
        window,
        "Volume",
        AccessibleAction::SetValue,
        Some(ActionData::NumericValue(5.0)),
        cx,
    );
    assert_eq!(value(window, cx), 6.0);
    action(
        window,
        "Volume",
        AccessibleAction::SetValue,
        Some(ActionData::NumericValue(f64::NAN)),
        cx,
    );
    assert_eq!(value(window, cx), 6.0);
    action(window, "Volume", AccessibleAction::Increment, None, cx);
    assert_eq!(value(window, cx), 9.0);
    action(window, "Volume", AccessibleAction::Decrement, None, cx);
    assert_eq!(value(window, cx), 6.0);
    window
        .update(cx, |view, _, cx| {
            view.disabled = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let (_, slider) = node(window, "Volume", cx);
    assert!(slider.is_disabled());
    for supported in [
        AccessibleAction::Focus,
        AccessibleAction::Increment,
        AccessibleAction::Decrement,
        AccessibleAction::SetValue,
    ] {
        assert!(!slider.supports_action(supported));
        action(
            window,
            "Volume",
            supported,
            Some(ActionData::NumericValue(0.0)),
            cx,
        );
    }
    key(window, "home", cx);
    assert_eq!(value(window, cx), 6.0);
}

#[gpui::test]
fn stepper_has_one_tab_stop_and_disables_each_boundary_action(cx: &mut TestAppContext) {
    let window = open(cx);
    let (_, stepper) = node(window, "Quantity", cx);
    assert_eq!(stepper.role(), Role::SpinButton);
    let (_, decrease) = node(window, "Decrease Quantity", cx);
    assert!(decrease.is_disabled());
    assert!(!decrease.supports_action(AccessibleAction::Click));
    let (_, increase) = node(window, "Increase Quantity", cx);
    assert!(!increase.supports_action(AccessibleAction::Focus));
    action(
        window,
        "Increase Quantity",
        AccessibleAction::Click,
        None,
        cx,
    );
    assert_eq!(value(window, cx), 3.0);
    window
        .update(cx, |view, window, _| {
            assert!(view.stepper_focus.is_focused(window))
        })
        .unwrap();
    key(window, "end", cx);
    assert_eq!(value(window, cx), 10.0);
    assert!(node(window, "Increase Quantity", cx).1.is_disabled());
    action(
        window,
        "Decrease Quantity",
        AccessibleAction::Click,
        None,
        cx,
    );
    assert_eq!(value(window, cx), 9.0);
    action(
        window,
        "Quantity",
        AccessibleAction::SetValue,
        Some(ActionData::NumericValue(-10.0)),
        cx,
    );
    assert_eq!(value(window, cx), 0.0);
    window
        .update(cx, |view, window, cx| {
            view.slider_focus.focus(window, cx);
            window.focus_next(cx);
            assert!(view.stepper_focus.is_focused(window));
            window.focus_next(cx);
            assert!(!view.stepper_focus.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn segmented_selection_skips_disabled_options_and_exposes_radio_states(cx: &mut TestAppContext) {
    let window = open(cx);
    assert_eq!(node(window, "View mode", cx).1.role(), Role::RadioGroup);
    let (_, list) = node(window, "List", cx);
    assert_eq!(list.role(), Role::RadioButton);
    assert_eq!(list.toggled(), Some(Toggled::True));
    assert!(node(window, "Unavailable", cx).1.is_disabled());
    action(window, "List", AccessibleAction::Focus, None, cx);
    key(window, "right", cx);
    assert_eq!(
        window
            .update(cx, |view, _, _| view.selected.clone())
            .unwrap(),
        "grid"
    );
    assert_eq!(node(window, "Grid", cx).1.toggled(), Some(Toggled::True));
    key(window, "right", cx);
    assert_eq!(node(window, "List", cx).1.toggled(), Some(Toggled::True));
    key(window, "end", cx);
    assert_eq!(node(window, "Grid", cx).1.toggled(), Some(Toggled::True));
    action(window, "List", AccessibleAction::Click, None, cx);
    assert_eq!(node(window, "List", cx).1.toggled(), Some(Toggled::True));
    action(window, "Unavailable", AccessibleAction::Click, None, cx);
    assert_eq!(node(window, "List", cx).1.toggled(), Some(Toggled::True));
}

fn frame(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> usize {
    cx.executor().advance_clock(Duration::from_millis(16));
    let count = window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap();
    cx.run_until_parked();
    count
}

#[gpui::test]
fn progress_exposes_busy_value_and_stops_hidden_or_reduced_motion_frames(cx: &mut TestAppContext) {
    let window = open(cx);
    let (_, progress) = node(window, "Download", cx);
    assert_eq!(progress.role(), Role::ProgressIndicator);
    assert_eq!(progress.numeric_value(), Some(0.25));
    assert!(progress.is_busy());
    assert_eq!(frame(window, cx), 0);
    window
        .update(cx, |view, _, cx| {
            view.progress = None;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(node(window, "Download", cx).1.numeric_value(), None);
    assert!(frame(window, cx) > 0);
    cx.test_window(window.into())
        .simulate_visibility_change(WindowVisibility::Hidden);
    cx.run_until_parked();
    frame(window, cx);
    assert_eq!(frame(window, cx), 0);
    cx.test_window(window.into())
        .simulate_visibility_change(WindowVisibility::Visible);
    cx.run_until_parked();
    assert!(frame(window, cx) > 0);
    cx.update(|cx| cx.set_reduce_motion(true));
    cx.run_until_parked();
    frame(window, cx);
    assert_eq!(frame(window, cx), 0);
    window
        .update(cx, |view, _, cx| {
            view.clip_progress = true;
            cx.set_reduce_motion(false);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    frame(window, cx);
    assert_eq!(frame(window, cx), 0);
    window
        .update(cx, |view, _, cx| {
            view.clip_progress = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert!(frame(window, cx) > 0);
    window
        .update(cx, |view, _, cx| {
            view.show_progress = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    frame(window, cx);
    assert_eq!(frame(window, cx), 0);
    window
        .update(cx, |view, _, cx| {
            view.show_progress = true;
            view.progress = Some(1.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert!(!node(window, "Download", cx).1.is_busy());
    assert_eq!(frame(window, cx), 0);
}
