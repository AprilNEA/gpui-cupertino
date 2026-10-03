//! Controlled navigation with shared option identity and keyboard behavior.

use gpui::{
    AnyElement, App, ElementId, IntoElement, RenderOnce, Role, SharedString, StyleRefinement,
    Window, div, prelude::*, px,
};

use super::choice_group::{ChoiceGroup, ChoiceOption, ChoicePresentation};

type ChangeListener = Box<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// One tab and its content. Retain stateful content entities in the owning view.
pub struct Tab {
    id: SharedString,
    label: SharedString,
    disabled: bool,
    content: AnyElement,
}

impl Tab {
    /// Creates a tab with a stable identity and an accessible, visible label.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            content: content.into_any_element(),
        }
    }

    /// Prevents pointer, keyboard, and accessibility selection of this tab.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A tab list and its selected panel.
///
/// Selection belongs to the caller. Only the selected content is mounted. Arrow
/// keys, Home, and End select enabled tabs. IDs must be unique within the list.
/// An unknown selection renders no panel until the caller selects an existing tab.
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    label: SharedString,
    selected: SharedString,
    tabs: Vec<Tab>,
    listener: Option<ChangeListener>,
    style: StyleRefinement,
}

impl Tabs {
    /// Creates controlled tabs with one explicitly selected identity.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        selected: impl Into<SharedString>,
        tabs: impl IntoIterator<Item = Tab>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: selected.into(),
            tabs: tabs.into_iter().collect(),
            listener: None,
            style: StyleRefinement::default(),
        }
    }

    /// Receives a new selection. Update the caller's value and notify its view.
    pub fn on_change(
        mut self,
        listener: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.listener = Some(Box::new(listener));
        self
    }
}

impl Styled for Tabs {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tabs {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let options = self
            .tabs
            .iter()
            .map(|tab| ChoiceOption::new(tab.id.clone(), tab.label.clone()).disabled(tab.disabled));
        let choices = ChoiceGroup::new("tab-list", self.label, options)
            .presentation(ChoicePresentation::Tabs)
            .selected(self.selected.clone())
            .when_some(self.listener, |choices, listener| {
                choices.on_change(listener)
            });
        let panel = self.tabs.into_iter().find(|tab| tab.id == self.selected);
        let mut root = div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(choices);
        root.style().refine(&self.style);
        root.when_some(panel, |root, tab| {
            root.child(
                div().id("tab-panel").min_h_0().flex_1().child(
                    div()
                        .id(tab.id)
                        .role(Role::TabPanel)
                        .aria_label(tab.label)
                        .size_full()
                        .child(tab.content),
                ),
            )
        })
    }
}

/// A single-selection sidebar with stable item identities and roving focus.
///
/// Up/Down, Home, and End select enabled items. The caller renders the associated
/// detail content separately and retains the selected item across data updates.
#[derive(IntoElement)]
pub struct Sidebar(ChoiceGroup);

impl Sidebar {
    /// Creates labeled navigation from unique, stable option IDs.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Self {
        Self(ChoiceGroup::new(id, label, options).presentation(ChoicePresentation::Sidebar))
    }

    /// Selects the item with this identity.
    pub fn selected(mut self, selected: impl Into<SharedString>) -> Self {
        self.0 = self.0.selected(selected);
        self
    }

    /// Disables every navigation item and removes the group from Tab traversal.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.0 = self.0.disabled(disabled);
        self
    }

    /// Receives a new selected identity from pointer, keyboard, or accessibility input.
    pub fn on_change(
        mut self,
        listener: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.0 = self.0.on_change(listener);
        self
    }
}

impl Styled for Sidebar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}
