use std::{collections::HashSet, rc::Rc};

use gpui::{
    App, BoxShadow, ElementId, FocusHandle, IntoElement, KeyBinding, RenderOnce, Role,
    SharedString, StyleRefinement, Window,
    accesskit::{Orientation, Toggled},
    div,
    prelude::*,
    px,
};

use crate::theme::Theme;

type ChangeListener = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(Clone, PartialEq, gpui::Action)]
#[action(no_json, no_register)]
struct Navigate(&'static str);

pub(crate) fn init(cx: &mut App) {
    for (context, keys) in [
        (
            "CupertinoRadioGroup",
            &["left", "right", "up", "down", "home", "end"][..],
        ),
        (
            "CupertinoHorizontalChoice",
            &["left", "right", "home", "end"][..],
        ),
        ("CupertinoSidebar", &["up", "down", "home", "end"][..]),
    ] {
        cx.bind_keys(
            keys.iter()
                .map(|key| KeyBinding::new(key, Navigate(key), Some(context))),
        );
    }
}

/// A labeled single-selection option with an identity independent of its position.
#[derive(Clone, Debug)]
pub struct ChoiceOption {
    id: SharedString,
    label: SharedString,
    disabled: bool,
}

impl ChoiceOption {
    /// Creates an option. IDs must be unique within the containing control.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Rejects activation and skips the option during keyboard navigation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ChoicePresentation {
    #[default]
    Radio,
    Segmented,
    Tabs,
    Sidebar,
}

impl ChoicePresentation {
    fn roles(self) -> (Role, Role) {
        match self {
            Self::Radio | Self::Segmented => (Role::RadioGroup, Role::RadioButton),
            Self::Tabs => (Role::TabList, Role::Tab),
            Self::Sidebar => (Role::ListBox, Role::ListBoxOption),
        }
    }

    fn horizontal(self) -> bool {
        matches!(self, Self::Segmented | Self::Tabs)
    }

    fn destination(self, key: &str, current: usize, count: usize) -> Option<usize> {
        match key {
            "home" => Some(0),
            "end" => Some(count - 1),
            "left" if self.horizontal() || self == Self::Radio => {
                Some((current + count - 1) % count)
            }
            "right" if self.horizontal() || self == Self::Radio => Some((current + 1) % count),
            "up" if !self.horizontal() => Some((current + count - 1) % count),
            "down" if !self.horizontal() => Some((current + 1) % count),
            _ => None,
        }
    }
}

#[derive(Default)]
struct FocusState(Vec<(SharedString, FocusHandle)>);

#[derive(IntoElement)]
pub(crate) struct ChoiceGroup {
    id: ElementId,
    label: SharedString,
    options: Vec<ChoiceOption>,
    selected: Option<SharedString>,
    disabled: bool,
    presentation: ChoicePresentation,
    listener: Option<ChangeListener>,
    style: StyleRefinement,
}

impl ChoiceGroup {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Self {
        let options: Vec<_> = options.into_iter().collect();
        let mut ids = HashSet::with_capacity(options.len());
        assert!(
            options.iter().all(|option| ids.insert(option.id.clone())),
            "choice option IDs must be unique"
        );
        Self {
            id: id.into(),
            label: label.into(),
            options,
            selected: None,
            disabled: false,
            presentation: ChoicePresentation::Radio,
            listener: None,
            style: StyleRefinement::default(),
        }
    }

    pub(crate) fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn presentation(mut self, presentation: ChoicePresentation) -> Self {
        self.presentation = presentation;
        self
    }

    pub(crate) fn on_change(
        mut self,
        listener: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.listener = Some(Rc::new(listener));
        self
    }
}

impl Styled for ChoiceGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ChoiceGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| FocusState::default());
        let enabled: Vec<_> = self
            .options
            .iter()
            .filter(|option| !self.disabled && !option.disabled)
            .collect();
        let tab_id = enabled
            .iter()
            .find(|option| Some(&option.id) == self.selected.as_ref())
            .or_else(|| enabled.first())
            .map(|option| option.id.clone());
        let handles = state.update(cx, |state, cx| {
            let focused = state
                .0
                .iter()
                .find(|(_, handle)| handle.is_focused(window))
                .map(|(id, _)| id.clone());
            state
                .0
                .retain(|(id, _)| self.options.iter().any(|option| option.id == *id));
            for option in &self.options {
                if !state.0.iter().any(|(id, _)| *id == option.id) {
                    state.0.push((option.id.clone(), cx.focus_handle()));
                }
            }
            if focused.is_some_and(|id| !enabled.iter().any(|option| option.id == id)) {
                if let Some((_, focus)) = state.0.iter().find(|(id, _)| Some(id) == tab_id.as_ref())
                {
                    focus.focus(window, cx);
                } else {
                    window.blur(cx);
                }
            }
            state.0.clone()
        });
        let navigation: Rc<Vec<_>> = Rc::new(
            enabled
                .iter()
                .map(|option| {
                    let (_, handle) = handles
                        .iter()
                        .find(|(id, _)| *id == option.id)
                        .expect("every current option has a focus handle");
                    (option.id.clone(), handle.clone())
                })
                .collect(),
        );
        let presentation = self.presentation;
        let (group_role, option_role) = presentation.roles();
        let mut group = div()
            .id(self.id)
            .role(group_role)
            .aria_label(self.label)
            .aria_orientation(if presentation.horizontal() {
                Orientation::Horizontal
            } else {
                Orientation::Vertical
            })
            .flex()
            .when(!presentation.horizontal(), |group| group.flex_col())
            .gap(px(if presentation == ChoicePresentation::Radio {
                4.0
            } else {
                2.0
            }))
            .when(presentation == ChoicePresentation::Segmented, |group| {
                group
                    .p(px(2.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.control_border)
                    .bg(theme.control_background)
            })
            .when(self.disabled, |group| {
                group.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
            });
        group.style().refine(&self.style);
        for option in self.options {
            let selected = self.selected.as_ref() == Some(&option.id);
            let disabled = self.disabled || option.disabled;
            let (_, focus) = handles
                .iter()
                .find(|(id, _)| *id == option.id)
                .expect("every current option has a focus handle");
            let mut row = div()
                .id(option.id.clone())
                .role(option_role)
                .aria_label(option.label.clone())
                .when(
                    matches!(
                        presentation,
                        ChoicePresentation::Radio | ChoicePresentation::Segmented
                    ),
                    |row| {
                        row.aria_toggled(if selected {
                            Toggled::True
                        } else {
                            Toggled::False
                        })
                    },
                )
                .when(
                    matches!(
                        presentation,
                        ChoicePresentation::Tabs | ChoicePresentation::Sidebar
                    ),
                    |row| row.aria_selected(selected),
                )
                .flex()
                .items_center()
                .gap(px(8.0))
                .min_h(px(28.0))
                .rounded(px(5.0))
                .text_size(px(13.0))
                .line_height(px(18.0))
                .text_color(theme.foreground)
                .when(presentation != ChoicePresentation::Radio, |row| {
                    row.px(px(10.0)).py(px(4.0)).when(selected, |row| {
                        row.bg(theme.accent).text_color(theme.accent_foreground)
                    })
                })
                .when(presentation == ChoicePresentation::Radio, |row| {
                    row.child(
                        div()
                            .size(px(16.0))
                            .rounded_full()
                            .border_1()
                            .border_color(if selected {
                                theme.accent
                            } else {
                                theme.control_border
                            })
                            .bg(if selected {
                                theme.accent
                            } else {
                                theme.control_background
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(selected, |indicator| {
                                indicator.child(
                                    div()
                                        .size(px(6.0))
                                        .rounded_full()
                                        .bg(theme.accent_foreground),
                                )
                            }),
                    )
                })
                .child(option.label);
            if disabled {
                row = row
                    .opacity(0.5)
                    .cursor_default()
                    .a11y_synthetic_children(|builder| builder.parent_node().set_disabled());
            } else {
                let tab_stop = tab_id.as_ref() == Some(&option.id);
                let click_id = option.id.clone();
                let click_listener = self.listener.clone();
                let click_focus = focus.clone();
                let navigation = navigation.clone();
                let selected_id = self.selected.clone();
                let key_listener = self.listener.clone();
                row = row
                    .track_focus(&focus.clone().tab_stop(tab_stop))
                    .key_context(match presentation {
                        ChoicePresentation::Radio => "CupertinoRadioGroup",
                        ChoicePresentation::Segmented | ChoicePresentation::Tabs => {
                            "CupertinoHorizontalChoice"
                        }
                        ChoicePresentation::Sidebar => "CupertinoSidebar",
                    })
                    .cursor_pointer()
                    .hover(move |style| {
                        style.bg(if selected && presentation != ChoicePresentation::Radio {
                            theme.accent.blend(theme.accent_foreground.opacity(0.08))
                        } else {
                            theme.foreground.opacity(0.06)
                        })
                    })
                    .focus_visible(move |style| {
                        style.shadow(vec![
                            BoxShadow::new(px(0.0), px(0.0), theme.focus_ring)
                                .spread_radius(px(2.0)),
                        ])
                    })
                    .on_click(move |_, window, cx| {
                        click_focus.focus(window, cx);
                        if !selected && let Some(listener) = &click_listener {
                            listener(&click_id, window, cx);
                        }
                    })
                    .on_action(move |event: &Navigate, window, cx| {
                        let current = navigation
                            .iter()
                            .position(|(id, _)| *id == option.id)
                            .expect("enabled option belongs to navigation");
                        if let Some(index) =
                            presentation.destination(event.0, current, navigation.len())
                        {
                            let (id, focus) = &navigation[index];
                            focus.focus(window, cx);
                            if Some(id) != selected_id.as_ref()
                                && let Some(listener) = &key_listener
                            {
                                listener(id, window, cx);
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    });
            }
            group = group.child(row);
        }
        group
    }
}

/// A controlled radio group with one tab stop and stable option identities.
///
/// Arrow keys wrap through enabled options. Home and End select the first and last
/// enabled options. If no enabled option is selected, Tab enters the first enabled option.
#[derive(IntoElement)]
pub struct RadioGroup(ChoiceGroup);

impl RadioGroup {
    /// Creates a group. Duplicate option IDs are a programming error and panic.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Self {
        Self(ChoiceGroup::new(id, label, options))
    }

    /// Sets the selected option ID. An absent or unknown ID leaves all options unselected.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.0 = self.0.selected(id);
        self
    }

    /// Disables all options and removes the group's tab stop.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.0 = self.0.disabled(disabled);
        self
    }

    /// Receives a different selected ID after pointer, keyboard, or accessibility input.
    pub fn on_change(
        mut self,
        listener: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.0 = self.0.on_change(listener);
        self
    }
}

impl Styled for RadioGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}
