use std::{cell::Cell, rc::Rc};

use gpui::{
    Along, AnyElement, App, Axis, Bounds, CursorStyle, ElementId, FocusHandle, IntoElement,
    MouseButton, RenderOnce, Role, SharedString, StyleRefinement, Window, accesskit::Orientation,
    canvas, div, fill, point, prelude::*, px, size,
};

use super::{
    ValueRange, ValueRangeError,
    numeric_controls::{Interaction, NumericControl, capture_drag, emit},
};
use crate::theme::Theme;

const DIVIDER: f32 = 8.0;

struct PaneState {
    first_focus: FocusHandle,
    second_focus: FocusHandle,
    offset: Rc<Cell<f32>>,
    axis: Axis,
}

/// Two clipped panes separated by a controlled, keyboard-accessible divider.
///
/// The ratio describes the first pane's share after subtracting the divider.
/// Retain the requested ratio in the parent. The caller retains child entities.
#[derive(IntoElement)]
pub struct SplitView {
    control: NumericControl,
    axis: Axis,
    first: AnyElement,
    second: AnyElement,
}

impl SplitView {
    /// Creates horizontal panes at ratio `0.5`, bounded by `0.1..=0.9` in `0.01` steps.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        first: impl IntoElement,
        second: impl IntoElement,
    ) -> Self {
        let mut control = NumericControl::new(
            id,
            label,
            ValueRange::new(0.1, 0.9, 0.01).expect("constant fractional range is valid"),
        );
        control.value = 0.5;
        Self {
            control,
            axis: Axis::Horizontal,
            first: first.into_any_element(),
            second: second.into_any_element(),
        }
    }

    /// Places the first pane above the second pane.
    pub fn vertical(mut self) -> Self {
        self.axis = Axis::Vertical;
        self
    }

    /// Sets fractional bounds and a keyboard step, then snaps the current ratio.
    ///
    /// Bounds must be inside `0..=1`. Explicit endpoints zero and one permit
    /// collapsing a pane. A collapsed pane does not render its child content.
    pub fn range(mut self, range: ValueRange) -> Result<Self, ValueRangeError> {
        if range.minimum() < 0.0 || range.maximum() > 1.0 {
            return Err(ValueRangeError::FractionBounds);
        }
        self.control.value = range.snap(self.control.value)?;
        self.control.range = range;
        Ok(self)
    }

    /// Sets a finite ratio, clamped and snapped to the configured range.
    pub fn ratio(mut self, ratio: f64) -> Result<Self, ValueRangeError> {
        self.control.value = self.control.range.snap(ratio)?;
        Ok(self)
    }

    /// Disables divider interaction. Pane contents remain interactive.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.control.disabled = disabled;
        self
    }

    /// Uses a caller-owned focus handle for the divider.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.control.focus = Some(focus.clone());
        self
    }

    /// Receives requested ratios after pointer, keyboard, or accessibility input.
    pub fn on_change(mut self, listener: impl Fn(&f64, &mut Window, &mut App) + 'static) -> Self {
        self.control.on_change = Some(Rc::new(listener));
        self
    }
}

impl Styled for SplitView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.control.style
    }
}

impl RenderOnce for SplitView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let control = self.control;
        let theme = Theme::for_window(window);
        let axis = self.axis;
        let Interaction {
            focus,
            dragging,
            bounds,
        } = control.interaction(window, cx);
        let state = window.use_keyed_state(control.id.clone(), cx, |_, cx| PaneState {
            first_focus: cx.focus_handle(),
            second_focus: cx.focus_handle(),
            offset: Rc::default(),
            axis,
        });
        let (first_focus, second_focus, offset) = state.update(cx, |state, cx| {
            if state.axis != axis {
                dragging.set(false);
                state.axis = axis;
            }
            if (control.value == 0.0 && state.first_focus.contains_focused(window, cx))
                || (control.value == 1.0 && state.second_focus.contains_focused(window, cx))
            {
                if control.disabled {
                    window.blur(cx);
                } else {
                    focus.focus(window, cx);
                }
            }
            (
                state.first_focus.clone(),
                state.second_focus.clone(),
                state.offset.clone(),
            )
        });
        let ratio = control.value;
        let range = control.range;
        let mut divider = control
            .base(Role::Splitter, &focus, theme)
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex_shrink_0()
            .bg(theme.control_background)
            .aria_orientation(if axis == Axis::Horizontal {
                Orientation::Vertical
            } else {
                Orientation::Horizontal
            })
            .when(axis == Axis::Horizontal, |divider| {
                divider.w(px(DIVIDER)).h_full()
            })
            .when(axis == Axis::Vertical, |divider| {
                divider.h(px(DIVIDER)).w_full()
            });
        let listener = control.on_change.filter(|_| !control.disabled);
        if listener.is_some() {
            let dragging = dragging.clone();
            let bounds = bounds.clone();
            let offset = offset.clone();
            divider = divider
                .cursor(if axis == Axis::Horizontal {
                    CursorStyle::ResizeColumn
                } else {
                    CursorStyle::ResizeRow
                })
                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    focus.focus(window, cx);
                    let bounds = bounds.get();
                    let available = bounds.size.along(axis) - px(DIVIDER);
                    if available <= px(0.0) {
                        return;
                    }
                    offset.set(
                        (event.position.along(axis)
                            - bounds.origin.along(axis)
                            - available * ratio as f32)
                            .as_f32(),
                    );
                    dragging.set(true);
                    cx.stop_propagation();
                });
        }
        let measuring = bounds.clone();
        divider = divider.child(
            canvas(
                |_, _, _| (),
                move |divider, _, window, _| {
                    let line = if axis == Axis::Horizontal {
                        Bounds::new(
                            point(divider.center().x - px(0.5), divider.top()),
                            size(px(1.0), divider.size.height),
                        )
                    } else {
                        Bounds::new(
                            point(divider.left(), divider.center().y - px(0.5)),
                            size(divider.size.width, px(1.0)),
                        )
                    };
                    window.paint_quad(fill(line, theme.control_border));
                    if let Some(listener) = listener {
                        capture_drag(dragging, window, move |position, window, cx| {
                            let bounds = bounds.get();
                            let available = bounds.size.along(axis) - px(DIVIDER);
                            if available > px(0.0)
                                && let Ok(next) = range.snap(
                                    ((position.along(axis)
                                        - bounds.origin.along(axis)
                                        - px(offset.get()))
                                        / available) as f64,
                                )
                            {
                                emit(&listener, ratio, next, window, cx);
                            }
                        });
                    }
                },
            )
            .size_full(),
        );

        let pane = |id, share: f32| {
            div()
                .id(id)
                .min_w(px(0.0))
                .min_h(px(0.0))
                .flex_basis(px(0.0))
                .flex_grow(share)
                .flex_shrink_0()
                .overflow_hidden()
                .when(axis == Axis::Horizontal, |pane| pane.h_full())
                .when(axis == Axis::Vertical, |pane| pane.w_full())
        };
        let first = pane("first-pane", ratio as f32).when(ratio > 0.0, |pane| {
            pane.track_focus(&first_focus).child(self.first)
        });
        let second = pane("second-pane", (1.0 - ratio) as f32).when(ratio < 1.0, |pane| {
            pane.track_focus(&second_focus).child(self.second)
        });
        let content = div()
            .relative()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .when(axis == Axis::Vertical, |content| content.flex_col())
            .child(
                canvas(move |bounds, _, _| measuring.set(bounds), |_, _, _, _| {})
                    .absolute()
                    .size_full()
                    .top(px(0.0))
                    .left(px(0.0)),
            )
            .child(first)
            .child(divider)
            .child(second);
        let mut root = div()
            .id(control.id)
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0));
        root.style().refine(&control.style);
        root.child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratios_require_finite_values_and_fractional_ranges() {
        assert!(matches!(
            SplitView::new("split", "Split", div(), div()).ratio(f64::NAN),
            Err(ValueRangeError::NonFiniteValue)
        ));
        for range in [
            ValueRange::new(-1.0, 1.0, 0.1).unwrap(),
            ValueRange::new(0.0, 2.0, 0.1).unwrap(),
        ] {
            assert!(matches!(
                SplitView::new("split", "Split", div(), div()).range(range),
                Err(ValueRangeError::FractionBounds)
            ));
        }
    }
}
