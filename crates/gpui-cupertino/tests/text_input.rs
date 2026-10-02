//! Verify editing through keyboard dispatch, the platform IME bridge, and accessibility actions.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AppContext, Bounds, Context, Entity, EntityInputHandler, Modifiers, MouseButton,
    MouseDownEvent, PlatformWindow, Render, TestAppContext, VisualContext, VisualTestContext,
    Window, div, point, prelude::*, px,
};
use gpui_cupertino::components::{TextInput, TextInputEvent};

struct Harness {
    input: Entity<TextInput>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p(px(37.0))
            .child(div().w(px(144.0)).child(self.input.clone()))
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<TextInput>, &mut VisualTestContext) {
    cx.update(gpui_cupertino::init);
    let (host, cx) = cx.add_window_view(|window, cx| Harness {
        input: cx.new(|cx| TextInput::new("Name", window, cx).placeholder("Enter a name")),
    });
    let input = host.read_with(cx, |host, _| host.input.clone());
    cx.focus(&input);
    cx.run_until_parked();
    (input, cx)
}

fn shortcut(cx: &mut VisualTestContext, key: &str) {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&format!("{modifier}-{key}"));
}

fn value(input: &Entity<TextInput>, cx: &VisualTestContext) -> String {
    input.read_with(cx, |input, _| input.value().to_owned())
}

#[gpui::test]
fn keyboard_edits_graphemes_clipboard_history_and_events(cx: &mut TestAppContext) {
    let (input, cx) = open(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.update(|_, cx| {
        let events = events.clone();
        cx.subscribe(&input, move |_, event: &TextInputEvent, _| {
            events.borrow_mut().push(event.clone());
        })
    });
    input.update(cx, |input, cx| input.set_value("a👩🏽‍💻e\u{301}", cx));
    assert!(events.borrow().is_empty());
    cx.run_until_parked();
    cx.simulate_keystrokes("end backspace backspace");
    assert_eq!(value(&input, cx), "a");
    shortcut(cx, "z");
    assert_eq!(value(&input, cx), "a👩🏽‍💻");
    shortcut(cx, "z");
    assert_eq!(value(&input, cx), "a👩🏽‍💻e\u{301}");
    shortcut(cx, "shift-z");
    assert_eq!(value(&input, cx), "a👩🏽‍💻");
    cx.simulate_keystrokes("home shift-right");
    shortcut(cx, "x");
    assert_eq!(value(&input, cx), "👩🏽‍💻");
    shortcut(cx, "v");
    assert_eq!(value(&input, cx), "a👩🏽‍💻");
    shortcut(cx, "a");
    cx.simulate_input("你好");
    assert_eq!(value(&input, cx), "你好");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        events.borrow().last(),
        Some(&TextInputEvent::Submitted("你好".into()))
    );
    assert!(
        events
            .borrow()
            .contains(&TextInputEvent::Changed("a".into()))
    );
}

#[gpui::test]
fn platform_composition_uses_relative_utf16_selection_and_one_undo(cx: &mut TestAppContext) {
    let (input, cx) = open(cx);
    input.update(cx, |input, cx| input.set_value("🙂尾", cx));
    cx.run_until_parked();
    let handle = cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        window.window_handle()
    });
    let mut platform = cx.test_window(handle);
    let mut handler = platform
        .take_input_handler()
        .expect("focused input must register a platform handler");
    handler.replace_and_mark_text_in_range(Some(2..2), "ni", Some(1..2));
    assert_eq!(handler.selected_text_range(false).unwrap().range, 3..4);
    handler.replace_and_mark_text_in_range(None, "你😀", Some(1..3));
    assert_eq!(handler.selected_text_range(false).unwrap().range, 3..5);
    assert_eq!(handler.marked_text_range(), Some(2..5));
    handler.replace_text_in_range(None, "你好");
    assert_eq!(value(&input, cx), "🙂你好尾");
    assert_eq!(handler.selected_text_range(false).unwrap().range, 4..4);
    assert_eq!(handler.marked_text_range(), None);
    let mut adjusted = None;
    assert_eq!(
        handler.text_for_range(1..2, &mut adjusted).as_deref(),
        Some("🙂")
    );
    assert_eq!(adjusted, Some(0..2));
    platform.set_input_handler(handler);
    cx.run_until_parked();
    shortcut(cx, "z");
    assert_eq!(value(&input, cx), "🙂尾");
}

#[gpui::test]
fn caret_geometry_accounts_for_parent_offset_and_horizontal_scroll(cx: &mut TestAppContext) {
    let (input, cx) = open(cx);
    let text = "abcdefghij🙂".repeat(8);
    input.update(cx, |input, cx| input.set_value(&text, cx));
    cx.run_until_parked();
    let end = text.encode_utf16().count();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        input.update(cx, |input, cx| {
            let caret = input
                .bounds_for_range(end..end, Bounds::default(), window, cx)
                .unwrap();
            assert!(caret.left() > px(37.0 + 100.0));
            assert!(caret.right() < px(37.0 + 144.0));
            assert!(caret.top() >= px(37.0));
            assert_eq!(
                input.character_index_for_point(point(caret.left(), caret.center().y), window, cx),
                Some(end)
            );
            assert_eq!(
                input.character_index_for_point(point(px(0.0), px(0.0)), window, cx),
                None
            );
        });
    });
    cx.simulate_keystrokes("home");
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        input.update(cx, |input, cx| {
            let caret = input
                .bounds_for_range(0..0, Bounds::default(), window, cx)
                .unwrap();
            assert!(caret.left() < px(37.0 + 20.0));
            assert_eq!(
                input.character_index_for_point(point(caret.left(), caret.center().y), window, cx),
                Some(0)
            );
        });
    });
}

#[gpui::test]
fn ime_geometry_uses_current_unicode_text_before_the_next_frame(cx: &mut TestAppContext) {
    let (input, cx) = open(cx);
    input.update(cx, |input, cx| input.set_value("abcdefghij", cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let before_paint = input.update(cx, |input, cx| {
            input.set_value("🙂中", cx);
            let start = input
                .bounds_for_range(0..0, Bounds::default(), window, cx)
                .unwrap();
            let caret = input
                .bounds_for_range(2..2, Bounds::default(), window, cx)
                .unwrap();
            let indices = (0..30)
                .map(|step| {
                    input
                        .character_index_for_point(
                            point(start.left() + px(step as f32 * 2.0), start.center().y),
                            window,
                            cx,
                        )
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert!(indices.iter().all(|index| [0, 2, 3].contains(index)));
            (start, caret, indices)
        });
        window.draw(cx).clear(cx);
        input.update(cx, |input, cx| {
            assert_eq!(
                input.bounds_for_range(2..2, Bounds::default(), window, cx),
                Some(before_paint.1)
            );
            for (step, index) in before_paint.2.into_iter().enumerate() {
                assert_eq!(
                    input.character_index_for_point(
                        point(
                            before_paint.0.left() + px(step as f32 * 2.0),
                            before_paint.0.center().y,
                        ),
                        window,
                        cx,
                    ),
                    Some(index)
                );
            }
        });
    });
}

#[gpui::test]
fn pointer_selection_continues_outside_the_field_and_stops_on_release(cx: &mut TestAppContext) {
    let (input, cx) = open(cx);
    input.update(cx, |input, cx| input.set_value("hello", cx));
    cx.run_until_parked();
    let start = cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        input.update(cx, |input, cx| {
            input
                .bounds_for_range(2..2, Bounds::default(), window, cx)
                .unwrap()
                .center()
        })
    });
    let outside = point(px(300.0), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            let selection = input.selected_text_range(true, window, cx).unwrap();
            assert_eq!(selection.range, 2..5);
            assert!(!selection.reversed);
        });
    });
    cx.simulate_mouse_move(
        point(px(10.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            let selection = input.selected_text_range(true, window, cx).unwrap();
            assert_eq!(selection.range, 0..2);
            assert!(selection.reversed);
        });
    });
    cx.simulate_mouse_up(
        point(px(10.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    // A later drag that starts outside the field must not reuse the previous selection gesture.
    cx.simulate_mouse_down(outside, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            assert_eq!(
                input.selected_text_range(true, window, cx).unwrap().range,
                0..2
            );
        });
    });
}

#[gpui::test]
fn double_click_selects_the_hit_word_at_its_trailing_edge(cx: &mut TestAppContext) {
    let (input, cx) = open(cx);
    input.update(cx, |input, cx| input.set_value("hello world", cx));
    cx.run_until_parked();
    for (character, expected) in [(4..5, 0..5), (10..11, 6..11)] {
        let position = cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            input.update(cx, |input, cx| {
                let bounds = input
                    .bounds_for_range(character, Bounds::default(), window, cx)
                    .unwrap();
                point(bounds.right() - bounds.size.width / 4.0, bounds.center().y)
            })
        });
        cx.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                assert_eq!(
                    input.selected_text_range(true, window, cx).unwrap().range,
                    expected
                );
            });
        });
    }
}

#[gpui::test]
fn readonly_and_disabled_inputs_reject_edits_and_expose_accessible_state(cx: &mut TestAppContext) {
    use gpui::{
        AccessibleAction, Role,
        accesskit::{ActionData, ActionRequest, TreeId},
    };

    let (input, cx) = open(cx);
    let handle = cx.update(|window, _| window.window_handle());
    let platform = cx.test_window(handle);
    platform.activate_accessibility();
    cx.run_until_parked();
    let tree = platform.accessibility_tree_update().unwrap();
    let (node_id, node) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::TextInput)
        .unwrap();
    let node_id = *node_id;
    assert_eq!(node.label(), Some("Name"));
    assert_eq!(node.placeholder(), Some("Enter a name"));
    assert!(node.supports_action(AccessibleAction::SetValue));
    platform.simulate_accessibility_action(ActionRequest {
        action: AccessibleAction::SetValue,
        target_tree: TreeId::ROOT,
        target_node: node_id,
        data: Some(ActionData::Value("a\n中".into())),
    });
    cx.run_until_parked();
    assert_eq!(value(&input, cx), "a 中");
    input.update(cx, |input, cx| input.set_read_only(true, cx));
    cx.run_until_parked();
    shortcut(cx, "a");
    shortcut(cx, "x");
    cx.simulate_input("x");
    assert_eq!(value(&input, cx), "a 中");
    shortcut(cx, "c");
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("a 中")
        )
    });
    let tree = platform.accessibility_tree_update().unwrap();
    let node = &tree.nodes.iter().find(|(id, _)| *id == node_id).unwrap().1;
    assert!(node.is_read_only());
    assert!(!node.supports_action(AccessibleAction::SetValue));
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_disabled(true, window, cx);
            input.replace_text_in_range(None, "ignored", window, cx);
        })
    });
    cx.run_until_parked();
    assert_eq!(value(&input, cx), "a 中");
    let tree = platform.accessibility_tree_update().unwrap();
    let node = &tree.nodes.iter().find(|(id, _)| *id == node_id).unwrap().1;
    assert!(node.is_disabled());
    assert!(!node.supports_action(AccessibleAction::Focus));
    cx.update(|window, cx| assert!(window.focused(cx).is_none()));
}
