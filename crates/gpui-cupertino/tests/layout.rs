//! Exercise scroll boundaries, child input ownership, and form validation semantics.

use gpui::{
    AccessibleAction, Context, FocusHandle, IntoElement, KeyDownEvent, Render, Role, ScrollHandle,
    TestAppContext, Window,
    accesskit::{ActionRequest, Live, TreeId},
    div,
    prelude::*,
    px,
};
use gpui_cupertino::components::{
    Button, EmptyState, Form, FormField, FormSection, ScrollArea, ScrollAxes, Toolbar,
};

struct Probe {
    scroll: ScrollHandle,
    child_focus: FocusHandle,
    content_height: f32,
    axes: ScrollAxes,
    invalid: bool,
    child_keys: usize,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(
                ScrollArea::new("viewport", "Document")
                    .axes(self.axes)
                    .track_scroll(&self.scroll)
                    .w(px(240.))
                    .h(px(120.))
                    .child(
                        div().w(px(600.)).h(px(self.content_height)).child(
                            div()
                                .id("child-input")
                                .track_focus(&self.child_focus)
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, _| {
                                    if event.keystroke.key == "down" {
                                        this.child_keys += 1;
                                    }
                                }))
                                .child("Document content"),
                        ),
                    ),
            )
            .child(
                Form::new("preferences", "Preferences").child(
                    FormSection::new("account", "Account")
                        .description("Settings are kept locally.")
                        .child(
                            FormField::new(
                                "name",
                                "Display name",
                                Button::new("edit-name", "Edit name"),
                            )
                            .description("Shown to other users.")
                            .when(self.invalid, |field| field.error("Enter a display name.")),
                        ),
                ),
            )
            .child(Toolbar::new("actions", "Actions").child(Button::new("save", "Save")))
            .child(EmptyState::new("empty", "No selection", "Choose an item."))
    }
}

#[gpui::test]
fn keyboard_and_accessibility_scroll_clamp_and_leave_child_keys_alone(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        Probe {
            scroll: ScrollHandle::new(),
            child_focus: cx.focus_handle().tab_stop(true),
            content_height: 720.,
            axes: ScrollAxes::Both,
            invalid: false,
            child_keys: 0,
        }
    });
    let handle = cx.update(|window, _| window.window_handle());
    let platform = cx.test_window(handle);
    platform.activate_accessibility();
    cx.run_until_parked();
    let node = platform
        .accessibility_tree_update()
        .unwrap()
        .nodes
        .into_iter()
        .find(|(_, node)| node.label() == Some("Document"))
        .unwrap();
    assert_eq!(node.1.role(), Role::ScrollView);
    for action in [
        AccessibleAction::Focus,
        AccessibleAction::ScrollDown,
        AccessibleAction::ScrollRight,
    ] {
        assert!(node.1.supports_action(action));
    }
    let action = |action| {
        platform.simulate_accessibility_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node.0,
            data: None,
        })
    };
    let scroll = view.read_with(cx, |view, _| view.scroll.clone());
    assert_eq!(scroll.max_offset().y, px(600.));
    action(AccessibleAction::Focus);
    cx.run_until_parked();
    cx.simulate_keystrokes("pagedown");
    assert_eq!(scroll.offset().y, px(-108.));
    cx.simulate_keystrokes("end");
    assert_eq!(scroll.offset().y, px(-600.));
    cx.simulate_keystrokes("down");
    assert_eq!(scroll.offset().y, px(-600.));
    cx.simulate_keystrokes("home");
    assert_eq!(scroll.offset().y, px(0.));
    action(AccessibleAction::ScrollRight);
    cx.run_until_parked();
    assert_eq!(scroll.offset().x, px(-216.));

    cx.update(|window, cx| {
        let focus = view.read(cx).child_focus.clone();
        focus.focus(window, cx);
    });
    cx.simulate_keystrokes("down");
    assert_eq!(scroll.offset().y, px(0.));
    assert_eq!(view.read_with(cx, |view, _| view.child_keys), 1);

    action(AccessibleAction::ScrollDown);
    cx.run_until_parked();
    assert_eq!(scroll.offset().y, px(-108.));
    view.update(cx, |view, cx| {
        view.content_height = 60.;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(scroll.max_offset().y, px(0.));
    assert_eq!(scroll.offset().y, px(0.));
}

#[gpui::test]
fn vertical_viewport_excludes_horizontal_actions_and_validation_alert_tracks_state(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Probe {
        scroll: ScrollHandle::new(),
        child_focus: cx.focus_handle().tab_stop(true),
        content_height: 720.,
        axes: ScrollAxes::Vertical,
        invalid: true,
        child_keys: 0,
    });
    let handle = cx.update(|window, _| window.window_handle());
    let platform = cx.test_window(handle);
    platform.activate_accessibility();
    cx.run_until_parked();
    let nodes = platform.accessibility_tree_update().unwrap().nodes;
    let viewport = &nodes
        .iter()
        .find(|(_, n)| n.label() == Some("Document"))
        .unwrap()
        .1;
    assert!(!viewport.supports_action(AccessibleAction::ScrollLeft));
    assert!(!viewport.supports_action(AccessibleAction::ScrollRight));
    assert!(
        nodes
            .iter()
            .any(|(_, n)| n.role() == Role::Form && n.label() == Some("Preferences"))
    );
    assert!(
        nodes
            .iter()
            .any(|(_, n)| n.role() == Role::Toolbar && n.label() == Some("Actions"))
    );
    assert!(nodes.iter().any(|(_, n)| n.role() == Role::Alert
        && n.label() == Some("Enter a display name.")
        && n.value() == Some("Enter a display name.")
        && n.live() == Some(Live::Assertive)));
    for (label, description) in [
        ("Account", "Settings are kept locally."),
        ("Display name", "Shown to other users."),
        ("No selection", "Choose an item."),
    ] {
        assert!(nodes.iter().any(|(_, node)| node.role() == Role::Group
            && node.label() == Some(label)
            && node.description() == Some(description)));
    }
    view.update(cx, |view, cx| {
        view.invalid = false;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        !platform
            .accessibility_tree_update()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, n)| n.role() == Role::Alert)
    );
}

#[gpui::test]
fn changing_scroll_axes_resets_only_the_excluded_offset(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Probe {
        scroll: ScrollHandle::new(),
        child_focus: cx.focus_handle().tab_stop(true),
        content_height: 720.,
        axes: ScrollAxes::Both,
        invalid: false,
        child_keys: 0,
    });
    cx.run_until_parked();
    let scroll = view.read_with(cx, |view, _| view.scroll.clone());
    for (axes, expected) in [
        (ScrollAxes::Vertical, gpui::point(px(0.), px(-90.))),
        (ScrollAxes::Horizontal, gpui::point(px(-80.), px(0.))),
    ] {
        scroll.set_offset(gpui::point(px(-80.), px(-90.)));
        view.update(cx, |view, cx| {
            view.axes = axes;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(scroll.offset(), expected);
    }
}
