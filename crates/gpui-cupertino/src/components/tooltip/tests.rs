use super::*;
use crate::components::Button;
use gpui::{Modifiers, Render, TestAppContext, VisualTestContext, inert};

#[derive(Clone, PartialEq, gpui::Action)]
#[action(no_json, no_register)]
struct ParentEscape;

#[derive(Clone, PartialEq, gpui::Action)]
#[action(no_json, no_register)]
struct OverrideEscape;

struct Probe {
    hint: Entity<TooltipState>,
    trigger: FocusHandle,
    outside: FocusHandle,
    mounted: bool,
    inert: bool,
    parent_actions: usize,
    overrides: usize,
    captures: usize,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context("TooltipHost")
            .on_action(cx.listener(|view, _: &ParentEscape, _, _| view.parent_actions += 1))
            .on_action(cx.listener(|view, _: &OverrideEscape, _, _| view.overrides += 1))
            .capture_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, _| {
                if event.keystroke.key == "escape" {
                    view.captures += 1;
                }
            }))
            .when(self.mounted, |root| {
                root.child(inert(
                    div()
                        .id("hint-target")
                        .debug_selector(|| "hint-target".into())
                        .w(px(100.))
                        .h(px(30.))
                        .child(Tooltip::new(
                            &self.hint,
                            Button::new("hint-button", "Help").track_focus(&self.trigger),
                        )),
                    self.inert,
                ))
            })
            .child(Button::new("outside", "Outside").track_focus(&self.outside))
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<Probe>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::init(cx);
        cx.bind_keys([KeyBinding::new("escape", ParentEscape, Some("TooltipHost"))]);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let trigger = cx.focus_handle().tab_stop(true);
        let outside = cx.focus_handle().tab_stop(true);
        outside.focus(window, cx);
        Probe {
            hint: cx.new(|cx| TooltipState::new("Help text", &trigger, window, cx)),
            trigger,
            outside,
            mounted: true,
            inert: false,
            parent_actions: 0,
            overrides: 0,
            captures: 0,
        }
    });
    cx.run_until_parked();
    (view, cx)
}

fn hover(cx: &mut VisualTestContext) {
    let bounds = cx.debug_bounds("hint-target").unwrap();
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
}

#[gpui::test]
fn hover_escape_dismisses_before_parent_handlers_and_waits_for_pointer_leave(
    cx: &mut TestAppContext,
) {
    let (view, cx) = open(cx);
    let hint = view.read_with(cx, |view, _| view.hint.clone());
    hover(cx);
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
    cx.simulate_keystrokes("escape");
    assert!(!hint.read_with(cx, |hint, _| hint.is_visible()));
    view.read_with(cx, |view, _| {
        assert_eq!(view.parent_actions, 0);
        assert_eq!(view.captures, 0);
    });
    cx.update(|window, cx| assert!(view.read(cx).outside.is_focused(window)));
    hover(cx);
    assert!(!hint.read_with(cx, |hint, _| hint.is_visible()));
    cx.simulate_keystrokes("escape");
    assert_eq!(view.read_with(cx, |view, _| view.parent_actions), 1);
    cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
    hover(cx);
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
}

#[gpui::test]
fn hidden_or_unmounted_hints_do_not_intercept_and_explicit_user_bindings_win(
    cx: &mut TestAppContext,
) {
    let (view, cx) = open(cx);
    let hint = view.read_with(cx, |view, _| view.hint.clone());
    hover(cx);
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
    view.update(cx, |view, cx| {
        view.inert = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    assert_eq!(view.read_with(cx, |view, _| view.parent_actions), 1);
    view.update(cx, |view, cx| {
        view.inert = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
    hover(cx);
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
    view.update(cx, |view, cx| {
        view.mounted = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    assert_eq!(view.read_with(cx, |view, _| view.parent_actions), 2);
    view.update(cx, |view, cx| {
        view.mounted = true;
        cx.notify();
    });
    cx.run_until_parked();
    hover(cx);
    assert!(hint.read_with(cx, |hint, _| hint.is_visible()));
    cx.update(|_, cx| cx.bind_keys([KeyBinding::new("escape", OverrideEscape, None)]));
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    assert_eq!(view.read_with(cx, |view, _| view.overrides), 1);
}
