use super::*;

struct CachedHost {
    disabled: bool,
    child: Entity<InertView>,
}
impl Render for CachedHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        inert(
            self.child
                .clone()
                .cached(gpui::StyleRefinement::default().size_full()),
            self.disabled,
        )
    }
}

#[gpui::test]
fn cached_subtrees_rebuild_input_after_inert_transitions(cx: &mut TestAppContext) {
    let (host, cx) = cx.add_window_view(|_, cx| CachedHost {
        disabled: false,
        child: cx.new(|cx| InertView {
            disabled: false,
            focus: cx.focus_handle().tab_stop(true),
            clicks: 0,
            keys: 0,
        }),
    });
    let child = host.read_with(cx, |host, _| host.child.clone());
    let point = cx.debug_bounds("deferred-button").unwrap().center();
    for (disabled, expected) in [(false, 1), (true, 1), (true, 1), (false, 2), (false, 3)] {
        host.update(cx, |host, cx| {
            host.disabled = disabled;
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_click(point, Modifiers::none());
        assert_eq!(child.read_with(cx, |child, _| child.clicks), expected);
    }
}

#[gpui::test]
fn inert_cancels_keyboard_and_pointer_presses_across_reenable(cx: &mut TestAppContext) {
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
    let point = cx.debug_bounds("deferred-button").unwrap().center();
    cx.simulate_keystrokes("enter");
    for disabled in [true, false] {
        view.update(cx, |view, cx| {
            view.disabled = disabled;
            cx.notify();
        });
        cx.run_until_parked();
    }
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    assert_eq!(view.read_with(cx, |view, _| view.clicks), 0);
    cx.simulate_mouse_down(point, gpui::MouseButton::Left, Modifiers::none());
    for disabled in [true, false] {
        view.update(cx, |view, cx| {
            view.disabled = disabled;
            cx.notify();
        });
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(point, gpui::MouseButton::Left, Modifiers::none());
    assert_eq!(view.read_with(cx, |view, _| view.clicks), 0);
    cx.simulate_click(point, Modifiers::none());
    assert_eq!(view.read_with(cx, |view, _| view.clicks), 1);
}
