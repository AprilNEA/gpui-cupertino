use super::*;
use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, Point, Render, Size, TestAppContext, canvas,
    size,
};
use std::{cell::RefCell, rc::Rc};

struct PopoverTest {
    popover: Entity<PopoverState>,
    content_focus: FocusHandle,
    outside_focus: FocusHandle,
    origin: Point<Pixels>,
    content_size: Size<Pixels>,
    paint_order: Rc<RefCell<Vec<&'static str>>>,
    outside_clicks: usize,
    escaped: usize,
}

impl PopoverTest {
    fn new(window: &mut Window, cx: &mut Context<Self>, initial_focus: bool) -> Self {
        let content_focus = cx.focus_handle().tab_stop(true);
        let popover = cx.new(|cx| {
            let state = PopoverState::new(cx);
            if initial_focus {
                state.initial_focus(&content_focus)
            } else {
                state
            }
        });
        let trigger_focus = popover.read(cx).trigger_focus.clone();
        trigger_focus.focus(window, cx);
        Self {
            popover,
            content_focus,
            outside_focus: cx.focus_handle().tab_stop(true),
            origin: point(px(100.), px(100.)),
            content_size: size(px(160.), px(120.)),
            paint_order: Rc::default(),
            outside_clicks: 0,
            escaped: 0,
        }
    }
}

impl Render for PopoverTest {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.popover.read(cx);
        let trigger = div()
            .id("trigger")
            .debug_selector(|| "trigger".into())
            .role(Role::Button)
            .aria_label("Show details")
            .aria_expanded(state.is_open())
            .track_focus(state.trigger_focus_handle())
            .w(px(80.))
            .h(px(24.))
            .on_click(cx.listener(|this, _, window, cx| {
                this.popover
                    .update(cx, |state, cx| state.toggle(window, cx));
            }));
        let panel_order = self.paint_order.clone();
        let sibling_order = self.paint_order.clone();
        let content = div()
            .id("content")
            .debug_selector(|| "content".into())
            .role(Role::Button)
            .aria_label("Content control")
            .track_focus(&self.content_focus)
            .w(self.content_size.width)
            .h(self.content_size.height)
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, _, _| panel_order.borrow_mut().push("panel"),
                )
                .size_full(),
            );

        div()
            .size_full()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, _| {
                if event.keystroke.key == "escape" {
                    this.escaped += 1;
                }
            }))
            .child(
                div()
                    .absolute()
                    .left(self.origin.x)
                    .top(self.origin.y)
                    .child(Popover::new(&self.popover, trigger, "Details", content)),
            )
            .child(
                div()
                    .id("outside")
                    .debug_selector(|| "outside".into())
                    .role(Role::Button)
                    .aria_label("Outside control")
                    .track_focus(&self.outside_focus)
                    .absolute()
                    .left(px(10.))
                    .top(px(10.))
                    .w(px(50.))
                    .h(px(30.))
                    .on_click(cx.listener(|this, _, _, _| this.outside_clicks += 1)),
            )
            .child(canvas(
                |_, _, _| (),
                move |_, _, _, _| sibling_order.borrow_mut().push("sibling"),
            ))
    }
}

#[gpui::test]
fn escape_closes_and_restores_trigger_focus(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| PopoverTest::new(window, cx, false));
    let state = view.read_with(cx, |view, _| view.popover.clone());

    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.update(|window, cx| {
        let state = state.read(cx);
        assert!(state.is_open());
        assert!(state.panel_focus.is_focused(window));
    });
    assert!(cx.debug_bounds("popover-panel").is_some());

    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        let state = state.read(cx);
        assert!(!state.is_open());
        assert!(state.trigger_focus.is_focused(window));
    });
    assert!(cx.debug_bounds("popover-panel").is_none());
    assert_eq!(view.read_with(cx, |view, _| view.escaped), 0);
}

#[gpui::test]
fn trigger_toggle_and_inside_click_do_not_dismiss_early(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| PopoverTest::new(window, cx, true));
    let state = view.read_with(cx, |view, _| view.popover.clone());
    let trigger = cx.debug_bounds("trigger").unwrap().center();

    cx.simulate_click(trigger, Modifiers::none());
    assert!(state.read_with(cx, |state, _| state.is_open()));
    cx.update(|window, cx| {
        assert!(view.read(cx).content_focus.is_focused(window));
    });

    let content = cx.debug_bounds("content").unwrap().center();
    cx.simulate_click(content, Modifiers::none());
    assert!(state.read_with(cx, |state, _| state.is_open()));

    cx.simulate_keystrokes("escape");
    assert!(!state.read_with(cx, |state, _| state.is_open()));
    cx.update(|window, cx| assert!(state.read(cx).trigger_focus.is_focused(window)));

    cx.simulate_click(trigger, Modifiers::none());
    assert!(state.read_with(cx, |state, _| state.is_open()));
    cx.simulate_click(trigger, Modifiers::none());
    assert!(!state.read_with(cx, |state, _| state.is_open()));
    cx.update(|window, cx| assert!(state.read(cx).trigger_focus.is_focused(window)));
}

#[gpui::test]
fn outside_click_reaches_destination_and_preserves_its_focus(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| PopoverTest::new(window, cx, true));
    let state = view.read_with(cx, |view, _| view.popover.clone());
    cx.update(|window, cx| state.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();

    let outside = cx.debug_bounds("outside").unwrap().center();
    cx.simulate_click(outside, Modifiers::none());
    cx.update(|window, cx| {
        assert!(!state.read(cx).is_open());
        assert!(view.read(cx).outside_focus.is_focused(window));
        assert_eq!(view.read(cx).outside_clicks, 1);
    });
}

#[gpui::test]
fn programmatic_close_does_not_steal_focus_after_it_leaves(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| PopoverTest::new(window, cx, true));
    let state = view.read_with(cx, |view, _| view.popover.clone());
    cx.update(|window, cx| state.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();

    cx.update(|window, cx| {
        let outside_focus = view.read(cx).outside_focus.clone();
        outside_focus.focus(window, cx);
        state.update(cx, |state, cx| state.close(window, cx));
        assert!(view.read(cx).outside_focus.is_focused(window));
    });
    cx.run_until_parked();
    assert!(!state.read_with(cx, |state, _| state.is_open()));
}

#[gpui::test]
fn panel_anchors_below_trigger_and_fits_after_resize(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| PopoverTest::new(window, cx, false));
    let state = view.read_with(cx, |view, _| view.popover.clone());
    cx.update(|window, cx| state.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();

    let trigger = cx.debug_bounds("trigger").unwrap();
    let panel = cx.debug_bounds("popover-panel").unwrap();
    assert_eq!(panel.origin, trigger.bottom_left() + point(px(0.), px(8.)));
    assert_eq!(panel.size, size(px(160.), px(120.)));

    cx.simulate_resize(size(px(240.), px(200.)));
    let panel = cx.debug_bounds("popover-panel").unwrap();
    assert!(panel.left() >= px(0.));
    assert!(panel.top() >= px(0.));
    assert!(panel.right() <= px(232.));
    assert!(panel.bottom() <= px(192.));

    view.update(cx, |view, cx| {
        view.content_size = size(px(900.), px(900.));
        cx.notify();
    });
    cx.run_until_parked();
    let panel = cx.debug_bounds("popover-panel").unwrap();
    assert_eq!(panel.size, size(px(224.), px(184.)));
    assert!(panel.left() >= px(0.));
    assert!(panel.top() >= px(0.));
    assert!(panel.right() <= px(240.));
    assert!(panel.bottom() <= px(200.));
}

#[gpui::test]
fn panel_paints_after_later_siblings(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| PopoverTest::new(window, cx, false));
    let (state, paint_order) = view.read_with(cx, |view, _| {
        (view.popover.clone(), view.paint_order.clone())
    });
    paint_order.borrow_mut().clear();
    cx.update(|window, cx| state.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();

    let paint_order = paint_order.borrow();
    assert_eq!(&paint_order[paint_order.len() - 2..], &["sibling", "panel"]);
}

#[gpui::test]
fn accessibility_reports_panel_focus_and_retains_identity_on_reopen(cx: &mut TestAppContext) {
    use gpui::{
        AccessibleAction,
        accesskit::{ActionRequest, TreeId},
    };

    let (_, cx) = cx.add_window_view(|window, cx| PopoverTest::new(window, cx, false));
    let handle = cx.update(|window, _| window.window_handle());
    let platform_window = cx.test_window(handle);
    platform_window.activate_accessibility();
    cx.run_until_parked();
    let tree = platform_window.accessibility_tree_update().unwrap();
    let (trigger_id, trigger_node) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Show details"))
        .unwrap();
    let trigger_id = *trigger_id;
    assert_eq!(trigger_node.is_expanded(), Some(false));
    assert!(
        !tree
            .nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Dialog)
    );

    platform_window.simulate_accessibility_action(ActionRequest {
        action: AccessibleAction::Click,
        target_tree: TreeId::ROOT,
        target_node: trigger_id,
        data: None,
    });
    cx.run_until_parked();
    let tree = platform_window.accessibility_tree_update().unwrap();
    let (panel_id, panel) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Dialog)
        .unwrap();
    let panel_id = *panel_id;
    assert_eq!(panel.label(), Some("Details"));
    assert_eq!(tree.focus, panel_id);
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == trigger_id)
            .unwrap()
            .1
            .is_expanded(),
        Some(true)
    );

    cx.simulate_keystrokes("escape");
    let tree = platform_window.accessibility_tree_update().unwrap();
    assert_eq!(tree.focus, trigger_id);
    assert!(
        !tree
            .nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Dialog)
    );

    let trigger = cx.debug_bounds("trigger").unwrap().center();
    cx.simulate_click(trigger, Modifiers::none());
    let tree = platform_window.accessibility_tree_update().unwrap();
    assert!(
        tree.nodes
            .iter()
            .any(|(id, node)| *id == panel_id && node.role() == Role::Dialog)
    );
    assert_eq!(tree.focus, panel_id);
}

#[cfg(target_os = "macos")]
#[gpui::test]
fn glass_captures_window_content_before_painting_popover_content(cx: &mut TestAppContext) {
    use crate::{
        materials::Glass,
        platform::{AccessibilityPreferences, set_test_preferences},
    };
    use cupertino::materials::{ClearGlassMaterial, GlassShape};
    use gpui::rgb;

    struct GlassPopover(Entity<PopoverState>);

    impl Render for GlassPopover {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let material =
                ClearGlassMaterial::new(GlassShape::RoundedRectangle { corner_radius: 12. }, 10.)
                    .unwrap();
            let content = Glass::clear(
                material,
                div()
                    .w(px(160.))
                    .h(px(120.))
                    .child(div().w(px(1.)).h(px(16.)).bg(rgb(0xffffff))),
            );
            div()
                .size_full()
                .bg(rgb(0x224466))
                .child(Popover::new(
                    &self.0,
                    div().w(px(80.)).h(px(24.)),
                    "Glass details",
                    content,
                ))
                .child(div().absolute().size(px(200.)).bg(rgb(0x446688)))
        }
    }

    cx.update(|cx| set_test_preferences(cx, AccessibilityPreferences::default()));
    let window = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let state = cx.new(|cx| PopoverState::new(cx));
        state.update(cx, |state, cx| state.open(window, cx));
        GlassPopover(state)
    });
    cx.run_until_parked();
    window
        .update(cx, |_, window, _| {
            let glass = window.painted_clear_backdrops();
            assert_eq!(glass.len(), 1);
            let glass = glass[0];
            let quads = window.painted_quads();
            assert_eq!(quads.len(), 3);
            let marker_width = px(1.).scale(window.scale_factor());
            let marker = quads
                .iter()
                .find(|quad| quad.bounds.size.width == marker_width)
                .unwrap();
            assert!(marker.order > glass.order);
            assert_eq!(marker.content_mask, glass.content_mask);
            for quad in quads
                .iter()
                .filter(|quad| quad.bounds.size.width != marker_width)
            {
                assert!(quad.order < glass.order);
            }
        })
        .unwrap();
}
