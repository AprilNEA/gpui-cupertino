//! Exercise Button through GPUI's platform input, focus tree, and accessibility tree.

use gpui::{
    AccessibleAction, Context, FocusHandle, InputEvent, IntoElement, KeyDownEvent, KeyUpEvent,
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Render, Role, TestAppContext,
    Window, WindowHandle,
    accesskit::{ActionRequest, Node, NodeId, TreeId},
    div, point,
    prelude::*,
    px, size,
};
use gpui_cupertino::components::Button;

struct Probe {
    disabled: bool,
    clicks: usize,
    first: FocusHandle,
    last: FocusHandle,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap(px(16.0))
            .child(
                Button::new("first", "First")
                    .primary()
                    .w(px(100.0))
                    .h(px(32.0))
                    .track_focus(&self.first)
                    .aria_expanded(false)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.clicks += 1;
                        cx.notify();
                    }))
                    .disabled(self.disabled),
            )
            .child(
                Button::new("disabled", "Unavailable")
                    .disabled(true)
                    .w(px(100.0))
                    .h(px(32.0))
                    .on_click(cx.listener(|_, _, _, _| {
                        panic!("disabled button activated");
                    })),
            )
            .child(
                Button::new("last", "Last")
                    .w(px(100.0))
                    .h(px(32.0))
                    .track_focus(&self.last)
                    .on_click(|_, _, _| {}),
            )
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Probe> {
    let window = cx.open_window(size(px(400.0), px(100.0)), |_, cx| Probe {
        disabled: false,
        clicks: 0,
        first: cx.focus_handle(),
        last: cx.focus_handle(),
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

fn event(window: WindowHandle<Probe>, event: impl InputEvent, cx: &mut TestAppContext) {
    cx.test_window(window.into())
        .simulate_input(event.to_platform_input());
    cx.run_until_parked();
}

fn key_down(window: WindowHandle<Probe>, key: &str, cx: &mut TestAppContext) {
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

fn key_up(window: WindowHandle<Probe>, key: &str, cx: &mut TestAppContext) {
    event(
        window,
        KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        },
        cx,
    );
}

fn clicks(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> usize {
    window.update(cx, |view, _, _| view.clicks).unwrap()
}

fn set_disabled(window: WindowHandle<Probe>, disabled: bool, cx: &mut TestAppContext) {
    window
        .update(cx, |view, _, cx| {
            view.disabled = disabled;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
}

fn mouse_down(window: WindowHandle<Probe>, x: f32, cx: &mut TestAppContext) {
    event(
        window,
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(x), px(16.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
        cx,
    );
}

fn mouse_up(window: WindowHandle<Probe>, x: f32, cx: &mut TestAppContext) {
    event(
        window,
        MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(x), px(16.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
        },
        cx,
    );
}

#[gpui::test]
fn pointer_keyboard_and_accessibility_share_activation(cx: &mut TestAppContext) {
    let window = open(cx);
    mouse_down(window, 50.0, cx);
    assert_eq!(clicks(window, cx), 0);
    mouse_up(window, 50.0, cx);
    assert_eq!(clicks(window, cx), 1);

    for key in ["enter", "space"] {
        let before = clicks(window, cx);
        key_down(window, key, cx);
        assert_eq!(clicks(window, cx), before);
        key_up(window, key, cx);
        assert_eq!(clicks(window, cx), before + 1);
    }
    for key in ["cmd-enter", "shift-space"] {
        key_down(window, key, cx);
        key_up(window, key, cx);
    }
    assert_eq!(clicks(window, cx), 3);

    let (id, button) = node(window, "First", cx);
    assert_eq!(button.role(), Role::Button);
    assert_eq!(button.is_expanded(), Some(false));
    assert!(!button.is_disabled());
    assert!(button.supports_action(AccessibleAction::Focus));
    assert!(button.supports_action(AccessibleAction::Click));
    assert!(button.children().is_empty(), "label must be announced once");
    cx.test_window(window.into())
        .simulate_accessibility_action(ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: None,
        });
    cx.run_until_parked();
    assert_eq!(clicks(window, cx), 4);
}

#[gpui::test]
fn navigation_skips_disabled_buttons_and_cancels_an_interrupted_press(cx: &mut TestAppContext) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            window.focus_next(cx);
            assert!(view.first.is_focused(window));
        })
        .unwrap();
    cx.run_until_parked();
    key_down(window, "space", cx);
    window
        .update(cx, |view, window, cx| {
            window.focus_next(cx);
            assert!(view.last.is_focused(window));
            window.focus_prev(cx);
            assert!(view.first.is_focused(window));
        })
        .unwrap();
    cx.run_until_parked();
    key_up(window, "space", cx);
    assert_eq!(clicks(window, cx), 0);

    let (id, disabled) = node(window, "Unavailable", cx);
    assert!(disabled.is_disabled());
    assert!(!disabled.supports_action(AccessibleAction::Click));
    assert!(!disabled.supports_action(AccessibleAction::Focus));
    mouse_down(window, 166.0, cx);
    mouse_up(window, 166.0, cx);
    for action in [AccessibleAction::Click, AccessibleAction::Focus] {
        cx.test_window(window.into())
            .simulate_accessibility_action(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: id,
                data: None,
            });
        cx.run_until_parked();
    }
    assert_eq!(clicks(window, cx), 0);
    window
        .update(cx, |view, window, _| assert!(view.first.is_focused(window)))
        .unwrap();
}

#[gpui::test]
fn disabling_during_a_press_cancels_activation(cx: &mut TestAppContext) {
    let window = open(cx);
    mouse_down(window, 50.0, cx);
    set_disabled(window, true, cx);
    mouse_up(window, 50.0, cx);
    for key in ["enter", "space"] {
        key_down(window, key, cx);
        key_up(window, key, cx);
    }
    assert_eq!(clicks(window, cx), 0);
    assert!(node(window, "First", cx).1.is_disabled());

    set_disabled(window, false, cx);
    mouse_up(window, 50.0, cx);
    assert_eq!(
        clicks(window, cx),
        0,
        "disabled press must not survive re-enabling"
    );
    mouse_down(window, 50.0, cx);
    mouse_up(window, 50.0, cx);
    assert_eq!(clicks(window, cx), 1);

    key_down(window, "enter", cx);
    set_disabled(window, true, cx);
    key_up(window, "enter", cx);
    set_disabled(window, false, cx);
    key_up(window, "enter", cx);
    assert_eq!(
        clicks(window, cx),
        1,
        "disabled keyboard press must be cancelled"
    );
}
