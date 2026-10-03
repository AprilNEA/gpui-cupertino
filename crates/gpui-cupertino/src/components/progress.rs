use std::{error::Error, fmt};

use gpui::{
    App, Bounds, ElementId, IntoElement, RenderOnce, Role, SharedString, StyleRefinement, Window,
    canvas, div, fill, point, prelude::*, px, size,
};

use crate::theme::Theme;

/// A progress fraction outside the finite inclusive interval `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressValueError;

impl fmt::Display for ProgressValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("progress must be finite and between zero and one")
    }
}

impl Error for ProgressValueError {}

/// An accessible determinate progress bar or indeterminate loading indicator.
///
/// Indeterminate progress animates only during visible painting. Reduced motion
/// displays a stationary segment. Removing or clipping the control stops frame requests.
#[derive(IntoElement)]
pub struct Progress {
    id: ElementId,
    label: SharedString,
    value: Option<f64>,
    style: StyleRefinement,
}

impl Progress {
    /// Creates an indeterminate loading indicator with an accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: None,
            style: StyleRefinement::default(),
        }
    }

    /// Sets determinate progress. Rejects nonfinite values and values outside `0..=1`.
    pub fn value(mut self, value: f64) -> Result<Self, ProgressValueError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ProgressValueError);
        }
        self.value = Some(value);
        Ok(self)
    }
}

impl Styled for Progress {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Progress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            let visibility = cx.observe_window_visibility(window, |_, _, window, cx| {
                if window.is_visible() {
                    cx.notify();
                }
            });
            (cx.background_executor().now(), visibility)
        });
        let epoch = state.read(cx).0;
        let value = self.value;
        let mut element = div()
            .id(self.id)
            .role(Role::ProgressIndicator)
            .aria_label(self.label)
            .w(px(160.0))
            .h(px(6.0))
            .rounded_full()
            .when_some(value, |element, value| {
                element
                    .aria_numeric_value(value)
                    .aria_min_numeric_value(0.0)
                    .aria_max_numeric_value(1.0)
            })
            .a11y_synthetic_children(move |builder| {
                if value.is_none_or(|value| value < 1.0) {
                    builder.parent_node().set_busy();
                }
            });
        element.style().refine(&self.style);
        element.child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    if !window.is_visible()
                        || bounds.intersect(&window.content_mask().bounds).is_empty()
                    {
                        return;
                    }
                    let rounding = bounds.size.height / 2.0;
                    window.paint_quad(
                        fill(bounds, theme.control_border.opacity(0.35)).corner_radii(rounding),
                    );
                    let (offset, width) = if let Some(value) = value {
                        (0.0, value as f32)
                    } else if cx.reduce_motion() {
                        (0.35, 0.3)
                    } else {
                        window.request_animation_frame();
                        let phase = ((cx.background_executor().now() - epoch).as_secs_f64() % 1.4
                            / 1.4) as f32;
                        (
                            0.7 * (0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()),
                            0.3,
                        )
                    };
                    let filled = Bounds::new(
                        point(bounds.left() + bounds.size.width * offset, bounds.top()),
                        size(bounds.size.width * width, bounds.size.height),
                    );
                    window.paint_quad(fill(filled, theme.accent).corner_radii(rounding));
                },
            )
            .size_full(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_rejects_unknown_and_out_of_range_fractions() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.1] {
            assert!(Progress::new("progress", "Download").value(value).is_err());
        }
        for value in [0.0, 0.5, 1.0] {
            assert!(Progress::new("progress", "Download").value(value).is_ok());
        }
    }
}
