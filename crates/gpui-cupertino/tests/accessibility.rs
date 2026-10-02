//! Verify the test platform's accessibility path before component-specific checks.

use gpui::{
    AccessibleAction, Context, FocusHandle, IntoElement, Render, Role, TestAppContext, Window,
    accesskit::{ActionRequest, TreeId},
    div,
    prelude::*,
    px, size,
};

struct Probe {
    activations: usize,
    focus: FocusHandle,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("activate")
            .role(Role::Button)
            .aria_label("Activate")
            .track_focus(&self.focus)
            .size(px(60.0))
            .on_click(cx.listener(|view, _, _, cx| {
                view.activations += 1;
                cx.notify();
            }))
    }
}

#[gpui::test]
fn platform_accessibility_activation_and_actions_reach_the_rendered_view(cx: &mut TestAppContext) {
    let window = cx.open_window(size(px(200.0), px(100.0)), |_, cx| Probe {
        activations: 0,
        focus: cx.focus_handle(),
    });
    let platform = cx.test_window(window.into());
    assert!(platform.accessibility_tree_update().is_none());

    platform.activate_accessibility();
    cx.run_until_parked();
    let tree = platform.accessibility_tree_update().unwrap();
    let (id, node) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Activate"))
        .unwrap();
    assert_eq!(node.role(), Role::Button);
    assert!(node.supports_action(AccessibleAction::Click));
    assert!(node.supports_action(AccessibleAction::Focus));

    for action in [AccessibleAction::Focus, AccessibleAction::Click] {
        platform.simulate_accessibility_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: *id,
            data: None,
        });
        cx.run_until_parked();
    }
    window
        .update(cx, |view, window, _| {
            assert_eq!(view.activations, 1);
            assert!(view.focus.is_focused(window));
        })
        .unwrap();
    assert_eq!(platform.accessibility_tree_update().unwrap().focus, *id);
}
