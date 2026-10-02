use gpui::{
    App, BoxShadow, ClickEvent, ElementId, FocusHandle, IntoElement, RenderOnce, Role,
    SharedString, StyleRefinement, Window, div, prelude::*, px,
};

use crate::theme::Theme;

type ClickListener = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A labeled button with pointer, keyboard, and accessibility activation.
///
/// Keep the element ID stable across renders. The visible label is also the
/// accessible name. GPUI activates a focused button when Enter or Space is released.
/// Use the application's Tab bindings to move between controls.
///
/// ```
/// use gpui_cupertino::components::Button;
///
/// let save = Button::new("save", "Save")
///     .primary()
///     .on_click(|_, _, _| println!("Save"));
/// ```
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    primary: bool,
    disabled: bool,
    focus_handle: Option<FocusHandle>,
    expanded: Option<bool>,
    on_click: Option<ClickListener>,
    style: StyleRefinement,
}

impl Button {
    /// Creates an ordinary button with a stable ID and a visible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            primary: false,
            disabled: false,
            focus_handle: None,
            expanded: None,
            on_click: None,
            style: StyleRefinement::default(),
        }
    }

    /// Uses the accent background for the primary action.
    ///
    /// This changes appearance only. It does not create a window-wide Enter binding.
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    /// Disables pointer, keyboard, and accessibility activation and removes the tab stop.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Uses a caller-owned focus handle, preserving its tab index.
    ///
    /// Enabled buttons are tab stops. A disabled button does not register the handle.
    pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle.clone());
        self
    }

    /// Reports whether the popup controlled by this button is expanded.
    pub fn aria_expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Sets the activation callback for pointer, keyboard, and accessibility input.
    ///
    /// Use [`gpui::Context::listener`] to access the owning view's state.
    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(listener));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let (background, foreground) = if self.primary {
            (theme.accent, theme.accent_foreground)
        } else {
            (theme.control_background, theme.foreground)
        };
        let mut button = div()
            .id(self.id)
            .role(Role::Button)
            .aria_label(self.label.clone())
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .min_h(px(28.0))
            .px(px(12.0))
            .py(px(4.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(if self.primary {
                background
            } else {
                theme.control_border
            })
            .bg(background)
            .text_color(foreground)
            .text_size(px(13.0))
            .line_height(px(18.0))
            .child(self.label)
            .when_some(self.expanded, |button, expanded| {
                button.aria_expanded(expanded)
            });
        button.style().refine(&self.style);
        if self.disabled {
            button
                .cursor_default()
                .opacity(0.5)
                .a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        } else {
            button
                .tab_index(0)
                .when_some(self.focus_handle, |button, handle| {
                    button.track_focus(&handle.tab_stop(true))
                })
                .cursor_pointer()
                .hover(move |style| style.bg(background.blend(foreground.opacity(0.06))))
                .active(move |style| style.bg(background.blend(foreground.opacity(0.12))))
                .focus_visible(move |style| {
                    style.shadow(vec![
                        BoxShadow::new(px(0.0), px(0.0), theme.focus_ring).spread_radius(px(3.0)),
                    ])
                })
                .when_some(self.on_click, |button, listener| button.on_click(listener))
        }
    }
}
