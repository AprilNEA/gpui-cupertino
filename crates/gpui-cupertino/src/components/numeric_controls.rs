use std::{cell::Cell, rc::Rc};

use gpui::{
    AccessibleAction, App, Bounds, BoxShadow, DispatchPhase, ElementId, FocusHandle, IntoElement,
    KeyBinding, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderOnce, Role,
    SharedString, Stateful, StyleRefinement, Window,
    accesskit::{ActionData, Orientation},
    canvas, div, fill, point,
    prelude::*,
    px, size,
};

use super::{ValueRange, ValueRangeError};
use crate::theme::Theme;

pub(super) type ChangeListener = Rc<dyn Fn(&f64, &mut Window, &mut App)>;

#[derive(Clone, PartialEq, gpui::Action)]
#[action(no_json, no_register)]
enum NumericAction {
    Increment,
    Decrement,
    Minimum,
    Maximum,
}

pub(super) fn init(cx: &mut App) {
    let context = Some("CupertinoNumericControl");
    cx.bind_keys([
        KeyBinding::new("right", NumericAction::Increment, context),
        KeyBinding::new("up", NumericAction::Increment, context),
        KeyBinding::new("left", NumericAction::Decrement, context),
        KeyBinding::new("down", NumericAction::Decrement, context),
        KeyBinding::new("home", NumericAction::Minimum, context),
        KeyBinding::new("end", NumericAction::Maximum, context),
    ]);
}

#[derive(Clone)]
pub(super) struct Interaction {
    pub(super) focus: FocusHandle,
    pub(super) dragging: Rc<Cell<bool>>,
    pub(super) bounds: Rc<Cell<Bounds<Pixels>>>,
}

pub(super) struct NumericControl {
    pub(super) id: ElementId,
    label: SharedString,
    pub(super) range: ValueRange,
    pub(super) value: f64,
    pub(super) disabled: bool,
    pub(super) focus: Option<FocusHandle>,
    pub(super) on_change: Option<ChangeListener>,
    pub(super) style: StyleRefinement,
}

impl NumericControl {
    pub(super) fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        range: ValueRange,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            range,
            value: range.minimum(),
            disabled: false,
            focus: None,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    pub(super) fn base(
        &self,
        role: Role,
        focus: &FocusHandle,
        theme: Theme,
    ) -> Stateful<gpui::Div> {
        let range = self.range;
        let value = self.value;
        let disabled = self.disabled;
        let read_only = self.on_change.is_none();
        let mut element = div()
            .id(self.id.clone())
            .role(role)
            .aria_label(self.label.clone())
            .aria_numeric_value(value)
            .aria_min_numeric_value(range.minimum())
            .aria_max_numeric_value(range.maximum())
            .aria_numeric_value_step(range.step())
            .h(px(28.0))
            .min_w(px(80.0))
            .text_size(px(13.0))
            .text_color(theme.foreground)
            .a11y_synthetic_children(move |builder| {
                if disabled {
                    builder.parent_node().set_disabled();
                }
                if read_only {
                    builder.parent_node().set_read_only();
                }
            });
        if disabled {
            return element.opacity(0.5);
        }
        element = element
            .track_focus(&focus.clone().tab_stop(true))
            .focus_visible(move |style| {
                style.shadow(vec![
                    BoxShadow::new(px(0.0), px(0.0), theme.focus_ring).spread_radius(px(3.0)),
                ])
            });
        if let Some(listener) = &self.on_change {
            let key_listener = listener.clone();
            let increment = listener.clone();
            let decrement = listener.clone();
            let set_value = listener.clone();
            element = element
                .key_context("CupertinoNumericControl")
                .on_action(move |action: &NumericAction, window, cx| {
                    let next = match action {
                        NumericAction::Increment => range.increment(value),
                        NumericAction::Decrement => range.decrement(value),
                        NumericAction::Minimum => range.minimum(),
                        NumericAction::Maximum => range.maximum(),
                    };
                    emit(&key_listener, value, next, window, cx);
                    cx.stop_propagation();
                })
                .on_a11y_action(AccessibleAction::Increment, move |_, window, cx| {
                    emit(&increment, value, range.increment(value), window, cx);
                })
                .on_a11y_action(AccessibleAction::Decrement, move |_, window, cx| {
                    emit(&decrement, value, range.decrement(value), window, cx);
                })
                .on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                    if let Some(ActionData::NumericValue(requested)) = data
                        && let Ok(next) = range.snap(*requested)
                    {
                        emit(&set_value, value, next, window, cx);
                    }
                });
        }
        element
    }

    pub(super) fn interaction(&self, window: &mut Window, cx: &mut App) -> Interaction {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| Interaction {
            focus: cx.focus_handle(),
            dragging: Rc::default(),
            bounds: Rc::default(),
        });
        let mut state = state.read(cx).clone();
        if let Some(focus) = &self.focus {
            state.focus = focus.clone();
        }
        if self.disabled || self.on_change.is_none() || !state.focus.is_focused(window) {
            state.dragging.set(false);
        }
        if self.disabled && state.focus.is_focused(window) {
            window.blur(cx);
        }
        state
    }
}

pub(super) fn emit(
    listener: &ChangeListener,
    value: f64,
    next: f64,
    window: &mut Window,
    cx: &mut App,
) {
    if next != value {
        listener(&next, window, cx);
    }
}

pub(super) fn capture_drag(
    dragging: Rc<Cell<bool>>,
    window: &mut Window,
    listener: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
) {
    let listener = Rc::new(listener);
    let moving = dragging.clone();
    let move_listener = listener.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || !moving.get() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            moving.set(false);
            return;
        }
        move_listener(event.position, window, cx);
        cx.stop_propagation();
    });
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Capture
            && event.button == MouseButton::Left
            && dragging.replace(false)
        {
            listener(event.position, window, cx);
        }
    });
}

/// A controlled horizontal slider with pointer, keyboard, and accessibility input.
///
/// Retain the requested value in the parent and render that value on the next frame.
/// Arrow keys step the value. Home and End select the range endpoints.
#[derive(IntoElement)]
pub struct Slider(NumericControl);

impl Slider {
    /// Creates a slider at the range minimum with a stable ID and accessible label.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        range: ValueRange,
    ) -> Self {
        Self(NumericControl::new(id, label, range))
    }

    /// Sets a finite value, clamped and snapped through [`ValueRange::snap`].
    pub fn value(mut self, value: f64) -> Result<Self, ValueRangeError> {
        self.0.value = self.0.range.snap(value)?;
        Ok(self)
    }

    /// Disables all editing and removes the control from keyboard navigation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.0.disabled = disabled;
        self
    }

    /// Uses a caller-owned focus handle.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.0.focus = Some(focus.clone());
        self
    }

    /// Receives each requested change; unchanged values do not emit callbacks.
    pub fn on_change(mut self, listener: impl Fn(&f64, &mut Window, &mut App) + 'static) -> Self {
        self.0.on_change = Some(Rc::new(listener));
        self
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.0.style
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let control = self.0;
        let theme = Theme::for_window(window);
        let Interaction {
            focus,
            dragging,
            bounds,
        } = control.interaction(window, cx);
        let mut element = control
            .base(Role::Slider, &focus, theme)
            .w(px(160.0))
            .rounded(px(6.0))
            .aria_orientation(Orientation::Horizontal);
        element.style().refine(&control.style);
        let range = control.range;
        let value = control.value;
        let enabled = !control.disabled;
        if let Some(listener) = control.on_change.clone().filter(|_| enabled) {
            let dragging = dragging.clone();
            let bounds = bounds.clone();
            element = element.cursor_pointer().on_mouse_down(
                MouseButton::Left,
                move |event, window, cx| {
                    focus.focus(window, cx);
                    dragging.set(true);
                    if let Some(next) = pointer_value(range, event.position.x, bounds.get()) {
                        emit(&listener, value, next, window, cx);
                    }
                    cx.stop_propagation();
                },
            );
        }
        element.child(
            canvas(
                move |actual, _, _| {
                    bounds.set(actual);
                },
                move |bounds, _, window, _| {
                    let track = Bounds::new(
                        point(bounds.left() + px(8.0), bounds.center().y - px(2.0)),
                        size(bounds.size.width - px(16.0), px(4.0)),
                    );
                    window.paint_quad(fill(track, theme.control_border).corner_radii(px(2.0)));
                    let thumb_x = track.left() + track.size.width * range.fraction(value);
                    let filled = Bounds::new(
                        track.origin,
                        size(thumb_x - track.left(), track.size.height),
                    );
                    window.paint_quad(fill(filled, theme.accent).corner_radii(px(2.0)));
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(thumb_x - px(8.0), bounds.center().y - px(8.0)),
                                size(px(16.0), px(16.0)),
                            ),
                            theme.accent,
                        )
                        .corner_radii(px(8.0)),
                    );
                    if let Some(listener) = control.on_change.filter(|_| enabled) {
                        capture_drag(dragging, window, move |position, window, cx| {
                            if let Some(next) = pointer_value(range, position.x, bounds) {
                                emit(&listener, value, next, window, cx);
                            }
                        });
                    }
                },
            )
            .size_full(),
        )
    }
}

fn pointer_value(range: ValueRange, x: Pixels, bounds: Bounds<Pixels>) -> Option<f64> {
    let width = bounds.size.width - px(16.0);
    if width <= px(0.0) {
        return None;
    }
    let fraction = ((x - bounds.left() - px(8.0)) / width).clamp(0.0, 1.0) as f64;
    range
        .snap(range.minimum() + fraction * (range.maximum() - range.minimum()))
        .ok()
}

/// A controlled numeric stepper with bounded decrease and increase buttons.
#[derive(IntoElement)]
pub struct Stepper(NumericControl);

impl Stepper {
    /// Creates a stepper at the range minimum with a stable ID and accessible label.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        range: ValueRange,
    ) -> Self {
        Self(NumericControl::new(id, label, range))
    }

    /// Sets a finite value, clamped and snapped through [`ValueRange::snap`].
    pub fn value(mut self, value: f64) -> Result<Self, ValueRangeError> {
        self.0.value = self.0.range.snap(value)?;
        Ok(self)
    }

    /// Disables all editing and removes the control from keyboard navigation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.0.disabled = disabled;
        self
    }

    /// Uses a caller-owned focus handle.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.0.focus = Some(focus.clone());
        self
    }

    /// Receives each requested change; unchanged values do not emit callbacks.
    pub fn on_change(mut self, listener: impl Fn(&f64, &mut Window, &mut App) + 'static) -> Self {
        self.0.on_change = Some(Rc::new(listener));
        self
    }
}

impl Styled for Stepper {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.0.style
    }
}

impl RenderOnce for Stepper {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let control = self.0;
        let theme = Theme::for_window(window);
        let focus = control.interaction(window, cx).focus;
        let value = control.value;
        let range = control.range;
        let mut base = control
            .base(Role::SpinButton, &focus, theme)
            .flex()
            .items_center()
            .gap(px(8.0))
            .rounded(px(6.0));
        base.style().refine(&control.style);
        for (id, glyph, next) in [
            ("decrease", "−", range.decrement(value)),
            ("increase", "+", range.increment(value)),
        ] {
            if id == "increase" {
                base = base.child(div().min_w(px(32.0)).text_center().child(value.to_string()));
            }
            let listener = control.on_change.clone();
            let focus = focus.clone();
            let label = SharedString::from(format!(
                "{} {}",
                if id == "decrease" {
                    "Decrease"
                } else {
                    "Increase"
                },
                control.label
            ));
            let disabled = control.disabled || next == value || control.on_change.is_none();
            let button = div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .flex()
                .items_center()
                .justify_center()
                .w(px(28.0))
                .h(px(28.0))
                .rounded(px(5.0))
                .bg(theme.control_background)
                .border_1()
                .border_color(theme.control_border)
                .child(glyph)
                .when(disabled, |button| {
                    button
                        .opacity(0.5)
                        .a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
                })
                .when(!disabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(move |style| {
                            style.bg(theme
                                .control_background
                                .blend(theme.foreground.opacity(0.06)))
                        })
                        .active(move |style| {
                            style.bg(theme
                                .control_background
                                .blend(theme.foreground.opacity(0.12)))
                        })
                        .on_click(move |_, window, cx| {
                            focus.focus(window, cx);
                            if let Some(listener) = &listener {
                                emit(listener, value, next, window, cx);
                            }
                        })
                });
            base = base.child(button);
        }
        base
    }
}
