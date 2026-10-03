//! Reveal newly focused descendants without retaining control of wheel scrolling.

use gpui::{
    Context, Entity, FocusHandle, IntoElement, KeyBinding, Render, ScrollDelta, ScrollHandle,
    ScrollWheelEvent, StyleRefinement, TestAppContext, VisualTestContext, Window, actions,
    deferred, div, inert, point, prelude::*, px,
};
use gpui_cupertino::components::{Button, ScrollArea};

actions!(
    scroll_focus_test,
    [
        /// Move to the next test control.
        Next,
        /// Move to the previous test control.
        Previous,
    ]
);

struct Targets {
    first: FocusHandle,
    last: FocusHandle,
    inert: bool,
    deferred: bool,
    renders: usize,
}

impl Render for Targets {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let last = div()
            .id("last-slot")
            .debug_selector(|| "last-target".into())
            .absolute()
            .top(px(270.))
            .w(px(100.))
            .h(px(30.))
            .child(
                Button::new("last", "Last")
                    .size_full()
                    .track_focus(&self.last),
            );
        let last = if self.deferred {
            deferred(last).into_any_element()
        } else {
            inert(last, self.inert).into_any_element()
        };
        div()
            .relative()
            .w(px(200.))
            .h(px(400.))
            .child(
                Button::new("first", "First")
                    .w(px(100.))
                    .h(px(30.))
                    .track_focus(&self.first),
            )
            .child(last)
    }
}

struct Probe {
    child: Entity<Targets>,
    outer: ScrollHandle,
    inner: ScrollHandle,
    outside: FocusHandle,
    nested: bool,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let targets = self
            .child
            .clone()
            .cached(StyleRefinement::default().w(px(200.)).h(px(400.)));
        let content = if self.nested {
            div()
                .relative()
                .w(px(200.))
                .h(px(500.))
                .child(
                    div().absolute().top(px(220.)).child(
                        ScrollArea::new("inner", "Inner document")
                            .track_scroll(&self.inner)
                            .w(px(200.))
                            .h(px(80.))
                            .child(targets),
                    ),
                )
                .into_any_element()
        } else {
            targets.into_any_element()
        };
        div()
            .key_context("ScrollFocusTest")
            .size_full()
            .on_action(|_: &Next, window, cx| window.focus_next(cx))
            .on_action(|_: &Previous, window, cx| window.focus_prev(cx))
            .child(
                ScrollArea::new("outer", "Outer document")
                    .track_scroll(&self.outer)
                    .w(px(200.))
                    .h(px(100.))
                    .child(content),
            )
            .child(
                Button::new("outside", "Outside")
                    .absolute()
                    .left(px(220.))
                    .track_focus(&self.outside),
            )
    }
}

fn open(cx: &mut TestAppContext, nested: bool) -> (Entity<Probe>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_cupertino::init(cx);
        cx.bind_keys([
            KeyBinding::new("tab", Next, Some("ScrollFocusTest")),
            KeyBinding::new("shift-tab", Previous, Some("ScrollFocusTest")),
        ]);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let child = cx.new(|cx| Targets {
            first: cx.focus_handle(),
            last: cx.focus_handle(),
            inert: false,
            deferred: false,
            renders: 0,
        });
        let outside = cx.focus_handle();
        if nested {
            outside.focus(window, cx);
        } else {
            child.read(cx).first.clone().focus(window, cx);
        }
        Probe {
            child,
            outside,
            outer: ScrollHandle::new(),
            inner: ScrollHandle::new(),
            nested,
        }
    });
    settle(cx);
    (view, cx)
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

#[gpui::test]
fn tab_reveals_a_cached_descendant_minimally_and_wheel_scrolling_stays_free(
    cx: &mut TestAppContext,
) {
    let (view, cx) = open(cx, false);
    let (child, scroll) = view.read_with(cx, |view, _| (view.child.clone(), view.outer.clone()));
    let initial_renders = child.read_with(cx, |child, _| child.renders);
    view.update(cx, |_, cx| cx.notify());
    settle(cx);
    assert_eq!(
        child.read_with(cx, |child, _| child.renders),
        initial_renders
    );
    assert_eq!(scroll.offset().y, px(0.));

    cx.simulate_keystrokes("tab");
    settle(cx);
    cx.update(|window, cx| assert!(child.read(cx).last.is_focused(window)));
    assert!(child.read_with(cx, |child, _| child.renders) > initial_renders);
    assert_eq!(scroll.offset().y, px(-200.));
    assert_eq!(
        cx.debug_bounds("last-target").unwrap().bottom(),
        scroll.bounds().bottom()
    );

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.), px(50.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(60.))),
        ..Default::default()
    });
    settle(cx);
    assert_eq!(scroll.offset().y, px(-140.));
    cx.update(|window, _| window.refresh());
    settle(cx);
    assert_eq!(scroll.offset().y, px(-140.));
    cx.simulate_keystrokes("shift-tab");
    settle(cx);
    assert_eq!(scroll.offset().y, px(0.));
}

#[gpui::test]
fn nested_scroll_areas_resolve_inner_geometry_before_revealing_the_outer_target(
    cx: &mut TestAppContext,
) {
    let (view, cx) = open(cx, true);
    let (child, outer, inner) = view.read_with(cx, |view, _| {
        (view.child.clone(), view.outer.clone(), view.inner.clone())
    });
    assert_eq!(outer.offset().y, px(0.));
    assert_eq!(inner.offset().y, px(0.));
    cx.update(|window, cx| child.read(cx).last.clone().focus(window, cx));
    settle(cx);
    assert_eq!(inner.offset().y, px(-220.));
    assert_eq!(outer.offset().y, px(-200.));
    assert_eq!(
        cx.debug_bounds("last-target").unwrap().bottom(),
        outer.bounds().bottom()
    );
    cx.update(|window, _| window.refresh());
    settle(cx);
    assert_eq!(inner.offset().y, px(-220.));
    assert_eq!(outer.offset().y, px(-200.));
}

#[gpui::test]
fn inert_descendants_do_not_create_focus_reveal_targets(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, false);
    let (child, scroll) = view.read_with(cx, |view, _| (view.child.clone(), view.outer.clone()));
    child.update(cx, |child, cx| {
        child.inert = true;
        cx.notify();
    });
    settle(cx);
    cx.update(|window, cx| child.read(cx).last.clone().focus(window, cx));
    settle(cx);
    assert_eq!(scroll.offset().y, px(0.));
    cx.update(|window, cx| child.read(cx).first.clone().focus(window, cx));
    child.update(cx, |child, cx| {
        child.inert = false;
        cx.notify();
    });
    settle(cx);
    cx.update(|window, cx| child.read(cx).last.clone().focus(window, cx));
    settle(cx);
    assert_eq!(scroll.offset().y, px(-200.));
}

#[gpui::test]
fn deferred_descendants_do_not_scroll_the_background(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, false);
    let (child, scroll) = view.read_with(cx, |view, _| (view.child.clone(), view.outer.clone()));
    child.update(cx, |child, cx| {
        child.deferred = true;
        cx.notify();
    });
    settle(cx);
    cx.update(|window, cx| child.read(cx).last.clone().focus(window, cx));
    settle(cx);
    cx.update(|window, cx| assert!(child.read(cx).last.is_focused(window)));
    assert_eq!(scroll.offset().y, px(0.));
}
