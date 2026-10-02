//! Application forms and labeled layout containers.

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, Role, SharedString,
    StyleRefinement, Window, div, prelude::*, px,
};

use crate::theme::Theme;

/// A vertically arranged form. Child controls retain their own input and focus state.
#[derive(IntoElement)]
pub struct Form {
    id: ElementId,
    label: SharedString,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Form {
    /// Creates a form with a stable identity and accessible name.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl RenderOnce for Form {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut form = div()
            .id(self.id)
            .role(Role::Form)
            .aria_label(self.label)
            .flex()
            .flex_col()
            .gap(px(24.))
            .min_w_0()
            .children(self.children);
        form.style().refine(&self.style);
        form
    }
}

/// A named form group with optional explanatory text.
#[derive(IntoElement)]
pub struct FormSection {
    id: ElementId,
    title: SharedString,
    description: Option<SharedString>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl FormSection {
    /// Creates a form section with a visible heading and accessible group name.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Adds explanatory text beneath the section's controls.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl RenderOnce for FormSection {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let mut section = div()
            .id(self.id)
            .role(Role::Group)
            .aria_label(self.title.clone())
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_color(theme.foreground)
            .text_size(px(13.))
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.title),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .p(px(16.))
                    .rounded(px(10.))
                    .bg(theme.control_background)
                    .border_1()
                    .border_color(theme.control_border)
                    .children(self.children),
            )
            .when_some(self.description, |section, description| {
                section.aria_description(description.clone()).child(
                    div()
                        .text_color(theme.secondary_foreground)
                        .child(description),
                )
            });
        section.style().refine(&self.style);
        section
    }
}

/// A visible field label, control, and optional help or validation message.
///
/// Give the control the same accessible name. The field does not replace the
/// control's label, editing state, or validation behavior.
#[derive(IntoElement)]
pub struct FormField {
    id: ElementId,
    label: SharedString,
    control: AnyElement,
    description: Option<SharedString>,
    error: Option<SharedString>,
    style: StyleRefinement,
}

impl FormField {
    /// Arranges a label above its control.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            control: control.into_any_element(),
            description: None,
            error: None,
            style: StyleRefinement::default(),
        }
    }

    /// Adds help text beneath the control.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Displays the caller's validation message as an accessible alert.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }
}

impl Styled for FormField {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FormField {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let mut field = div()
            .id(self.id)
            .role(Role::Group)
            .aria_label(self.label.clone())
            .flex()
            .flex_col()
            .gap(px(6.))
            .min_w_0()
            .text_color(theme.foreground)
            .text_size(px(13.))
            .child(self.label)
            .child(self.control)
            .when_some(self.description, |field, description| {
                field.aria_description(description.clone()).child(
                    div()
                        .text_color(theme.secondary_foreground)
                        .child(description),
                )
            })
            .when_some(self.error, |field, error| {
                field.child(
                    div()
                        .id("validation-error")
                        .role(Role::Alert)
                        .aria_label(error.clone())
                        .a11y_synthetic_children({
                            let error = error.clone();
                            move |builder| {
                                let node = builder.parent_node();
                                node.set_value(error.to_string());
                                node.set_live(gpui::accesskit::Live::Assertive);
                            }
                        })
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(error),
                )
            });
        field.style().refine(&self.style);
        field
    }
}

/// A labeled horizontal toolbar. Controls keep their normal Tab traversal.
#[derive(IntoElement)]
pub struct Toolbar {
    id: ElementId,
    label: SharedString,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Toolbar {
    /// Creates a toolbar for caller-supplied buttons, selectors, and other controls.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let mut toolbar = div()
            .id(self.id)
            .role(Role::Toolbar)
            .aria_label(self.label)
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .bg(theme.control_background)
            .text_color(theme.foreground)
            .border_b_1()
            .border_color(theme.control_border)
            .children(self.children);
        toolbar.style().refine(&self.style);
        toolbar
    }
}

/// A centered explanation and optional actions for an empty collection or detail view.
#[derive(IntoElement)]
pub struct EmptyState {
    id: ElementId,
    title: SharedString,
    description: SharedString,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl EmptyState {
    /// Creates visible and accessible empty-state text.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl RenderOnce for EmptyState {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let mut empty = div()
            .id(self.id)
            .role(Role::Group)
            .aria_label(self.title.clone())
            .aria_description(self.description.clone())
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .p(px(24.))
            .text_color(theme.foreground)
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.title),
            )
            .child(
                div()
                    .text_color(theme.secondary_foreground)
                    .child(self.description),
            )
            .children(self.children);
        empty.style().refine(&self.style);
        empty
    }
}

macro_rules! container_traits {
    ($($name:ident),+ $(,)?) => {
        $(
            impl Styled for $name {
                fn style(&mut self) -> &mut StyleRefinement { &mut self.style }
            }

            impl ParentElement for $name {
                fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                    self.children.extend(elements);
                }
            }
        )+
    };
}

container_traits!(Form, FormSection, Toolbar, EmptyState);
