use std::ops::Range;

use gpui::{
    AccessibleAction, App, Bounds, Context, CursorStyle, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render,
    Role, ShapedLine, SharedString, Subscription, TextInputAction, TextInputConfiguration,
    TextStyle, UTF16Selection, Window, accesskit::ActionData, div, point, prelude::*, px,
};

use crate::theme::Theme;

mod bindings;
mod editing;
mod paint;

pub(crate) use bindings::init;
use editing::Editing;

/// A value change or explicit submission from a single-line input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextInputEvent {
    /// User editing changed the value, including updates during IME composition.
    Changed(SharedString),
    /// The user pressed Enter outside IME composition.
    Submitted(SharedString),
}

/// A retained single-line input with Unicode editing and GPUI platform IME support.
///
/// Call [`crate::init`] once before creating windows. Retain an `Entity<TextInput>`
/// and render the entity as a child. The label supplies the accessible name.
/// Line separators and tabs become spaces. Undo retains the last 100 user edits.
pub struct TextInput {
    focus_handle: FocusHandle,
    label: SharedString,
    placeholder: SharedString,
    editing: Editing,
    disabled: bool,
    read_only: bool,
    selecting: bool,
    last_line: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    last_style: Option<TextStyle>,
    scroll_x: Pixels,
    _focus_subscriptions: [Subscription; 2],
}

impl TextInput {
    /// Create an empty input with a persistent focus handle and accessible label.
    pub fn new(
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);
        let on_focus = cx.on_focus(&focus_handle, window, |_, _, cx| cx.notify());
        let on_blur = cx.on_blur(&focus_handle, window, |input, _, cx| {
            input.editing.unmark();
            input.selecting = false;
            cx.notify();
        });
        Self {
            focus_handle,
            label: label.into(),
            placeholder: SharedString::default(),
            editing: Editing::default(),
            disabled: false,
            read_only: false,
            selecting: false,
            last_line: None,
            last_bounds: None,
            last_style: None,
            scroll_x: px(0.0),
            _focus_subscriptions: [on_focus, on_blur],
        }
    }

    /// Set the hint displayed while the value is empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Read the current value, including any active IME composition.
    pub fn value(&self) -> &str {
        &self.editing.text
    }

    /// Replace the value and clear selection, composition, and undo history.
    ///
    /// The caret moves to the end. Programmatic changes do not emit `Changed`.
    pub fn set_value(&mut self, value: impl AsRef<str>, cx: &mut Context<Self>) {
        self.editing = Editing::new(value.as_ref());
        cx.notify();
    }

    /// Disable editing and focus. Disabling the focused input removes its focus.
    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.editing.unmark();
            self.selecting = false;
            if self.focus_handle.is_focused(window) {
                window.blur(cx);
            }
        }
        cx.notify();
    }

    /// Prevent edits while preserving focus, selection, copying, and submission.
    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        self.editing.unmark();
        cx.notify();
    }

    fn editable(&self) -> bool {
        !self.disabled && !self.read_only
    }

    fn edit(&mut self, edit: impl FnOnce(&mut Editing), cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        let before = self.editing.text.clone();
        edit(&mut self.editing);
        if self.editing.text != before {
            cx.emit(TextInputEvent::Changed(self.editing.text.clone()));
        }
        cx.notify();
    }

    fn move_cursor(&mut self, right: bool, extend: bool, cx: &mut Context<Self>) {
        let offset = match (right, extend || self.editing.selection.is_empty()) {
            (false, false) => self.editing.selection.start,
            (true, false) => self.editing.selection.end,
            (false, true) => self.editing.previous(self.editing.cursor()),
            (true, true) => self.editing.next(self.editing.cursor()),
        };
        self.editing.select_to(offset, extend);
        cx.notify();
    }

    fn move_to_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let offset = if end { self.editing.text.len() } else { 0 };
        self.editing.select_to(offset, extend);
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        self.editing.select_to(0, false);
        self.editing.select_to(self.editing.text.len(), true);
        cx.notify();
    }

    fn delete(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.edit(
            |editing| {
                if editing.selection.is_empty() {
                    let next = if forward {
                        editing.next(editing.cursor())
                    } else {
                        editing.previous(editing.cursor())
                    };
                    editing.select_to(next, true);
                }
                editing.replace(None, "");
            },
            cx,
        );
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(offset) = self.index_for_point(event.position, window) else {
            return;
        };
        window.focus(&self.focus_handle, cx);
        self.selecting = true;
        if event.click_count >= 3 {
            self.select_all(cx);
        } else if event.click_count == 2 {
            let bounds = self
                .last_bounds
                .expect("point lookup requires rendered bounds");
            // Word selection uses the hit character, not the nearest insertion boundary.
            let offset = self
                .last_line
                .as_ref()
                .expect("point lookup shapes the text")
                .index_for_x(event.position.x - bounds.left() + self.scroll_x)
                .unwrap_or(self.editing.text.len());
            let word = self.editing.word_at(offset);
            self.editing.select_to(word.start, false);
            self.editing.select_to(word.end, true);
        } else {
            self.editing.select_to(offset, event.modifiers.shift);
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn index_for_point(&mut self, point: Point<Pixels>, window: &mut Window) -> Option<usize> {
        paint::refresh_layout(self, window)?;
        if self.editing.text.is_empty() {
            return Some(0);
        }
        let line = self.last_line.as_ref()?;
        let bounds = self.last_bounds?;
        Some(line.closest_index_for_x(point.x - bounds.left() + self.scroll_x))
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.selecting
            && event.pressed_button == Some(MouseButton::Left)
            && let Some(mut offset) = self.index_for_point(event.position, window)
        {
            let bounds = self
                .last_bounds
                .expect("point lookup requires rendered bounds");
            if event.position.y < bounds.top() {
                offset = 0;
            } else if event.position.y > bounds.bottom() {
                offset = self.editing.text.len();
            }
            self.editing.select_to(offset, true);
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.editing.byte_range(range);
        *adjusted_range = Some(self.editing.to_utf16(range.clone()));
        Some(self.editing.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        (ignore_disabled_input || self.editable()).then(|| UTF16Selection {
            range: self.editing.to_utf16(self.editing.selection.clone()),
            reversed: self.editing.reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.editing
            .marked
            .clone()
            .map(|range| self.editing.to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.editing.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(|editing| editing.replace(range, text), cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(|editing| editing.mark(range, text, selection), cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        paint::refresh_layout(self, window)?;
        let bounds = self.last_bounds?;
        let line = self.last_line.as_ref()?;
        let range = self.editing.byte_range(range);
        let left = bounds.left() - self.scroll_x;
        let start = (left + line.x_for_index(range.start)).clamp(bounds.left(), bounds.right());
        let end = (left + line.x_for_index(range.end)).clamp(bounds.left(), bounds.right());
        Some(Bounds::from_corners(
            point(start.min(end), bounds.top()),
            point(start.max(end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        if !self.last_bounds?.contains(&point) {
            return None;
        }
        let index = self.index_for_point(point, window)?;
        Some(self.editing.to_utf16(index..index).start)
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.editing.byte_range(range);
        self.editing.select_to(range.start, false);
        self.editing.select_to(range.end, true);
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.editing.text.encode_utf16().count())
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.editable()
    }

    fn text_input_configuration(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> TextInputConfiguration {
        TextInputConfiguration {
            input_action: TextInputAction::Done,
            ..Default::default()
        }
    }

    fn text_input_editable_range(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.editable()
            .then(|| 0..self.editing.text.encode_utf16().count())
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let disabled = self.disabled;
        let read_only = self.read_only;
        let set_value = cx.listener(|input, value: &str, window, cx| {
            input.replace_text_in_range(
                Some(0..input.editing.text.encode_utf16().count()),
                value,
                window,
                cx,
            );
        });
        div()
            .id("input")
            .w_full()
            .min_w(px(40.0))
            .h(px(30.0))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_size(px(13.0))
            .line_height(px(20.0))
            .text_color(theme.foreground)
            .bg(theme.control_background)
            .border_1()
            .border_color(theme.control_border)
            .rounded(px(6.0))
            .role(Role::TextInput)
            .aria_label(self.label.clone())
            .aria_value(self.editing.text.clone())
            .aria_placeholder(self.placeholder.clone())
            .a11y_synthetic_children(move |builder| {
                if disabled {
                    builder.parent_node().set_disabled();
                }
                if read_only {
                    builder.parent_node().set_read_only();
                }
            })
            .when(disabled, |field| field.opacity(0.45))
            .when(!disabled, |field| {
                bindings::handlers(field, cx)
                    .track_focus(&self.focus_handle)
                    .key_context("CupertinoTextInput")
                    .cursor(CursorStyle::IBeam)
                    .focus(|style| style.border_color(theme.focus_ring))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|input, _, _, _| input.selecting = false),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|input, _, _, _| input.selecting = false),
                    )
            })
            .when(self.editable(), |field| {
                field.on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                    if let Some(ActionData::Value(value)) = data {
                        set_value(value, window, cx);
                    }
                })
            })
            .child(paint::element(cx.entity()))
    }
}
