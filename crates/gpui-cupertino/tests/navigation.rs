//! Exercise controlled navigation and per-tab scroll identity through GPUI.

use gpui::{
    AccessibleAction, Context, InputEvent, IntoElement, KeyDownEvent, Keystroke, Render, Role,
    SharedString, TestAppContext, Window, WindowHandle,
    accesskit::{ActionRequest, Node, NodeId, TreeId},
    div,
    prelude::*,
    px, size,
};
use gpui_cupertino::components::{ChoiceOption, ScrollArea, Sidebar, Tab, Tabs};

struct Probe {
    tab: SharedString,
    sidebar: SharedString,
    sidebar_disabled: bool,
    requests: usize,
}

fn content(label: &'static str, marker: &'static str) -> impl IntoElement {
    ScrollArea::new("shared-scroll", label)
        .w(px(240.0))
        .h(px(160.0))
        .child(
            div()
                .id("content")
                .role(Role::Group)
                .aria_label(marker)
                .h(px(900.0))
                .w_full(),
        )
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                Tabs::new(
                    "tabs",
                    "Editor tabs",
                    self.tab.clone(),
                    [
                        Tab::new(
                            "first",
                            "First tab",
                            content("First scroll", "First content"),
                        ),
                        Tab::new(
                            "disabled",
                            "Disabled tab",
                            content("Disabled scroll", "Disabled content"),
                        )
                        .disabled(true),
                        Tab::new(
                            "second",
                            "Second tab",
                            content("Second scroll", "Second content"),
                        ),
                    ],
                )
                .w(px(360.0))
                .h(px(220.0))
                .on_change(cx.listener(|view, selected: &SharedString, _, cx| {
                    view.tab = selected.clone();
                    view.requests += 1;
                    cx.notify();
                })),
            )
            .child(
                Sidebar::new(
                    "sidebar",
                    "Folders",
                    [
                        ChoiceOption::new("inbox", "Inbox"),
                        ChoiceOption::new("disabled", "Unavailable folder").disabled(true),
                        ChoiceOption::new("archive", "Archive"),
                    ],
                )
                .selected(self.sidebar.clone())
                .disabled(self.sidebar_disabled)
                .on_change(cx.listener(|view, selected: &SharedString, _, cx| {
                    view.sidebar = selected.clone();
                    view.requests += 1;
                    cx.notify();
                })),
            )
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Probe> {
    cx.update(gpui_cupertino::init);
    let window = cx.open_window(size(px(500.0), px(500.0)), |_, _| Probe {
        tab: "first".into(),
        sidebar: "inbox".into(),
        sidebar_disabled: false,
        requests: 0,
    });
    window
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.test_window(window.into()).activate_accessibility();
    cx.run_until_parked();
    window
}

fn node(
    window: WindowHandle<Probe>,
    role: Role,
    label: &str,
    cx: &TestAppContext,
) -> Option<(NodeId, Node)> {
    cx.test_window(window.into())
        .accessibility_tree_update()
        .unwrap()
        .nodes
        .into_iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label))
}

fn action(
    window: WindowHandle<Probe>,
    role: Role,
    label: &str,
    action: AccessibleAction,
    cx: &mut TestAppContext,
) {
    let target_node = node(window, role, label, cx).unwrap().0;
    cx.test_window(window.into())
        .simulate_accessibility_action(ActionRequest {
            action,
            data: None,
            target_node,
            target_tree: TreeId::ROOT,
        });
    cx.run_until_parked();
}

fn key(window: WindowHandle<Probe>, key: &str, cx: &mut TestAppContext) {
    cx.test_window(window.into()).simulate_input(
        KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        }
        .to_platform_input(),
    );
    cx.run_until_parked();
}

fn selected(window: WindowHandle<Probe>, role: Role, label: &str, cx: &TestAppContext) -> bool {
    node(window, role, label, cx).unwrap().1.is_selected() == Some(true)
}

fn offset(
    window: WindowHandle<Probe>,
    viewport: &str,
    marker: &str,
    cx: &mut TestAppContext,
) -> f64 {
    let viewport = node(window, Role::ScrollView, viewport, cx)
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let content = node(window, Role::Group, marker, cx)
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let scale = window
        .update(cx, |_, window, _| window.scale_factor())
        .unwrap() as f64;
    (content.y0 - viewport.y0) / scale
}

#[gpui::test]
fn tabs_mount_only_selection_and_skip_disabled_keyboard_and_accessibility_targets(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    assert!(node(window, Role::TabList, "Editor tabs", cx).is_some());
    assert!(node(window, Role::TabPanel, "First tab", cx).is_some());
    assert!(node(window, Role::TabPanel, "Second tab", cx).is_none());
    assert!(node(window, Role::ScrollView, "Second scroll", cx).is_none());
    assert!(node(window, Role::ScrollView, "Disabled scroll", cx).is_none());
    assert!(selected(window, Role::Tab, "First tab", cx));
    let (_, disabled) = node(window, Role::Tab, "Disabled tab", cx).unwrap();
    assert!(disabled.is_disabled());
    assert!(!disabled.supports_action(AccessibleAction::Click));
    assert!(!disabled.supports_action(AccessibleAction::Focus));

    action(window, Role::Tab, "First tab", AccessibleAction::Focus, cx);
    key(window, "right", cx);
    assert!(selected(window, Role::Tab, "Second tab", cx));
    assert!(node(window, Role::TabPanel, "First tab", cx).is_none());
    assert!(node(window, Role::TabPanel, "Second tab", cx).is_some());
    action(
        window,
        Role::Tab,
        "Disabled tab",
        AccessibleAction::Click,
        cx,
    );
    action(
        window,
        Role::Tab,
        "Disabled tab",
        AccessibleAction::Focus,
        cx,
    );
    assert!(selected(window, Role::Tab, "Second tab", cx));
    key(window, "left", cx);
    assert!(selected(window, Role::Tab, "First tab", cx));
    key(window, "end", cx);
    assert!(selected(window, Role::Tab, "Second tab", cx));
    key(window, "cmd-left", cx);
    assert!(selected(window, Role::Tab, "Second tab", cx));
    action(window, Role::Tab, "First tab", AccessibleAction::Click, cx);
    assert!(selected(window, Role::Tab, "First tab", cx));
}

#[gpui::test]
fn unknown_tab_selection_mounts_no_panel_and_keyboard_recovers_from_first_enabled_tab(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.tab = "unknown".into();
            window.blur(cx);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    for label in ["First tab", "Disabled tab", "Second tab"] {
        assert!(node(window, Role::TabPanel, label, cx).is_none());
        assert!(!selected(window, Role::Tab, label, cx));
    }
    window
        .update(cx, |_, window, cx| window.focus_next(cx))
        .unwrap();
    cx.run_until_parked();
    let first = node(window, Role::Tab, "First tab", cx).unwrap().0;
    assert_eq!(
        cx.test_window(window.into())
            .accessibility_tree_update()
            .unwrap()
            .focus,
        first
    );
    key(window, "right", cx);
    assert!(selected(window, Role::Tab, "Second tab", cx));
    assert!(node(window, Role::TabPanel, "Second tab", cx).is_some());
}

#[gpui::test]
fn sidebar_selection_uses_vertical_navigation_and_disabling_removes_actions(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    assert!(node(window, Role::ListBox, "Folders", cx).is_some());
    assert!(selected(window, Role::ListBoxOption, "Inbox", cx));
    action(
        window,
        Role::ListBoxOption,
        "Inbox",
        AccessibleAction::Focus,
        cx,
    );
    key(window, "down", cx);
    assert!(selected(window, Role::ListBoxOption, "Archive", cx));
    key(window, "right", cx);
    assert!(selected(window, Role::ListBoxOption, "Archive", cx));
    key(window, "home", cx);
    assert!(selected(window, Role::ListBoxOption, "Inbox", cx));
    action(
        window,
        Role::ListBoxOption,
        "Archive",
        AccessibleAction::Click,
        cx,
    );
    action(
        window,
        Role::ListBoxOption,
        "Unavailable folder",
        AccessibleAction::Click,
        cx,
    );
    assert!(selected(window, Role::ListBoxOption, "Archive", cx));
    let requests = window
        .update(cx, |view, _, cx| {
            view.sidebar_disabled = true;
            cx.notify();
            view.requests
        })
        .unwrap();
    cx.run_until_parked();
    for label in ["Inbox", "Unavailable folder", "Archive"] {
        let (_, item) = node(window, Role::ListBoxOption, label, cx).unwrap();
        assert!(item.is_disabled());
        assert!(!item.supports_action(AccessibleAction::Click));
        assert!(!item.supports_action(AccessibleAction::Focus));
        action(
            window,
            Role::ListBoxOption,
            label,
            AccessibleAction::Click,
            cx,
        );
    }
    key(window, "home", cx);
    assert_eq!(
        window.update(cx, |view, _, _| view.requests).unwrap(),
        requests
    );
}

#[gpui::test]
fn tabs_namespace_identical_scroll_area_ids_without_leaking_scroll_offset(cx: &mut TestAppContext) {
    let window = open(cx);
    assert_eq!(offset(window, "First scroll", "First content", cx), 0.0);
    action(
        window,
        Role::ScrollView,
        "First scroll",
        AccessibleAction::ScrollDown,
        cx,
    );
    assert!(offset(window, "First scroll", "First content", cx) < -100.0);
    action(window, Role::Tab, "Second tab", AccessibleAction::Click, cx);
    assert_eq!(offset(window, "Second scroll", "Second content", cx), 0.0);
    action(
        window,
        Role::ScrollView,
        "Second scroll",
        AccessibleAction::ScrollDown,
        cx,
    );
    action(
        window,
        Role::ScrollView,
        "Second scroll",
        AccessibleAction::ScrollDown,
        cx,
    );
    let second_offset = offset(window, "Second scroll", "Second content", cx);
    assert!(second_offset < -200.0);
    action(window, Role::Tab, "First tab", AccessibleAction::Click, cx);
    assert_ne!(
        offset(window, "First scroll", "First content", cx),
        second_offset
    );
}
