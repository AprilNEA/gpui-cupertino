use gpui::{
    App, ElementId, IntoElement, RenderOnce, SharedString, StyleRefinement, Styled, Window,
};

use super::choice_group::{ChoiceGroup, ChoiceOption, ChoicePresentation};

/// A controlled segmented selector with radio semantics and one keyboard tab stop.
///
/// Left and Right wrap through enabled options. Home and End select the endpoints.
#[derive(IntoElement)]
pub struct SegmentedControl(ChoiceGroup);

impl SegmentedControl {
    /// Creates a selector. Duplicate option IDs are a programming error and panic.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Self {
        Self(ChoiceGroup::new(id, label, options).presentation(ChoicePresentation::Segmented))
    }

    /// Selects an option by ID. An unknown ID leaves all options unselected.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.0 = self.0.selected(id);
        self
    }

    /// Disables every option and removes the selector from keyboard navigation.
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

impl Styled for SegmentedControl {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}
