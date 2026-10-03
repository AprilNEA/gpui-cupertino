//! Scoped choice actions must take precedence over application navigation bindings.

use gpui::{
    Context, FocusHandle, IntoElement, KeyBinding, Render, SharedString, TestAppContext, Window,
    actions, div, prelude::*, px, size,
};
use gpui_cupertino::components::{Button, ChoiceOption, RadioGroup};

actions!(
    choice_conflict,
    [
        /// Simulate a navigation command on the parent view.
        OuterNavigation
    ]
);

struct Probe {
    selected: SharedString,
    outer: usize,
    outside: FocusHandle,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("ChoiceHost")
            .on_action(cx.listener(|view, _: &OuterNavigation, _, _| view.outer += 1))
            .child(
                RadioGroup::new(
                    "choices",
                    "Choices",
                    [ChoiceOption::new("a", "A"), ChoiceOption::new("b", "B")],
                )
                .selected(self.selected.clone())
                .on_change(cx.listener(|view, id: &SharedString, _, cx| {
                    view.selected = id.clone();
                    cx.notify();
                })),
            )
            .child(Button::new("outside", "Outside").track_focus(&self.outside))
    }
}

#[gpui::test]
fn scoped_choice_actions_win_without_disabling_application_bindings(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_cupertino::init(cx);
        cx.bind_keys(
            ["right", "left", "up", "down", "home", "end"]
                .into_iter()
                .map(|key| KeyBinding::new(key, OuterNavigation, Some("ChoiceHost"))),
        );
    });
    let window = cx.open_window(size(px(320.0), px(180.0)), |_, cx| Probe {
        selected: "a".into(),
        outer: 0,
        outside: cx.focus_handle(),
    });
    cx.run_until_parked();
    window
        .update(cx, |_, window, cx| window.focus_next(cx))
        .unwrap();
    cx.run_until_parked();
    cx.test_window(window.into())
        .simulate_input(gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("right").unwrap(),
            is_held: false,
            prefer_character_input: false,
        }));
    cx.run_until_parked();
    window
        .update(cx, |view, window, cx| {
            assert_eq!(view.selected.as_ref(), "b");
            assert_eq!(view.outer, 0);
            view.outside.focus(window, cx);
        })
        .unwrap();
    cx.run_until_parked();
    cx.test_window(window.into())
        .simulate_input(gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("right").unwrap(),
            is_held: false,
            prefer_character_input: false,
        }));
    cx.run_until_parked();
    window
        .update(cx, |view, _, _| assert_eq!(view.outer, 1))
        .unwrap();
}
