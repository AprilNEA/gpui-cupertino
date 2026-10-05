use crate::theme::Theme;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, HoverListenerMode, IntoElement, KeyBinding,
    KeyContext, RenderOnce, Role, SharedString, Subscription, Task, Window, actions, anchored,
    deferred, div, point, prelude::*, px,
};
use std::time::Duration;

actions!(cupertino_tooltip, [Dismiss]);
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("CupertinoTooltip"))]);
}

/// The lifetime of one noninteractive tooltip.
pub struct TooltipState {
    description: SharedString,
    hovered: bool,
    focused: bool,
    visible: bool,
    dismissed: bool,
    pending: Option<Task<()>>,
    _subscriptions: [Subscription; 2],
}

impl TooltipState {
    /// Observe a trigger's focus and retain its accessible tooltip text.
    pub fn new(
        description: impl Into<SharedString>,
        focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focused = focus.contains_focused(window, cx);
        // Focus listeners run during drawing, so invalidate the hint after the frame completes.
        let on_focus = cx.on_focus_in(focus, window, |state, _, cx| {
            state.focused = true;
            state.update_visibility(cx);
            let entity_id = cx.entity_id();
            cx.defer(move |cx| cx.notify(entity_id));
        });
        let on_blur = cx.on_focus_out(focus, window, |state, _, _, cx| {
            state.focused = false;
            state.update_visibility(cx);
            let entity_id = cx.entity_id();
            cx.defer(move |cx| cx.notify(entity_id));
        });
        Self {
            description: description.into(),
            hovered: false,
            focused,
            visible: focused,
            dismissed: false,
            pending: None,
            _subscriptions: [on_focus, on_blur],
        }
    }

    /// Return the description for the trigger's accessibility properties.
    pub fn description(&self) -> &SharedString {
        &self.description
    }

    /// Return whether the state requests a visible hint.
    /// Inert or unmounted triggers do not display the hint.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn update_visibility(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        if !self.hovered && !self.focused {
            self.dismissed = false;
            self.visible = false;
        } else if !self.dismissed {
            if self.focused {
                self.visible = true;
            } else if !self.visible {
                let delay = cx.background_executor().timer(Duration::from_millis(500));
                self.pending = Some(cx.spawn(async move |state, cx| {
                    delay.await;
                    let Some(state) = state.upgrade() else {
                        return;
                    };
                    state.update(cx, |state, cx| {
                        state.visible = state.hovered && !state.dismissed;
                        cx.notify();
                    });
                }));
            }
        }
        cx.notify();
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.dismissed = true;
        self.visible = false;
        cx.notify();
    }
}

/// A hover- and keyboard-focus tooltip that never takes focus.
///
/// Hover waits 500 ms. Focus displays the hint immediately. Escape or activation
/// hides the hint until hover and focus have both left the trigger. The wrapper
/// exposes the description to accessibility even while the visual hint is hidden.
#[derive(IntoElement)]
pub struct Tooltip {
    state: Entity<TooltipState>,
    trigger: AnyElement,
}

impl Tooltip {
    /// Wrap the trigger observed by the retained tooltip state.
    pub fn new(state: &Entity<TooltipState>, trigger: impl IntoElement) -> Self {
        Self {
            state: state.clone(),
            trigger: trigger.into_any_element(),
        }
    }
}

impl RenderOnce for Tooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let visible = state.visible && !window.is_inert();
        let description = state.description.clone();
        let accessible_description = description.clone();
        let hovered_state = self.state.clone();
        let key_state = self.state.clone();
        let mouse_state = self.state.clone();
        let action_state = self.state.clone();
        if visible {
            let window_id = window.window_handle().window_id();
            let hint = self.state.clone();
            window.use_keyed_state(("tooltip-dismiss", hint.entity_id()), cx, move |_, cx| {
                let scope = cx.weak_entity();
                cx.intercept_keystrokes(move |event, window, cx| {
                    // Entity values can outlive their last handle until the next effect flush.
                    if scope.upgrade().is_none()
                        || window.window_handle().window_id() != window_id
                        || event.keystroke.key != "escape"
                        || event.keystroke.modifiers.number_of_modifiers() != 0
                        || window.has_pending_keystrokes()
                        || !hint.read(cx).visible
                    {
                        return;
                    }
                    // Match the transient hint at the deepest context; explicit user bindings still win.
                    let mut contexts = event.context_stack.clone();
                    let mut context = KeyContext::default();
                    context.add("CupertinoTooltip");
                    contexts.push(context);
                    let (bindings, pending) = cx
                        .key_bindings()
                        .borrow()
                        .bindings_for_input(std::slice::from_ref(&event.keystroke), &contexts);
                    if !pending
                        && bindings
                            .first()
                            .is_some_and(|binding| binding.action().as_any().is::<Dismiss>())
                    {
                        hint.update(cx, |state, cx| state.dismiss(cx));
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                })
            });
        }
        let mut root = div()
            .id(("tooltip-trigger", self.state.entity_id()))
            .relative()
            .when(visible, |root| root.key_context("CupertinoTooltip"))
            .on_action(move |_: &Dismiss, _, cx| {
                action_state.update(cx, |state, cx| state.dismiss(cx))
            })
            .role(Role::Group)
            .aria_description(description.clone())
            .a11y_synthetic_children(move |builder| {
                builder
                    .parent_node()
                    .set_tooltip(accessible_description.as_ref())
            })
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(move |hovered, _, cx| {
                hovered_state.update(cx, |state, cx| {
                    state.hovered = *hovered;
                    state.update_visibility(cx);
                })
            })
            .capture_key_down(move |event, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    key_state.update(cx, |state, cx| state.dismiss(cx));
                }
            })
            .on_any_mouse_down(move |_, _, cx| {
                mouse_state.update(cx, |state, cx| state.dismiss(cx))
            })
            .child(self.trigger);
        if visible {
            let theme = Theme::for_window(window);
            let hint = div()
                .id("hint")
                .role(Role::Tooltip)
                .aria_label(description.clone())
                .px(px(8.))
                .py(px(5.))
                .rounded(px(5.))
                .border_1()
                .border_color(theme.control_border)
                .bg(theme.control_background)
                .text_color(theme.foreground)
                .text_size(px(12.))
                .max_w((window.viewport_size().width - px(16.)).max(px(0.)))
                .child(description);
            #[cfg(test)]
            let hint = hint.debug_selector(|| "tooltip-hint".into());
            root = root.child(
                div().absolute().top_full().left_0().child(
                    deferred(
                        anchored()
                            .offset(point(px(0.), px(6.)))
                            .snap_to_window_with_margin(px(8.))
                            .child(hint),
                    )
                    .with_priority(usize::MAX),
                ),
            );
        }
        root
    }
}

#[cfg(test)]
mod tests;
