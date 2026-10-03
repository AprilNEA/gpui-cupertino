use gpui::{
    App, Bounds, Context, EntityId, FocusHandle, Global, Pixels, ScrollHandle, WeakEntity, Window,
    WindowId,
};

#[derive(Default)]
struct Panels(Vec<(WindowId, WeakEntity<PanelState>)>);
impl Global for Panels {}

/// Retained open state and focus handles shared by popovers, dialogs, and sheets.
///
/// Retain one entity per panel. Panels form an opening-order stack within each window.
/// Closing a panel closes all panels opened above that panel.
pub struct PanelState {
    pub(crate) open: bool,
    pub(crate) trigger_focus: FocusHandle,
    pub(crate) panel_focus: FocusHandle,
    pub(crate) scroll: ScrollHandle,
    initial_focus: Option<FocusHandle>,
    pub(crate) trigger_bounds: Bounds<Pixels>,
}

/// The retained state for a popover.
pub type PopoverState = PanelState;

impl PanelState {
    /// Create a closed panel with stable trigger and panel focus handles.
    pub fn new(cx: &App) -> Self {
        Self {
            open: false,
            trigger_focus: cx.focus_handle().tab_stop(true),
            panel_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            initial_focus: None,
            trigger_bounds: Bounds::default(),
        }
    }

    /// Focus this descendant control when the panel opens.
    #[must_use]
    pub fn initial_focus(mut self, focus: &FocusHandle) -> Self {
        self.initial_focus = Some(focus.clone());
        self
    }

    /// Return the focus handle that the opening control must track.
    pub fn trigger_focus_handle(&self) -> &FocusHandle {
        &self.trigger_focus
    }

    /// Return whether the panel is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the panel above existing panels and focus its configured target.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        let entity = cx.entity().downgrade();
        let panels = &mut cx.default_global::<Panels>().0;
        panels.retain(|(_, panel)| panel.upgrade().is_some());
        panels.push((window.window_handle().window_id(), entity));
        self.initial_focus
            .as_ref()
            .unwrap_or(&self.panel_focus)
            .focus(window, cx);
        cx.notify();
        window.refresh();
    }

    /// Close this panel and its descendants, restoring focus when the stack owns focus.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        let id = cx.entity_id();
        let window_id = window.window_handle().window_id();
        let panels = &mut cx.default_global::<Panels>().0;
        let mut descendants = Vec::new();
        let mut found = false;
        panels.retain(|(entry_window, panel)| {
            if *entry_window != window_id {
                return true;
            }
            if panel.entity_id() == id {
                found = true;
                return false;
            }
            if found {
                descendants.push(panel.clone());
                false
            } else {
                true
            }
        });
        let mut restore = self.owns_focus(window, cx);
        for descendant in descendants
            .into_iter()
            .rev()
            .filter_map(|panel| panel.upgrade())
        {
            descendant.update(cx, |state, cx| {
                restore |= state.owns_focus(window, cx);
                state.open = false;
                cx.notify();
            });
        }
        self.open = false;
        if restore {
            self.trigger_focus.focus(window, cx);
        }
        cx.notify();
        window.refresh();
    }

    /// Toggle the panel through the same focus transitions as open and close.
    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(window, cx)
        } else {
            self.open(window, cx)
        }
    }

    pub(crate) fn owns_focus(&self, window: &Window, cx: &App) -> bool {
        self.panel_focus.contains_focused(window, cx)
            || self
                .initial_focus
                .as_ref()
                .is_some_and(|focus| focus.is_focused(window))
    }

    pub(crate) fn depth(id: EntityId, window: &Window, cx: &App) -> Option<(usize, bool)> {
        let panels = cx.try_global::<Panels>()?;
        let mut entries = panels
            .0
            .iter()
            .filter(|(entry_window, panel)| {
                *entry_window == window.window_handle().window_id() && panel.upgrade().is_some()
            })
            .peekable();
        let mut depth = 0;
        while let Some((_, panel)) = entries.next() {
            depth += 1;
            if panel.entity_id() == id {
                return Some((depth, entries.peek().is_none()));
            }
        }
        None
    }
}
