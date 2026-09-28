//! Exercise Cupertino parameters through GPUI's retained spring element.

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{
    AnimationExt, Context, IntoElement, Pixels, Render, TestAppContext, Window, WindowHandle, div,
    prelude::*, px, size,
};
use gpui_cupertino::{cupertino::motion::Spring, motion::spring};

struct Probe {
    target: Pixels,
    reference: bool,
    values: Rc<RefCell<Vec<Pixels>>>,
    reference_values: Rc<RefCell<Vec<Pixels>>>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let values = self.values.clone();
        let reference_values = self.reference_values.clone();
        let animation = spring(Spring::SNAPPY)
            .to(self.target)
            .from(px(0.0))
            .with_epsilon(0.01);
        div()
            .child(
                div().with_spring("retained", animation.clone(), move |element, value| {
                    values.borrow_mut().push(value);
                    element.left(value)
                }),
            )
            .when(self.reference, |element| {
                element.child(
                    div().with_spring("fresh", animation, move |element, value| {
                        reference_values.borrow_mut().push(value);
                        element.left(value)
                    }),
                )
            })
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Probe> {
    let window = cx.open_window(size(px(200.0), px(100.0)), |_, _| Probe {
        target: px(0.0),
        reference: false,
        values: Rc::default(),
        reference_values: Rc::default(),
    });
    cx.run_until_parked();
    window
}

fn target(window: WindowHandle<Probe>, value: f32, cx: &mut TestAppContext) {
    window
        .update(cx, |view, _, cx| {
            view.target = px(value);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
}

fn value(window: WindowHandle<Probe>, cx: &mut TestAppContext) -> Pixels {
    window
        .update(cx, |view, _, _| *view.values.borrow().last().unwrap())
        .unwrap()
}

fn frame(window: WindowHandle<Probe>, elapsed_ms: u64, cx: &mut TestAppContext) -> usize {
    cx.executor()
        .advance_clock(Duration::from_millis(elapsed_ms));
    let callbacks = window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap();
    cx.run_until_parked();
    callbacks
}

#[gpui::test(seeds(0, 1, 42))]
fn reversal_preserves_position_and_momentum_then_stops_requesting_frames(cx: &mut TestAppContext) {
    let window = open(cx);
    assert_eq!(value(window, cx), px(0.0));
    target(window, 100.0, cx);
    assert!(frame(window, 50, cx) > 0);
    let before = value(window, cx);
    assert!(before > px(0.0) && before < px(100.0));

    target(window, 0.0, cx);
    let at_reversal = value(window, cx);
    assert!((at_reversal - before).as_f32().abs() < 0.1);
    assert!(frame(window, 5, cx) > 0);
    assert!(
        value(window, cx) > at_reversal,
        "reversal discarded existing forward velocity"
    );

    target(window, 100.0, cx);
    assert!(frame(window, 5, cx) > 0);
    target(window, 0.0, cx);
    let settled = (0..240).any(|_| frame(window, 16, cx) == 0);
    assert!(settled, "the settled spring continued scheduling frames");
    assert_eq!(value(window, cx), px(0.0));
    assert_eq!(frame(window, 100, cx), 0);
}

#[gpui::test(seeds(0, 1, 42))]
fn enabling_reduced_motion_in_flight_snaps_and_does_not_restore_old_velocity(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    target(window, 100.0, cx);
    assert!(frame(window, 50, cx) > 0);
    assert!(value(window, cx) > px(0.0) && value(window, cx) < px(100.0));

    cx.update(|cx| cx.set_reduce_motion(true));
    cx.run_until_parked();
    assert_eq!(value(window, cx), px(100.0));
    frame(window, 16, cx); // Drain the frame requested before the preference changed.
    assert_eq!(frame(window, 16, cx), 0);
    target(window, 0.0, cx);
    assert_eq!(value(window, cx), px(0.0));
    assert_eq!(frame(window, 16, cx), 0);

    window
        .update(cx, |view, _, cx| {
            cx.set_reduce_motion(false);
            view.target = px(100.0);
            view.reference = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    for elapsed_ms in [5, 16, 33, 50] {
        assert!(frame(window, elapsed_ms, cx) > 0);
        window
            .update(cx, |view, _, _| {
                let retained = *view.values.borrow().last().unwrap();
                let fresh = *view.reference_values.borrow().last().unwrap();
                assert!(
                    (retained - fresh).as_f32().abs() < 0.01,
                    "re-enabled motion reused stale velocity: retained {retained:?}, fresh {fresh:?}"
                );
            })
            .unwrap();
    }
}

#[gpui::test(seeds(0, 1, 42))]
fn matching_times_match_at_sixty_and_one_twenty_hz_including_a_dropped_frame(
    cx: &mut TestAppContext,
) {
    let fast = open(cx);
    let slow = open(cx);
    target(fast, 100.0, cx);
    target(slow, 100.0, cx);
    for step in 1..=60 {
        cx.executor()
            .advance_clock(Duration::from_secs_f64(1.0 / 120.0));
        let draw_slow = step % 2 == 0 && step != 30;
        for window in [Some(fast), draw_slow.then_some(slow)]
            .into_iter()
            .flatten()
        {
            window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        if draw_slow {
            assert!(
                (value(fast, cx) - value(slow, cx)).as_f32().abs() < 0.05,
                "different frame cadence changed spring position at frame {step}"
            );
        }
    }
}
