use super::*;
use gpui::{
    Context, Entity, FocusHandle, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Modifiers,
    Render, Role, Subscription, TestAppContext, VisualTestContext, Window, actions, deferred, div,
    inert, point, prelude::*, px, rgb,
};

fn press(cx: &mut VisualTestContext, key: &str) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse(key).unwrap(),
    });
}

fn tree(cx: &mut VisualTestContext) -> gpui::accesskit::TreeUpdate {
    let handle = cx.update(|window, _| window.window_handle());
    let platform = cx.test_window(handle);
    platform.activate_accessibility();
    cx.run_until_parked();
    platform.accessibility_tree_update().unwrap()
}

struct Nested {
    parent: Entity<PanelState>,
    child: Entity<PanelState>,
    outside: usize,
}

impl Render for Nested {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let parent_trigger = Button::new("parent-trigger", "Parent")
            .track_focus(self.parent.read(cx).trigger_focus_handle())
            .on_click(cx.listener(|this, _, window, cx| {
                this.parent.update(cx, |state, cx| state.toggle(window, cx))
            }));
        let child_trigger = Button::new("child-trigger", "Child")
            .track_focus(self.child.read(cx).trigger_focus_handle())
            .on_click(cx.listener(|this, _, window, cx| {
                this.child.update(cx, |state, cx| state.toggle(window, cx))
            }));
        div()
            .size_full()
            .child(
                div()
                    .absolute()
                    .left(px(100.))
                    .top(px(50.))
                    .child(Popover::new(
                        &self.parent,
                        parent_trigger,
                        "Parent panel",
                        div().w(px(240.)).h(px(180.)).child(Popover::new(
                            &self.child,
                            child_trigger,
                            "Child panel",
                            div().w(px(120.)).h(px(60.)).child("Nested"),
                        )),
                    )),
            )
            .child(
                div()
                    .id("outside")
                    .debug_selector(|| "outside".into())
                    .absolute()
                    .left(px(10.))
                    .top(px(10.))
                    .size(px(30.))
                    .on_click(cx.listener(|this, _, _, _| this.outside += 1)),
            )
    }
}

#[gpui::test]
fn nested_panels_close_one_per_input_and_close_descendants_programmatically(
    cx: &mut TestAppContext,
) {
    cx.update(super::init);
    let (view, cx) = cx.add_window_view(|_, cx| Nested {
        parent: cx.new(|cx| PanelState::new(cx)),
        child: cx.new(|cx| PanelState::new(cx)),
        outside: 0,
    });
    let (parent, child) = view.read_with(cx, |view, _| (view.parent.clone(), view.child.clone()));
    cx.update(|window, cx| parent.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| child.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    press(cx, "escape");
    assert!(parent.read_with(cx, |state, _| state.is_open()));
    assert!(!child.read_with(cx, |state, _| state.is_open()));
    cx.update(|window, cx| assert!(child.read(cx).trigger_focus_handle().is_focused(window)));
    cx.update(|window, cx| child.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    let outside = cx.debug_bounds("outside").unwrap().center();
    cx.simulate_click(outside, Modifiers::none());
    assert!(parent.read_with(cx, |state, _| state.is_open()));
    assert!(!child.read_with(cx, |state, _| state.is_open()));
    assert_eq!(view.read_with(cx, |view, _| view.outside), 1);
    cx.update(|window, cx| child.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| parent.update(cx, |state, cx| state.close(window, cx)));
    assert!(!parent.read_with(cx, |state, _| state.is_open()));
    assert!(!child.read_with(cx, |state, _| state.is_open()));
    cx.update(|window, cx| assert!(parent.read(cx).trigger_focus_handle().is_focused(window)));
}

struct MenuView {
    menu: Entity<MenuState>,
    selected: Vec<gpui::SharedString>,
    _subscription: Subscription,
}
impl MenuView {
    fn new(cx: &mut Context<Self>) -> Self {
        let menu = cx.new(|cx| {
            MenuState::new(
                [
                    MenuItem::new("one", "One"),
                    MenuItem::new("disabled", "Unavailable").disabled(true),
                    MenuItem::new("three", "Three"),
                ],
                cx,
            )
        });
        let subscription = cx.subscribe(&menu, |this, _, event: &MenuEvent, _| {
            this.selected.push(event.id.clone())
        });
        Self {
            menu,
            selected: Vec::new(),
            _subscription: subscription,
        }
    }
}
impl Render for MenuView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.menu.read(cx);
        let trigger = Button::new("menu-trigger", "Actions")
            .track_focus(state.trigger_focus_handle(cx))
            .aria_expanded(state.is_open(cx))
            .on_click(cx.listener(|this, _, window, cx| {
                this.menu.update(cx, |state, cx| state.toggle(window, cx))
            }));
        div()
            .p(px(40.))
            .child(Menu::new(&self.menu, trigger, "Actions menu"))
    }
}

#[gpui::test]
fn menu_skips_disabled_actions_wraps_and_restores_focus(cx: &mut TestAppContext) {
    cx.update(super::init);
    let (view, cx) = cx.add_window_view(|_, cx| MenuView::new(cx));
    let menu = view.read_with(cx, |view, _| view.menu.clone());
    cx.update(|window, cx| menu.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    for (key, expected) in [
        ("down", "Three"),
        ("down", "One"),
        ("up", "Three"),
        ("home", "One"),
        ("end", "Three"),
    ] {
        press(cx, key);
        let update = tree(cx);
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == update.focus)
                .unwrap()
                .1
                .label(),
            Some(expected)
        );
    }
    let update = tree(cx);
    let (disabled, node) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Unavailable"))
        .unwrap();
    assert!(node.is_disabled());
    let handle = cx.update(|window, _| window.window_handle());
    cx.test_window(handle)
        .simulate_accessibility_action(gpui::accesskit::ActionRequest {
            action: gpui::AccessibleAction::Click,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: *disabled,
            data: None,
        });
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.selected.is_empty()));
    press(cx, "enter");
    assert_eq!(
        view.read_with(cx, |view, _| view.selected.clone()),
        vec![gpui::SharedString::from("three")]
    );
    cx.update(|window, cx| {
        assert!(!menu.read(cx).is_open(cx));
        assert!(menu.read(cx).trigger_focus_handle(cx).is_focused(window));
    });
}

actions!(panel_test, [BackgroundAction]);
struct ModalView {
    panel: Entity<PanelState>,
    first: FocusHandle,
    last: FocusHandle,
    background_clicks: usize,
    background_keys: usize,
    sheet: bool,
}
impl Render for ModalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let trigger = self.panel.read(cx).trigger_focus_handle().clone();
        let background = div()
            .size_full()
            .bg(rgb(0x112233))
            .on_action(cx.listener(|this, _: &BackgroundAction, _, _| this.background_keys += 1))
            .child(
                Button::new("background", "Background")
                    .track_focus(&trigger)
                    .on_click(cx.listener(|this, _, _, _| this.background_clicks += 1)),
            );
        let content = div()
            .w(px(240.))
            .flex()
            .flex_col()
            .child(Button::new("first", "First").track_focus(&self.first))
            .child(Button::new("last", "Last").track_focus(&self.last));
        if self.sheet {
            ModalHost::new(background)
                .sheet(Sheet::new(&self.panel, "Editor sheet", content).edge(SheetEdge::Trailing))
        } else {
            ModalHost::new(background).dialog(Dialog::new(&self.panel, "Confirm change", content))
        }
    }
}

#[gpui::test]
fn modal_excludes_background_input_and_accessibility_and_cycles_focus(cx: &mut TestAppContext) {
    cx.update(|cx| {
        super::init(cx);
        cx.bind_keys([KeyBinding::new("ctrl-b", BackgroundAction, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| {
        let first = cx.focus_handle().tab_stop(true);
        let panel = cx.new(|cx| PanelState::new(cx).initial_focus(&first));
        ModalView {
            panel,
            first,
            last: cx.focus_handle().tab_stop(true),
            background_clicks: 0,
            background_keys: 0,
            sheet: false,
        }
    });
    let initial = tree(cx);
    let background_id = initial
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Background"))
        .unwrap()
        .0;
    let panel = view.read_with(cx, |view, _| view.panel.clone());
    cx.update(|window, cx| panel.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    let update = tree(cx);
    assert!(
        !update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Background"))
    );
    assert!(
        update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Dialog)
            .unwrap()
            .1
            .is_modal()
    );
    cx.update(|window, cx| {
        assert!(view.read(cx).first.is_focused(window));
        assert!(
            window
                .painted_quads()
                .iter()
                .any(|quad| quad.background == gpui::Hsla::from(rgb(0x112233)).into())
        );
    });
    press(cx, "tab");
    cx.update(|window, cx| assert!(view.read(cx).last.is_focused(window)));
    press(cx, "tab");
    cx.update(|window, cx| assert!(view.read(cx).first.is_focused(window)));
    press(cx, "shift-tab");
    cx.update(|window, cx| assert!(view.read(cx).last.is_focused(window)));
    press(cx, "ctrl-b");
    cx.simulate_click(point(px(10.), px(10.)), Modifiers::none());
    let handle = cx.update(|window, _| window.window_handle());
    cx.test_window(handle)
        .simulate_accessibility_action(gpui::accesskit::ActionRequest {
            action: gpui::AccessibleAction::Click,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: background_id,
            data: None,
        });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| (view.background_clicks, view.background_keys)),
        (0, 0)
    );
    press(cx, "escape");
    cx.update(|window, cx| assert!(panel.read(cx).trigger_focus_handle().is_focused(window)));
    press(cx, "ctrl-b");
    assert_eq!(view.read_with(cx, |view, _| view.background_keys), 1);
    view.update(cx, |view, cx| {
        view.sheet = true;
        cx.notify();
    });
    cx.update(|window, cx| panel.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    let bounds = cx.debug_bounds("modal-panel").unwrap();
    cx.update(|window, _| assert_eq!(bounds.right(), window.viewport_size().width));
    press(cx, "escape");
    assert!(!panel.read_with(cx, |state, _| state.is_open()));
}

struct HintView {
    hint: Entity<TooltipState>,
    focus: FocusHandle,
    outside: FocusHandle,
}
impl Render for HintView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .id("hint-target")
                    .debug_selector(|| "hint-target".into())
                    .w(px(100.))
                    .h(px(30.))
                    .child(Tooltip::new(
                        &self.hint,
                        Button::new("hint-button", "Help").track_focus(&self.focus),
                    )),
            )
            .child(Button::new("outside", "Outside").track_focus(&self.outside))
    }
}

#[gpui::test]
fn tooltip_honors_hover_delay_focus_escape_and_blur(cx: &mut TestAppContext) {
    cx.update(super::init);
    let (view, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let focus = cx.focus_handle().tab_stop(true);
        let hint = cx.new(|cx| TooltipState::new("Additional help", &focus, window, cx));
        HintView {
            hint,
            focus,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
    let hint = view.read_with(cx, |view, _| view.hint.clone());
    let bounds = cx.debug_bounds("hint-target").unwrap();
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
    assert!(!hint.read_with(cx, |hint, _| hint.is_visible()));
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
    cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
    assert!(!hint.read_with(cx, |hint, _| hint.is_visible()));
    cx.update(|window, cx| view.read(cx).focus.clone().focus(window, cx));
    cx.run_until_parked();
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
    assert!(
        tree(cx).nodes.iter().any(
            |(_, node)| node.role() == Role::Tooltip && node.label() == Some("Additional help")
        )
    );
    press(cx, "escape");
    assert!(!hint.read_with(cx, |hint, _| hint.is_visible()));
    cx.update(|window, cx| view.read(cx).outside.clone().focus(window, cx));
    cx.run_until_parked();
    cx.update(|window, cx| view.read(cx).focus.clone().focus(window, cx));
    cx.run_until_parked();
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
}

struct InertView {
    disabled: bool,
    focus: FocusHandle,
    clicks: usize,
    keys: usize,
}
impl Render for InertView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        inert(
            div().size_full().child(deferred(
                div()
                    .id("deferred-button")
                    .debug_selector(|| "deferred-button".into())
                    .role(Role::Button)
                    .aria_label("Deferred action")
                    .track_focus(&self.focus)
                    .tab_index(0)
                    .size(px(80.))
                    .bg(rgb(0xff0000))
                    .on_key_down(cx.listener(|this, _, _, _| this.keys += 1))
                    .on_click(cx.listener(|this, _, _, _| this.clicks += 1)),
            )),
            self.disabled,
        )
    }
}

#[gpui::test]
fn inert_preserves_deferred_paint_and_removes_stale_focus_input(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let focus = cx.focus_handle().tab_stop(true);
        focus.focus(window, cx);
        InertView {
            disabled: false,
            focus,
            clicks: 0,
            keys: 0,
        }
    });
    let initial = tree(cx);
    assert!(
        initial
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Deferred action"))
    );
    view.update(cx, |view, cx| {
        view.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();
    press(cx, "enter");
    let bounds = cx.debug_bounds("deferred-button").unwrap();
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert_eq!(
        view.read_with(cx, |view, _| (view.clicks, view.keys)),
        (0, 0)
    );
    assert!(
        !tree(cx)
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Deferred action"))
    );
    cx.update(|window, _| assert!(!window.painted_quads().is_empty()));
    view.update(cx, |view, cx| {
        view.disabled = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_click(bounds.center(), Modifiers::none());
    assert_eq!(view.read_with(cx, |view, _| view.clicks), 1);
}

#[path = "panels_tests/inert.rs"]
mod inert_contract;

#[path = "panels_tests/modal.rs"]
mod modal_contract;

#[path = "panels_tests/identity.rs"]
mod identity_contract;

#[path = "panels_tests/focus.rs"]
mod focus_contract;
