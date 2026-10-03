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

use super::panel_state::{PanelState, PopoverState};
use gpui::{
    AnyElement, App, Entity, IntoElement, RenderOnce, Role, SharedString, Window, anchored,
    deferred, div, point, prelude::*, px,
};

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
/// Nested panels share a window-local dismissal stack. One Escape or outside
/// click closes only the top panel. Closing an ancestor closes its descendants.
#[derive(IntoElement)]
pub struct Popover {
    state: Entity<PopoverState>,
    trigger: AnyElement,
    label: SharedString,
    content: AnyElement,
    role: Role,
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
            role: Role::Dialog,
        }
    }
}

impl Popover {
    pub(crate) fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

impl Popover {
    pub(super) fn render_root(
        self,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let state = self.state.read(cx);
        let open = state.open;
        let panel_focus = state.panel_focus.clone();
        let scroll = state.scroll.clone();
        let (depth, top) =
            PanelState::depth(self.state.entity_id(), window, cx).unwrap_or_default();
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
            let action_state = self.state.clone();
            let escape_state = self.state;
            let viewport = window.viewport_size();
            let panel = div()
                .autoscroll_on_focus()
                .id("panel")
                .track_scroll(&scroll)
                .role(self.role)
                .key_context("CupertinoPanel")
                .aria_label(self.label)
                .track_focus(&panel_focus)
                .max_w((viewport.width - px(16.)).max(px(0.)))
                .max_h((viewport.height - px(16.)).max(px(0.)))
                .overflow_scroll()
                .occlude()
                .on_mouse_down_out(move |event, window, cx| {
                    // The trigger handles its own toggle on mouse-up.
                    if top
                        && !outside_state
                            .read(cx)
                            .trigger_bounds
                            .contains(&event.position)
                    {
                        outside_state.update(cx, |state, cx| state.close(window, cx));
                    }
                })
                .on_action(move |_: &super::modal::Dismiss, window, cx| {
                    if top {
                        action_state.update(cx, |state, cx| state.close(window, cx));
                    }
                })
                .on_key_down(move |event, window, cx| {
                    if top && event.keystroke.key == "escape" {
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
                    .with_priority(depth),
                ),
            );
        }

        root
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.render_root(window, cx)
    }
}

#[cfg(test)]
#[path = "popover/tests.rs"]
mod tests;
