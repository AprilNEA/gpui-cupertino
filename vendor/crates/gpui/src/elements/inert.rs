// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

/// Paint a subtree while excluding its input, focus, and accessibility registrations.
///
/// Use this boundary for content behind a modal panel. Descendant deferred elements
/// remain inert. Application-global actions and persistent application subscriptions
/// remain the application's responsibility.
pub fn inert(child: impl IntoElement, disabled: bool) -> Inert {
    Inert {
        child: child.into_any_element(),
        disabled,
    }
}

/// A paint-preserving input boundary created by [`inert`].
pub struct Inert {
    child: AnyElement,
    disabled: bool,
}

impl IntoElement for Inert {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Inert {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (
            window.with_inert(self.disabled, |window| {
                self.child.request_layout(window, cx)
            }),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_inert(self.disabled, |window| self.child.prepaint(window, cx));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_inert(self.disabled, |window| self.child.paint(window, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Context, Entity, FocusHandle, KeyUpEvent, Keystroke, Modifiers, Render, Role,
        StyleRefinement, TestAppContext, deferred, div, prelude::*, px, rgb,
    };
    use std::sync::{Arc, atomic::AtomicBool};

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
                        .on_a11y_action(crate::accesskit::Action::Click, |_, _, _| {})
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
    fn inert_preserves_deferred_paint_and_excludes_input_focus_and_accessibility(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = cx.add_window_view(|window, cx| {
            window.a11y =
                crate::window::a11y::A11y::new(Arc::new(AtomicBool::new(true)), false, None);
            let focus = cx.focus_handle().tab_stop(true);
            focus.focus(window, cx);
            InertView {
                disabled: false,
                focus,
                clicks: 0,
                keys: 0,
            }
        });
        cx.update(|window, _| {
            assert_eq!(window.a11y.node_bounds.len(), 1);
            assert_eq!(window.a11y.focus_ids.len(), 1);
            assert_eq!(window.a11y.action_listeners.len(), 1);
        });
        let bounds = cx.debug_bounds("deferred-button").unwrap();
        view.update(cx, |view, cx| {
            view.disabled = true;
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        });
        cx.simulate_click(bounds.center(), Modifiers::none());
        assert_eq!(
            view.read_with(cx, |view, _| (view.clicks, view.keys)),
            (0, 0)
        );
        assert_eq!(cx.debug_bounds("deferred-button"), Some(bounds));
        cx.update(|window, cx| {
            assert!(window.a11y.node_bounds.is_empty());
            assert!(window.a11y.focus_ids.is_empty());
            assert!(window.a11y.action_listeners.is_empty());
            assert!(!window.painted_quads().is_empty());
            window.blur(cx);
            window.focus_next(cx);
            assert!(window.focused(cx).is_none());
        });
        view.update(cx, |view, cx| {
            view.disabled = false;
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_click(bounds.center(), Modifiers::none());
        assert_eq!(view.read_with(cx, |view, _| view.clicks), 1);
        cx.update(|window, _| assert_eq!(window.a11y.node_bounds.len(), 1));
    }

    struct CachedHost {
        disabled: bool,
        child: Entity<InertView>,
    }
    impl Render for CachedHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            inert(
                self.child
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
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
}
