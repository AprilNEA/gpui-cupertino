use super::*;
use gpui::{Bounds, Pixels, size};

actions!(panel_focus_test, [NextControl, PreviousControl]);

fn init(cx: &mut gpui::App) {
    super::super::init(cx);
    cx.bind_keys([
        KeyBinding::new("tab", NextControl, Some("PanelFocusTest")),
        KeyBinding::new("shift-tab", PreviousControl, Some("PanelFocusTest")),
    ]);
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

#[derive(Clone, Copy, Debug)]
enum PanelKind {
    Popover,
    Dialog,
    Sheet(SheetEdge),
}

struct LongPanel {
    kind: PanelKind,
    panel: Entity<PanelState>,
    first: FocusHandle,
    last: FocusHandle,
}

impl Render for LongPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let trigger =
            Button::new("trigger", "Open").track_focus(self.panel.read(cx).trigger_focus_handle());
        let content = div()
            .relative()
            .w(px(200.))
            .h(px(900.))
            .child(
                Button::new("first", "First")
                    .w(px(200.))
                    .h(px(30.))
                    .track_focus(&self.first),
            )
            .child(
                div()
                    .id("last-slot")
                    .debug_selector(|| "panel-last-target".into())
                    .absolute()
                    .top(px(850.))
                    .w(px(200.))
                    .h(px(30.))
                    .child(
                        Button::new("last", "Last")
                            .size_full()
                            .track_focus(&self.last),
                    ),
            );
        let panel = match self.kind {
            PanelKind::Popover => {
                Popover::new(&self.panel, trigger, "Long popover", content).into_any_element()
            }
            PanelKind::Dialog => ModalHost::new(trigger)
                .dialog(Dialog::new(&self.panel, "Long dialog", content))
                .into_any_element(),
            PanelKind::Sheet(edge) => ModalHost::new(trigger)
                .sheet(Sheet::new(&self.panel, "Long sheet", content).edge(edge))
                .into_any_element(),
        };
        div()
            .key_context("PanelFocusTest")
            .size_full()
            .on_action(|_: &NextControl, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousControl, window, cx| window.focus_prev(cx))
            .child(panel)
    }
}

#[gpui::test]
fn oversized_panels_reveal_the_tab_target(cx: &mut TestAppContext) {
    cx.update(init);
    for kind in [
        PanelKind::Popover,
        PanelKind::Dialog,
        PanelKind::Sheet(SheetEdge::Top),
        PanelKind::Sheet(SheetEdge::Trailing),
        PanelKind::Sheet(SheetEdge::Bottom),
    ] {
        let (view, visual) = cx.add_window_view(|window, cx| {
            window.activate_window();
            let first = cx.focus_handle().tab_stop(true);
            LongPanel {
                kind,
                panel: cx.new(|cx| PanelState::new(cx).initial_focus(&first)),
                first,
                last: cx.focus_handle().tab_stop(true),
            }
        });
        visual.simulate_resize(size(px(360.), px(240.)));
        let panel = view.read_with(visual, |view, _| view.panel.clone());
        visual.update(|window, cx| panel.update(cx, |state, cx| state.open(window, cx)));
        settle(visual);
        let selector = match kind {
            PanelKind::Popover => "popover-panel",
            _ => "modal-panel",
        };
        assert!(
            visual.debug_bounds("panel-last-target").unwrap().top()
                > visual.debug_bounds(selector).unwrap().bottom()
        );
        press(visual, "tab");
        settle(visual);
        visual.update(|window, cx| assert!(view.read(cx).last.is_focused(window), "{kind:?}"));
        let target = visual.debug_bounds("panel-last-target").unwrap();
        let viewport = visual.debug_bounds(selector).unwrap();
        assert!(
            target.top() >= viewport.top(),
            "{kind:?}: {target:?} outside {viewport:?}"
        );
        assert!(
            target.bottom() <= viewport.bottom(),
            "{kind:?}: {target:?} outside {viewport:?}"
        );
        assert_eq!(
            target.bottom(),
            viewport.bottom(),
            "{kind:?} must use the minimum displacement"
        );
    }
}

struct MenuProbe {
    menu: Entity<MenuState>,
    before: FocusHandle,
    after: FocusHandle,
}

impl Render for MenuProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let trigger = Button::new("menu-trigger", "Menu")
            .track_focus(self.menu.read(cx).trigger_focus_handle(cx));
        div()
            .key_context("PanelFocusTest")
            .size_full()
            .on_action(|_: &NextControl, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousControl, window, cx| window.focus_prev(cx))
            .child(Button::new("before", "Before").track_focus(&self.before))
            .child(Menu::new(&self.menu, trigger, "Actions"))
            .child(Button::new("after", "After").track_focus(&self.after))
    }
}

fn open_menu(
    cx: &mut TestAppContext,
    items: Vec<MenuItem>,
) -> (Entity<MenuProbe>, &mut VisualTestContext) {
    cx.update(init);
    let (view, visual) = cx.add_window_view(|window, cx| {
        window.activate_window();
        MenuProbe {
            menu: cx.new(|cx| MenuState::new(items, cx)),
            before: cx.focus_handle().tab_stop(true),
            after: cx.focus_handle().tab_stop(true),
        }
    });
    visual.simulate_resize(size(px(360.), px(240.)));
    let menu = view.read_with(visual, |view, _| view.menu.clone());
    visual.update(|window, cx| menu.update(cx, |menu, cx| menu.open(window, cx)));
    settle(visual);
    (view, visual)
}

fn focused_menu_bounds(cx: &mut VisualTestContext) -> gpui::accesskit::Rect {
    let update = tree(cx);
    let node = update
        .nodes
        .iter()
        .find(|(id, _)| *id == update.focus)
        .unwrap()
        .1
        .clone();
    assert_eq!(node.role(), Role::MenuItem);
    let bounds = node.bounds().unwrap();
    let scale = cx.update(|window, _| f64::from(window.scale_factor()));
    gpui::accesskit::Rect {
        x0: bounds.x0 / scale,
        y0: bounds.y0 / scale,
        x1: bounds.x1 / scale,
        y1: bounds.y1 / scale,
    }
}

fn assert_visible(bounds: gpui::accesskit::Rect, viewport: Bounds<Pixels>) {
    assert!(
        bounds.y0 >= f64::from(f32::from(viewport.top())),
        "{bounds:?} outside {viewport:?}"
    );
    assert!(
        bounds.y1 <= f64::from(f32::from(viewport.bottom())),
        "{bounds:?} outside {viewport:?}"
    );
}

#[gpui::test]
fn long_menu_end_and_home_reveal_the_selected_item(cx: &mut TestAppContext) {
    let items = (0..40)
        .map(|index| MenuItem::new(format!("item-{index}"), format!("Item {index}")))
        .collect();
    let (_, visual) = open_menu(cx, items);
    press(visual, "end");
    settle(visual);
    let viewport = visual.debug_bounds("popover-panel").unwrap();
    assert_visible(focused_menu_bounds(visual), viewport);
    press(visual, "home");
    settle(visual);
    assert_visible(focused_menu_bounds(visual), viewport);
}

#[gpui::test]
fn menus_without_enabled_items_close_on_tab_and_continue_traversal(cx: &mut TestAppContext) {
    for items in [
        vec![],
        vec![MenuItem::new("disabled", "Unavailable").disabled(true)],
    ] {
        let (view, visual) = open_menu(cx, items);
        let menu = view.read_with(visual, |view, _| view.menu.clone());
        press(visual, "tab");
        settle(visual);
        visual.update(|window, cx| {
            assert!(!menu.read(cx).is_open(cx));
            assert!(view.read(cx).after.is_focused(window));
        });
        visual.update(|window, cx| menu.update(cx, |menu, cx| menu.open(window, cx)));
        settle(visual);
        press(visual, "shift-tab");
        settle(visual);
        visual.update(|window, cx| {
            assert!(!menu.read(cx).is_open(cx));
            assert!(view.read(cx).before.is_focused(window));
        });
    }
}
