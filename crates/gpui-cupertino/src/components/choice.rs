use cupertino::motion::Spring;
use gpui::{
    AnimationExt, App, BoxShadow, ElementId, FocusHandle, IntoElement, RenderOnce, Role,
    SharedString, StyleRefinement, Window, accesskit::Toggled, div, prelude::*, px,
};

use crate::{motion::spring, theme::Theme};

type ChangeListener = Box<dyn Fn(&CheckState, &mut Window, &mut App)>;

/// The checked value of a checkbox, including an aggregate mixed value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    /// No values are selected.
    #[default]
    Unchecked,
    /// All values are selected.
    Checked,
    /// Some values are selected. Activation selects all values.
    Mixed,
}

impl From<bool> for CheckState {
    fn from(checked: bool) -> Self {
        if checked {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }
}

impl CheckState {
    fn toggled(self) -> Toggled {
        match self {
            Self::Unchecked => Toggled::False,
            Self::Checked => Toggled::True,
            Self::Mixed => Toggled::Mixed,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Checked => Self::Unchecked,
            Self::Unchecked | Self::Mixed => Self::Checked,
        }
    }
}

struct Choice {
    id: ElementId,
    label: SharedString,
    value: CheckState,
    disabled: bool,
    focus: Option<FocusHandle>,
    listener: Option<ChangeListener>,
    style: StyleRefinement,
}

impl Choice {
    fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, value: CheckState) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value,
            disabled: false,
            focus: None,
            listener: None,
            style: StyleRefinement::default(),
        }
    }

    fn render(self, toggle: bool, window: &mut Window) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let checked = self.value != CheckState::Unchecked;
        let foreground = if checked {
            theme.accent_foreground
        } else {
            theme.foreground
        };
        let background = if checked {
            theme.accent
        } else {
            theme.control_background
        };
        let indicator = if toggle {
            div()
                .relative()
                .w(px(36.0))
                .h(px(22.0))
                .rounded_full()
                .border_1()
                .border_color(if checked {
                    theme.accent
                } else {
                    theme.control_border
                })
                .bg(background)
                .child(
                    div()
                        .absolute()
                        .top(px(2.0))
                        .size(px(16.0))
                        .rounded_full()
                        .bg(if checked {
                            theme.accent_foreground
                        } else {
                            theme.control_border
                        })
                        .with_spring(
                            "thumb",
                            spring(Spring::SNAPPY)
                                .to(px(if checked { 16.0 } else { 2.0 }))
                                .with_epsilon(0.05),
                            |thumb, position| thumb.left(position),
                        ),
                )
                .into_any_element()
        } else {
            div()
                .size(px(16.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(if checked {
                    theme.accent
                } else {
                    theme.control_border
                })
                .bg(background)
                .text_color(foreground)
                .flex()
                .items_center()
                .justify_center()
                .child(match self.value {
                    CheckState::Unchecked => "",
                    CheckState::Checked => "✓",
                    CheckState::Mixed => "−",
                })
                .into_any_element()
        };
        let mut control = div()
            .id(self.id)
            .role(if toggle { Role::Switch } else { Role::CheckBox })
            .aria_label(self.label.clone())
            .aria_toggled(self.value.toggled())
            .flex()
            .items_center()
            .gap(px(8.0))
            .min_h(px(28.0))
            .rounded(px(4.0))
            .text_size(px(13.0))
            .line_height(px(18.0))
            .text_color(theme.foreground)
            .child(indicator)
            .child(self.label);
        control.style().refine(&self.style);
        if self.disabled {
            control
                .cursor_default()
                .opacity(0.5)
                .a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        } else {
            control
                .tab_index(0)
                .when_some(self.focus, |control, focus| {
                    control.track_focus(&focus.tab_stop(true))
                })
                .cursor_pointer()
                .hover(move |style| style.bg(theme.foreground.opacity(0.04)))
                .active(move |style| style.bg(theme.foreground.opacity(0.08)))
                .focus_visible(move |style| {
                    style.shadow(vec![
                        BoxShadow::new(px(0.0), px(0.0), theme.focus_ring).spread_radius(px(3.0)),
                    ])
                })
                .when_some(self.listener, |control, listener| {
                    control.on_click(move |_, window, cx| listener(&self.value.next(), window, cx))
                })
        }
    }
}

/// A controlled Boolean switch with a retained, reduced-motion-aware thumb spring.
///
/// Keep the ID stable across renders. Update the supplied value in `on_change`.
#[derive(IntoElement)]
pub struct Toggle(Choice);

impl Toggle {
    /// Creates a switch with a visible and accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Self {
        Self(Choice::new(id, label, checked.into()))
    }

    /// Removes the tab stop and rejects pointer, keyboard, and accessibility input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.0.disabled = disabled;
        self
    }

    /// Uses the caller's focus handle and preserves the handle's tab index.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.0.focus = Some(focus.clone());
        self
    }

    /// Receives the proposed value after pointer, keyboard, or accessibility activation.
    pub fn on_change(mut self, listener: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.0.listener = Some(Box::new(move |value, window, cx| {
            listener(&(*value == CheckState::Checked), window, cx);
        }));
        self
    }
}

impl Styled for Toggle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.0.style
    }
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        self.0.render(true, window)
    }
}

/// A controlled checkbox. Activating a mixed value produces `CheckState::Checked`.
#[derive(IntoElement)]
pub struct Checkbox(Choice);

impl Checkbox {
    /// Creates a checkbox with a stable ID and a visible and accessible label.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        value: impl Into<CheckState>,
    ) -> Self {
        Self(Choice::new(id, label, value.into()))
    }

    /// Removes the tab stop and rejects pointer, keyboard, and accessibility input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.0.disabled = disabled;
        self
    }

    /// Uses the caller's focus handle and preserves the handle's tab index.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.0.focus = Some(focus.clone());
        self
    }

    /// Receives the proposed value after pointer, keyboard, or accessibility activation.
    pub fn on_change(
        mut self,
        listener: impl Fn(&CheckState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.0.listener = Some(Box::new(listener));
        self
    }
}

impl Styled for Checkbox {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.0.style
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        self.0.render(false, window)
    }
}
