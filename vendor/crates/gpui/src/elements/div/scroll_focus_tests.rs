// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

//! Reveal newly focused descendants without retaining control of wheel scrolling.

use crate::{
    Context, Entity, FocusHandle, IntoElement, KeyBinding, Render, ScrollDelta, ScrollHandle,
    ScrollWheelEvent, StyleRefinement, TestAppContext, VisualTestContext, Window, deferred, div,
    inert, point, prelude::*, px,
};

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
            .child(div().id("last").size_full().track_focus(&self.last));
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
                div()
                    .id("first")
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
                        div()
                            .autoscroll_on_focus()
                            .id("inner")
                            .overflow_y_scroll()
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
                div()
                    .autoscroll_on_focus()
                    .id("outer")
                    .overflow_y_scroll()
                    .track_scroll(&self.outer)
                    .w(px(200.))
                    .h(px(100.))
                    .child(content),
            )
            .child(
                div()
                    .id("outside")
                    .absolute()
                    .left(px(220.))
                    .track_focus(&self.outside),
            )
    }
}

fn open(cx: &mut TestAppContext, nested: bool) -> (Entity<Probe>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("tab", Next, Some("ScrollFocusTest")),
            KeyBinding::new("shift-tab", Previous, Some("ScrollFocusTest")),
        ]);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let child = cx.new(|cx| Targets {
            first: cx.focus_handle().tab_stop(true),
            last: cx.focus_handle().tab_stop(true),
            inert: false,
            deferred: false,
            renders: 0,
        });
        let outside = cx.focus_handle().tab_stop(true);
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

#[test]
fn focus_reveal_handles_large_targets_and_clamps_scroll_offsets() {
    for (top, height, initial, expected) in [
        (40., 20., 0., 0.),
        (-20., 140., -100., -100.),
        (150., 200., 0., -150.),
        (-250., 200., -300., -150.),
        (1000., 30., 0., -400.),
    ] {
        let scroll = ScrollHandle::new();
        {
            let mut state = scroll.0.borrow_mut();
            state.bounds =
                crate::Bounds::new(point(px(0.), px(0.)), crate::size(px(100.), px(100.)));
            state.max_offset = point(px(400.), px(400.));
            state.overflow = point(crate::Overflow::Hidden, crate::Overflow::Scroll);
            *state.offset.borrow_mut() = point(px(0.), px(initial));
        }
        let target = crate::Bounds::new(point(px(200.), px(top)), crate::size(px(20.), px(height)));
        let revealed = scroll.reveal_bounds(target);
        assert_eq!(scroll.offset(), point(px(0.), px(expected)));
        assert_eq!(
            revealed,
            crate::Bounds::new(
                target.origin + point(px(0.), px(expected - initial)),
                target.size,
            )
        );
    }
}
