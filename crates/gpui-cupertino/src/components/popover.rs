//! Nonmodal popovers with caller-owned state and content.
//!
//! ```no_run
//! use gpui::{Context, Entity, IntoElement, Render, Role, Window, div, prelude::*, px};
//! use gpui_cupertino::components::{Popover, PopoverState};
//!
//! struct Details {
//!     popover: Entity<PopoverState>,
//! }
//!
//! impl Details {
//!     fn new(cx: &mut Context<Self>) -> Self {
//!         Self { popover: cx.new(|cx| PopoverState::new(cx)) }
//!     }
//! }
//!
//! impl Render for Details {
//!     fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         let state = self.popover.read(cx);
//!         let trigger = div()
//!             .id("show-details")
//!             .role(Role::Button)
//!             .aria_label("Show details")
//!             .aria_expanded(state.is_open())
//!             .track_focus(state.trigger_focus_handle())
//!             .child("Show details")
//!             .on_click(cx.listener(|this, _, window, cx| {
//!                 this.popover.update(cx, |state, cx| state.toggle(window, cx));
//!             }));
//!         Popover::new(&self.popover, trigger, "Details", div().w(px(240.)).child("Details"))
//!     }
//! }
//! ```

use gpui::{
    AnyElement, App, Bounds, Context, Entity, FocusHandle, IntoElement, Pixels, RenderOnce, Role,
    SharedString, Window, anchored, deferred, div, point, prelude::*, px,
};

/// The open state and focus handles for one [`Popover`].
///
/// Retain this state in an [`Entity`] and render exactly one popover for that entity.
/// Give the trigger [`Self::trigger_focus_handle`] and its expanded state.
pub struct PopoverState {
    open: bool,
    trigger_focus: FocusHandle,
    panel_focus: FocusHandle,
    initial_focus: Option<FocusHandle>,
    trigger_bounds: Bounds<Pixels>,
}

impl PopoverState {
    /// Create a closed popover with stable focus handles.
    pub fn new(cx: &App) -> Self {
        Self {
            open: false,
            trigger_focus: cx.focus_handle().tab_stop(true),
            panel_focus: cx.focus_handle(),
            initial_focus: None,
            trigger_bounds: Bounds::default(),
        }
    }

    /// Focus this content control when the popover opens.
    ///
    /// The control must be a descendant of the popover content. Without an explicit
    /// target, the popover panel receives focus. Opening again does not reset focus.
    #[must_use]
    pub fn initial_focus(mut self, focus: &FocusHandle) -> Self {
        self.initial_focus = Some(focus.clone());
        self
    }

    /// Return the handle that the trigger control must track.
    pub fn trigger_focus_handle(&self) -> &FocusHandle {
        &self.trigger_focus
    }

    /// Return whether the popover is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the popover and focus its configured target.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.initial_focus
            .as_ref()
            .unwrap_or(&self.panel_focus)
            .focus(window, cx);
        cx.notify();
    }

    /// Close the popover and restore trigger focus if the popover still owns focus.
    ///
    /// Focus that has moved to another control remains on that control.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        if self.panel_focus.contains_focused(window, cx)
            || self
                .initial_focus
                .as_ref()
                .is_some_and(|focus| focus.is_focused(window))
        {
            self.trigger_focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Toggle the popover through the same focus transitions as open and close.
    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(window, cx);
        } else {
            self.open(window, cx);
        }
    }
}

/// A nonmodal panel anchored below an existing trigger control.
///
/// The trigger owns activation. Connect its click handler to [`PopoverState::toggle`],
/// track [`PopoverState::trigger_focus_handle`], and expose [`PopoverState::is_open`]
/// through the trigger's expanded accessibility state. The panel has a dialog role
/// and the supplied accessible label. An unconsumed Escape event from the panel
/// closes the panel. Mouse-down outside the panel and trigger also closes the
/// panel. Outside clicks continue to their original destination.
///
/// The panel fits inside the window and scrolls content that exceeds the viewport.
/// The caller styles the content; a `Glass` element may supply
/// its surface on macOS. Deferred painting preserves the trigger's layout and
/// paints the panel after normal window content. Tab navigation remains with the
/// host application; this nonmodal panel does not trap focus.
/// Nested popovers are not supported.
#[derive(IntoElement)]
pub struct Popover {
    state: Entity<PopoverState>,
    trigger: AnyElement,
    label: SharedString,
    content: AnyElement,
}

impl Popover {
    /// Build a popover with a trigger, an accessible panel label, and styled content.
    ///
    /// The retained state supplies stable element IDs across renders.
    pub fn new(
        state: &Entity<PopoverState>,
        trigger: impl IntoElement,
        label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            state: state.clone(),
            trigger: trigger.into_any_element(),
            label: label.into(),
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let open = state.open;
        let panel_focus = state.panel_focus.clone();
        let bounds_state = self.state.clone();
        let mut root = div()
            .on_children_prepainted(move |bounds, _, cx| {
                bounds_state.update(cx, |state, _| state.trigger_bounds = bounds[0]);
            })
            .id(("popover", self.state.entity_id()))
            .relative()
            .child(self.trigger);

        if open {
            let outside_state = self.state.clone();
            let escape_state = self.state;
            let viewport = window.viewport_size();
            let panel = div()
                .id("panel")
                .role(Role::Dialog)
                .aria_label(self.label)
                .track_focus(&panel_focus)
                .max_w((viewport.width - px(16.)).max(px(0.)))
                .max_h((viewport.height - px(16.)).max(px(0.)))
                .overflow_scroll()
                .occlude()
                .on_mouse_down_out(move |event, window, cx| {
                    // ponytail: outside dismissal covers one panel; nested popovers need shared bounds.
                    // The trigger handles its own toggle on mouse-up.
                    if !outside_state
                        .read(cx)
                        .trigger_bounds
                        .contains(&event.position)
                    {
                        outside_state.update(cx, |state, cx| state.close(window, cx));
                    }
                })
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        escape_state.update(cx, |state, cx| state.close(window, cx));
                        cx.stop_propagation();
                    }
                })
                .child(self.content);

            #[cfg(test)]
            let panel = panel.debug_selector(|| "popover-panel".into());

            root = root.child(
                div().absolute().top_full().left_0().child(
                    deferred(
                        anchored()
                            .offset(point(px(0.), px(8.)))
                            .snap_to_window_with_margin(px(8.))
                            .child(panel),
                    )
                    .with_priority(1),
                ),
            );
        }

        root
    }
}

#[cfg(test)]
#[path = "popover/tests.rs"]
mod tests;
