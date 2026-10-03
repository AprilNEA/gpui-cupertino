use super::PanelState;
use crate::theme::Theme;
use gpui::{
    AnyElement, App, Entity, IntoElement, KeyBinding, RenderOnce, Role, SharedString, Window,
    actions, anchored, deferred, div, inert, point, prelude::*, px, rgba,
};

actions!(cupertino_panel, [Next, Previous, Dismiss]);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", Next, Some("CupertinoModal")),
        KeyBinding::new("shift-tab", Previous, Some("CupertinoModal")),
        KeyBinding::new("escape", Dismiss, Some("CupertinoPanel")),
    ]);
}

/// A centered modal panel presented by [`ModalHost`].
pub struct Dialog {
    state: Entity<PanelState>,
    label: SharedString,
    content: AnyElement,
    outside_dismiss: bool,
}

impl Dialog {
    /// Create a dialog with a retained state, an accessible name, and caller-owned content.
    pub fn new(
        state: &Entity<PanelState>,
        label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            state: state.clone(),
            label: label.into(),
            content: content.into_any_element(),
            outside_dismiss: false,
        }
    }

    /// Allow a click on the backdrop to dismiss the dialog.
    ///
    /// The backdrop always consumes the click. Escape and programmatic closing remain available.
    pub fn dismiss_on_outside_click(mut self, dismiss: bool) -> Self {
        self.outside_dismiss = dismiss;
        self
    }
}

/// The viewport edge where a sheet enters the layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetEdge {
    /// Attach the sheet to the top edge.
    #[default]
    Top,
    /// Attach the sheet to the trailing edge.
    Trailing,
    /// Attach the sheet to the bottom edge.
    Bottom,
}

/// An edge-attached modal panel presented by [`ModalHost`].
pub struct Sheet {
    dialog: Dialog,
    edge: SheetEdge,
}

impl Sheet {
    /// Create a top-attached sheet with caller-owned state and content.
    pub fn new(
        state: &Entity<PanelState>,
        label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            dialog: Dialog::new(state, label, content),
            edge: SheetEdge::Top,
        }
    }

    /// Attach the sheet to a different viewport edge.
    pub fn edge(mut self, edge: SheetEdge) -> Self {
        self.edge = edge;
        self
    }

    /// Allow the backdrop to dismiss the sheet while consuming the click.
    pub fn dismiss_on_outside_click(mut self, dismiss: bool) -> Self {
        self.dialog.outside_dismiss = dismiss;
        self
    }
}

/// A full-window modal boundary that keeps the background painted but inert.
///
/// Place background handlers inside `background`. Application-global actions and
/// persistent subscriptions remain application-owned. Call [`crate::init`] at startup.
/// Tab cycles through the active modal subtree. Nested hosts support nested modals.
#[derive(IntoElement)]
pub struct ModalHost {
    background: AnyElement,
    panel: Option<(Dialog, Option<SheetEdge>)>,
}

impl ModalHost {
    /// Wrap the full application content for this window.
    pub fn new(background: impl IntoElement) -> Self {
        Self {
            background: background.into_any_element(),
            panel: None,
        }
    }

    /// Present a dialog when its retained state is open.
    pub fn dialog(mut self, dialog: Dialog) -> Self {
        self.panel = Some((dialog, None));
        self
    }

    /// Present a sheet when its retained state is open.
    pub fn sheet(mut self, sheet: Sheet) -> Self {
        self.panel = Some((sheet.dialog, Some(sheet.edge)));
        self
    }
}

impl RenderOnce for ModalHost {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let open = self
            .panel
            .as_ref()
            .is_some_and(|(panel, _)| panel.state.read(cx).is_open());
        let mut host = div().size_full().child(inert(self.background, open));
        if let Some((panel, edge)) = self.panel.filter(|_| open) {
            let state = panel.state.read(cx);
            let focus = state.panel_focus.clone();
            let scroll = state.scroll.clone();
            let (depth, top) =
                PanelState::depth(panel.state.entity_id(), window, cx).unwrap_or_default();
            if top && !state.owns_focus(window, cx) {
                let focus_state = panel.state.downgrade();
                // A just-restored descendant can be absent from the previous inert dispatch tree.
                window.on_next_frame(move |window, cx| {
                    let Some(entity) = focus_state.upgrade() else {
                        return;
                    };
                    let state = entity.read(cx);
                    if state.is_open()
                        && PanelState::depth(entity.entity_id(), window, cx)
                            .is_some_and(|(_, top)| top)
                        && !state.owns_focus(window, cx)
                    {
                        state.panel_focus.clone().focus(window, cx);
                    }
                });
            }
            let escape_state = panel.state.clone();
            let raw_escape_state = panel.state.clone();
            let outside_state = panel.state;
            let theme = Theme::for_window(window);
            let viewport = window.viewport_size();
            let surface = div()
                .autoscroll_on_focus()
                .id(("modal", outside_state.entity_id()))
                .track_scroll(&scroll)
                .role(Role::Dialog)
                .aria_label(panel.label)
                .a11y_synthetic_children(|builder| builder.parent_node().set_modal())
                .key_context("CupertinoPanel CupertinoModal")
                .track_focus(&focus)
                .tab_group()
                .tab_stop(false)
                .bg(theme.control_background)
                .text_color(theme.foreground)
                .border_1()
                .border_color(theme.control_border)
                .rounded(px(12.))
                .p(px(20.))
                .max_w((viewport.width - px(16.)).max(px(0.)))
                .max_h((viewport.height - px(16.)).max(px(0.)))
                .overflow_scroll()
                .occlude()
                .on_action(|_: &Next, window, cx| window.focus_next(cx))
                .on_action(|_: &Previous, window, cx| window.focus_prev(cx))
                .on_action(move |_: &Dismiss, window, cx| {
                    if top {
                        escape_state.update(cx, |state, cx| state.close(window, cx));
                    }
                })
                .on_key_down(move |event, window, cx| {
                    if top && event.keystroke.key == "escape" {
                        raw_escape_state.update(cx, |state, cx| state.close(window, cx));
                    }
                    cx.stop_propagation();
                })
                .on_key_up(|_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(move |_, window, cx| {
                    if top && panel.outside_dismiss {
                        outside_state.update(cx, |state, cx| state.close(window, cx));
                    }
                })
                .child(panel.content);
            #[cfg(test)]
            let surface = surface.debug_selector(|| "modal-panel".into());
            let mut backdrop = div()
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .bg(rgba(0x00000055))
                .occlude()
                .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation());
            backdrop = match edge {
                None => backdrop.items_center().justify_center(),
                Some(SheetEdge::Top) => backdrop.items_start().justify_center(),
                Some(SheetEdge::Trailing) => backdrop.items_center().justify_end(),
                Some(SheetEdge::Bottom) => backdrop.items_end().justify_center(),
            };
            host = host.child(
                deferred(
                    anchored()
                        .position(point(px(0.), px(0.)))
                        .child(backdrop.child(surface)),
                )
                .with_priority(depth),
            );
        }
        host
    }
}
