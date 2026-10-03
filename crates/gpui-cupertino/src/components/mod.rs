//! Reusable controls with GPUI input, focus, and accessibility behavior.

mod button;
mod choice;
mod choice_group;
mod collections;
mod layout;
mod menu;
mod modal;
mod navigation;
mod numeric_controls;
mod panel_state;
mod popover;
mod progress;
mod scroll_area;
mod segmented_control;
mod split_view;
mod text_input;
mod tooltip;
mod value_range;

pub use button::Button;
pub use choice::{CheckState, Checkbox, Toggle};
pub use choice_group::{ChoiceOption, RadioGroup};
pub use collections::{
    CollectionError, CollectionEvent, CollectionRow, CollectionView, ReorderDirection,
    SelectionMode, SortDirection, TableColumn, TableSort,
};
pub use layout::{EmptyState, Form, FormField, FormSection, Toolbar};
pub use menu::{Menu, MenuEvent, MenuItem, MenuState};
pub use modal::{Dialog, ModalHost, Sheet, SheetEdge};
pub use navigation::{Sidebar, Tab, Tabs};
pub use numeric_controls::{Slider, Stepper};
pub use panel_state::{PanelState, PopoverState};
pub use popover::Popover;
pub use progress::{Progress, ProgressValueError};
pub use scroll_area::{ScrollArea, ScrollAxes};
pub use segmented_control::SegmentedControl;
pub use split_view::SplitView;
pub use text_input::{TextInput, TextInputEvent};
pub use tooltip::{Tooltip, TooltipState};
pub use value_range::{ValueRange, ValueRangeError};

pub(crate) fn init(cx: &mut gpui::App) {
    text_input::init(cx);
    choice_group::init(cx);
    collections::init(cx);
    numeric_controls::init(cx);
    scroll_area::init(cx);
    modal::init(cx);
    menu::init(cx);
    tooltip::init(cx);
}

#[cfg(test)]
mod panels_tests;
