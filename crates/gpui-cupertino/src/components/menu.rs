use super::{PanelState, Popover};
use crate::theme::Theme;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, IntoElement, KeyBinding, RenderOnce, Role,
    SharedString, Window, actions, div, prelude::*, px,
};

actions!(
    cupertino_menu,
    [
        NextItem,
        PreviousItem,
        FirstItem,
        LastItem,
        NextControl,
        PreviousControl
    ]
);

pub(crate) fn init(cx: &mut App) {
    let context = Some("CupertinoMenu");
    cx.bind_keys([
        KeyBinding::new("down", NextItem, context),
        KeyBinding::new("up", PreviousItem, context),
        KeyBinding::new("home", FirstItem, context),
        KeyBinding::new("end", LastItem, context),
        KeyBinding::new("tab", NextControl, context),
        KeyBinding::new("shift-tab", PreviousControl, context),
    ]);
}

/// An action in a menu. The ID is emitted when the action is selected.
#[derive(Clone, Debug)]
pub struct MenuItem {
    id: SharedString,
    label: SharedString,
    disabled: bool,
}

impl MenuItem {
    /// Create an enabled menu action with a stable ID and visible label.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Exclude the action from pointer, keyboard, and accessibility activation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// An enabled menu action was selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEvent {
    /// The selected item's stable ID.
    pub id: SharedString,
}

/// Retained menu items, roving focus handles, and popup state.
pub struct MenuState {
    panel: Entity<PanelState>,
    items: Vec<(MenuItem, FocusHandle)>,
}
impl EventEmitter<MenuEvent> for MenuState {}

impl MenuState {
    /// Create a menu with stable action IDs.
    ///
    /// # Panics
    /// Panics if two actions have the same ID.
    pub fn new(items: impl IntoIterator<Item = MenuItem>, cx: &mut App) -> Self {
        let items: Vec<_> = items
            .into_iter()
            .map(|item| (item, cx.focus_handle()))
            .collect();
        let mut ids = std::collections::HashSet::new();
        assert!(
            items.iter().all(|(item, _)| ids.insert(item.id.clone())),
            "menu item IDs must be unique"
        );
        let panel = cx.new(|cx| {
            let state = PanelState::new(cx);
            if let Some((_, focus)) = items.iter().find(|(item, _)| !item.disabled) {
                state.initial_focus(focus)
            } else {
                state
            }
        });
        // Each opening starts at the first enabled item; navigation never focuses a disabled item.
        Self { panel, items }
    }

    /// Return the focus handle that the menu trigger must track.
    pub fn trigger_focus_handle<'a>(&'a self, cx: &'a App) -> &'a FocusHandle {
        self.panel.read(cx).trigger_focus_handle()
    }

    /// Return whether the menu is open.
    pub fn is_open(&self, cx: &App) -> bool {
        self.panel.read(cx).is_open()
    }

    /// Open the menu and focus its first enabled action.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_open(cx) {
            return;
        }
        self.panel.update(cx, |state, cx| state.open(window, cx));
        if let Some((_, focus)) = self.items.iter().find(|(item, _)| !item.disabled) {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Close the menu and restore the trigger when the menu owns focus.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panel.update(cx, |state, cx| state.close(window, cx));
        cx.notify();
    }

    /// Toggle the menu through its focus transitions.
    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_open(cx) {
            self.close(window, cx);
        } else {
            self.open(window, cx);
        }
    }

    fn navigate(&self, key: &str, window: &mut Window, cx: &mut App) {
        let enabled: Vec<_> = self
            .items
            .iter()
            .filter(|(item, _)| !item.disabled)
            .collect();
        if enabled.is_empty() {
            return;
        }
        let selected = enabled
            .iter()
            .position(|(_, focus)| focus.is_focused(window));
        let index = match key {
            "home" => 0,
            "end" => enabled.len() - 1,
            "up" => selected.map_or(enabled.len() - 1, |i| {
                (i + enabled.len() - 1) % enabled.len()
            }),
            _ => selected.map_or(0, |i| (i + 1) % enabled.len()),
        };
        enabled[index].1.focus(window, cx);
    }

    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let item = &self.items[index].0;
        if item.disabled || !self.is_open(cx) {
            return;
        }
        let event = MenuEvent {
            id: item.id.clone(),
        };
        self.close(window, cx);
        cx.emit(event);
    }
}

/// An anchored action menu with disabled items and wrapping keyboard navigation.
///
/// Give the trigger the state's focus handle and expanded state. Call `toggle`
/// on activation and subscribe to [`MenuEvent`] on the retained state.
#[derive(IntoElement)]
pub struct Menu {
    state: Entity<MenuState>,
    trigger: gpui::AnyElement,
    label: SharedString,
}

impl Menu {
    /// Build a menu around a caller-owned trigger.
    pub fn new(
        state: &Entity<MenuState>,
        trigger: impl IntoElement,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            state: state.clone(),
            trigger: trigger.into_any_element(),
            label: label.into(),
        }
    }
}

impl RenderOnce for Menu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let state = self.state.read(cx);
        let panel = state.panel.clone();
        let open = state.is_open(cx);
        let items = state.items.clone();
        let keyboard = self.state.clone();
        let next = self.state.clone();
        let previous = self.state.clone();
        let first = self.state.clone();
        let last = self.state.clone();
        let tab = self.state.clone();
        let shift_tab = self.state.clone();
        let content = div()
            .id("menu-content")
            .flex()
            .flex_col()
            .min_w(px(180.))
            .p(px(4.))
            .bg(theme.control_background)
            .text_color(theme.foreground)
            .rounded(px(8.))
            .border_1()
            .border_color(theme.control_border)
            .children(items.into_iter().enumerate().map(|(index, (item, focus))| {
                let selected_state = self.state.clone();
                let hover_focus = focus.clone();
                let row = div()
                    .id(item.id)
                    .role(Role::MenuItem)
                    .aria_label(item.label.clone())
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(4.))
                    .text_size(px(13.))
                    .child(item.label);
                if item.disabled {
                    row.on_any_mouse_down(|_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .opacity(0.5)
                    .a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
                } else {
                    row.track_focus(&focus)
                        .focus(move |style| {
                            style.bg(theme.accent).text_color(theme.accent_foreground)
                        })
                        .on_hover(move |hovered, window, cx| {
                            if *hovered {
                                hover_focus.focus(window, cx);
                            }
                        })
                        .on_click(move |_, window, cx| {
                            selected_state.update(cx, |state, cx| state.select(index, window, cx))
                        })
                }
            }));
        Popover::new(&panel, self.trigger, self.label, content)
            .role(Role::Menu)
            .render_root(window, cx)
            .when(open, |root| {
                root.key_context("CupertinoMenu")
                    .on_action(move |_: &NextItem, window, cx| {
                        next.update(cx, |state, cx| state.navigate("down", window, cx))
                    })
                    .on_action(move |_: &PreviousItem, window, cx| {
                        previous.update(cx, |state, cx| state.navigate("up", window, cx))
                    })
                    .on_action(move |_: &FirstItem, window, cx| {
                        first.update(cx, |state, cx| state.navigate("home", window, cx))
                    })
                    .on_action(move |_: &LastItem, window, cx| {
                        last.update(cx, |state, cx| state.navigate("end", window, cx))
                    })
                    .on_action(move |_: &NextControl, window, cx| {
                        tab.update(cx, |state, cx| state.close(window, cx));
                        window.focus_next(cx);
                    })
                    .on_action(move |_: &PreviousControl, window, cx| {
                        shift_tab.update(cx, |state, cx| state.close(window, cx));
                        window.focus_prev(cx);
                    })
                    .on_key_down(move |event, window, cx| {
                        match event.keystroke.key.as_str() {
                            "up" | "down" | "home" | "end" => keyboard.update(cx, |state, cx| {
                                state.navigate(&event.keystroke.key, window, cx)
                            }),
                            "tab" => {
                                keyboard.update(cx, |state, cx| state.close(window, cx));
                                if event.keystroke.modifiers.shift {
                                    window.focus_prev(cx)
                                } else {
                                    window.focus_next(cx)
                                }
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                    })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Button;
    use gpui::{Render, TestAppContext};

    struct StretchedMenu(Entity<MenuState>);

    impl Render for StretchedMenu {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().flex().items_stretch().h(px(100.)).child(Menu::new(
                &self.0,
                div()
                    .id("trigger")
                    .debug_selector(|| "stretched-trigger".into())
                    .h_full()
                    .child(Button::new("button", "Menu").h_full()),
                "Actions",
            ))
        }
    }

    #[gpui::test]
    fn closed_menu_preserves_the_triggers_stretched_height(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let (_, visual) =
            cx.add_window_view(|_, cx| StretchedMenu(cx.new(|cx| MenuState::new(Vec::new(), cx))));
        visual.run_until_parked();
        assert_eq!(
            visual
                .debug_bounds("stretched-trigger")
                .unwrap()
                .size
                .height,
            px(100.)
        );
    }
}
