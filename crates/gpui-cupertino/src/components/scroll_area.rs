//! Scrollable content with GPUI wheel input, keyboard navigation, and accessibility actions.

use gpui::{
    AccessibleAction, AnyElement, App, Axis, BoxShadow, ElementId, FocusHandle, IntoElement,
    ParentElement, Pixels, RenderOnce, Role, ScrollHandle, SharedString, StyleRefinement, Window,
    div, prelude::*, px,
};

use crate::theme::Theme;

/// The axes on which a scroll area permits movement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxes {
    /// Scroll vertically and clip horizontal overflow.
    #[default]
    Vertical,
    /// Scroll horizontally and clip vertical overflow.
    Horizontal,
    /// Scroll on both axes.
    Both,
}

impl ScrollAxes {
    fn permits(self, axis: Axis) -> bool {
        self == Self::Both
            || matches!(
                (self, axis),
                (Self::Vertical, Axis::Vertical) | (Self::Horizontal, Axis::Horizontal)
            )
    }

    fn primary(self) -> Axis {
        if self == Self::Horizontal {
            Axis::Horizontal
        } else {
            Axis::Vertical
        }
    }
}

struct State {
    scroll: ScrollHandle,
    focus: FocusHandle,
}

/// A bounded scroll viewport. Give this element a height or a flex layout constraint.
///
/// The area keeps its offset under a stable ID. Focus the area to use arrow keys,
/// Page Up/Down, Home, or End. Focused child controls keep their own key behavior.
/// GPUI supplies wheel scrolling and clipping. This component does not virtualize
/// children; use a virtual list for large collections.
#[derive(IntoElement)]
pub struct ScrollArea {
    id: ElementId,
    label: SharedString,
    axes: ScrollAxes,
    handle: Option<ScrollHandle>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ScrollArea {
    /// Creates a labeled vertical scroll area.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            axes: ScrollAxes::Vertical,
            handle: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Selects the permitted scroll axes.
    pub fn axes(mut self, axes: ScrollAxes) -> Self {
        self.axes = axes;
        self
    }

    /// Uses a caller-owned handle for observing or changing the scroll position.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.handle = Some(handle.clone());
        self
    }
}

impl Styled for ScrollArea {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for ScrollArea {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

fn scroll_by(handle: &ScrollHandle, axis: Axis, amount: Pixels, window: &mut Window) {
    let mut offset = handle.offset();
    let maximum = handle.max_offset();
    match axis {
        Axis::Vertical => offset.y = (offset.y - amount).clamp(-maximum.y, px(0.)),
        Axis::Horizontal => offset.x = (offset.x - amount).clamp(-maximum.x, px(0.)),
    }
    handle.set_offset(offset);
    window.refresh();
}

fn page_size(handle: &ScrollHandle, axis: Axis) -> Pixels {
    match axis {
        Axis::Vertical => handle.bounds().size.height * 0.9,
        Axis::Horizontal => handle.bounds().size.width * 0.9,
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| State {
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle().tab_stop(true),
        });
        let state = state.read(cx);
        let handle = self.handle.unwrap_or_else(|| state.scroll.clone());
        let focus = state.focus.clone();
        let theme = Theme::for_window(window);
        let axes = self.axes;
        let mut offset = handle.offset();
        if !axes.permits(Axis::Horizontal) {
            offset.x = px(0.);
        }
        if !axes.permits(Axis::Vertical) {
            offset.y = px(0.);
        }
        handle.set_offset(offset);
        let keyboard_handle = handle.clone();
        let mut area = div()
            .id(self.id)
            .role(Role::ScrollView)
            .aria_label(self.label)
            .track_focus(&focus)
            .tab_index(0)
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .track_scroll(&handle)
            .when(axes.permits(Axis::Vertical), |area| {
                area.overflow_y_scroll()
            })
            .when(axes.permits(Axis::Horizontal), |area| {
                area.overflow_x_scroll()
            })
            .focus_visible(move |style| {
                style.shadow(vec![
                    BoxShadow::new(px(0.), px(0.), theme.focus_ring).spread_radius(px(2.)),
                ])
            })
            .on_key_down(move |event, window, cx| {
                if !focus.is_focused(window) || event.keystroke.modifiers.modified() {
                    return;
                }
                let primary = axes.primary();
                let movement = match event.keystroke.key.as_str() {
                    "up" => Some((Axis::Vertical, px(-40.))),
                    "down" => Some((Axis::Vertical, px(40.))),
                    "left" => Some((Axis::Horizontal, px(-40.))),
                    "right" => Some((Axis::Horizontal, px(40.))),
                    "pageup" => Some((primary, -page_size(&keyboard_handle, primary))),
                    "pagedown" => Some((primary, page_size(&keyboard_handle, primary))),
                    "home" | "end" => {
                        let sign = if event.keystroke.key == "home" {
                            -1.
                        } else {
                            1.
                        };
                        let maximum = keyboard_handle.max_offset();
                        Some((
                            primary,
                            match primary {
                                Axis::Vertical => maximum.y * sign,
                                Axis::Horizontal => maximum.x * sign,
                            },
                        ))
                    }
                    _ => None,
                };
                if let Some((axis, amount)) = movement.filter(|(axis, _)| axes.permits(*axis)) {
                    scroll_by(&keyboard_handle, axis, amount, window);
                    cx.stop_propagation();
                }
            })
            .children(self.children);
        for (action, axis, direction) in [
            (AccessibleAction::ScrollUp, Axis::Vertical, -1.),
            (AccessibleAction::ScrollDown, Axis::Vertical, 1.),
            (AccessibleAction::ScrollLeft, Axis::Horizontal, -1.),
            (AccessibleAction::ScrollRight, Axis::Horizontal, 1.),
        ] {
            if axes.permits(axis) {
                let handle = handle.clone();
                area = area.on_a11y_action(action, move |_, window, _| {
                    scroll_by(&handle, axis, page_size(&handle, axis) * direction, window);
                });
            }
        }
        area.style().refine(&self.style);
        area
    }
}
