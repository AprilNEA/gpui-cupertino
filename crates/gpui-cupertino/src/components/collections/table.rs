use gpui::{
    AccessibleAction, App, Context, CursorStyle, DispatchPhase, IntoElement, KeyBinding,
    MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, Role, Window,
    accesskit::{ActionData, Orientation, SortDirection as AccessibleSort},
    canvas, div,
    prelude::*,
    px,
};

use super::render::action_listener;
use super::{CollectionEvent, CollectionView, SortDirection};

#[derive(Clone, PartialEq, gpui::Action)]
#[action(no_json, no_register)]
struct ResizeKey(&'static str);

pub(super) fn init(cx: &mut App) {
    cx.bind_keys(
        ["left", "right", "home", "end"].into_iter().map(|key| {
            KeyBinding::new(key, ResizeKey(key), Some("CupertinoCollectionColumnResize"))
        }),
    );
}
use crate::theme::Theme;

impl CollectionView {
    pub(super) fn header(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let entity = cx.entity();
        div()
            .id("header")
            .role(Role::Row)
            .aria_row_index(0)
            .flex()
            .h(px(32.0))
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.control_border)
            .children(self.columns.iter().enumerate().map(|(index, column)| {
                let id = column.id.clone();
                let sort = self
                    .sort
                    .as_ref()
                    .filter(|sort| sort.column == id)
                    .map(|sort| sort.direction);
                let resize_label: gpui::SharedString = format!("Resize {}", column.label).into();
                div()
                    .id(id.clone())
                    .role(Role::ColumnHeader)
                    .aria_label(column.label.clone())
                    .aria_column_index(index)
                    .tab_index(0)
                    .relative()
                    .w(column.width)
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(8.0))
                    .text_size(px(13.0))
                    .cursor_pointer()
                    .focus_visible(move |style| style.bg(theme.selection))
                    .a11y_synthetic_children(move |builder| {
                        if let Some(sort) = sort {
                            builder.parent_node().set_sort_direction(match sort {
                                SortDirection::Ascending => AccessibleSort::Ascending,
                                SortDirection::Descending => AccessibleSort::Descending,
                            });
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_sort(&id, cx)
                            .expect("the rendered column belongs to the table");
                    }))
                    .child(div().truncate().child(format!(
                        "{}{}",
                        column.label,
                        match sort {
                            None => "",
                            Some(SortDirection::Ascending) => " ↑",
                            Some(SortDirection::Descending) => " ↓",
                        }
                    )))
                    .child(
                        div()
                            .id("resize")
                            .role(Role::Splitter)
                            .aria_label(resize_label)
                            .aria_orientation(Orientation::Vertical)
                            .aria_numeric_value(column.width.as_f32() as f64)
                            .aria_min_numeric_value(48.0)
                            .aria_max_numeric_value(640.0)
                            .aria_numeric_value_step(8.0)
                            .tab_index(0)
                            .key_context("CupertinoCollectionColumnResize")
                            .absolute()
                            .right_0()
                            .top_0()
                            .bottom_0()
                            .w(px(6.0))
                            .cursor(CursorStyle::ResizeLeftRight)
                            .border_r_1()
                            .border_color(theme.control_border)
                            .focus_visible(move |style| style.bg(theme.focus_ring))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, window, cx| {
                                        this.resize = Some((
                                            index,
                                            event.position.x,
                                            this.columns[index].width,
                                        ));
                                        window.prevent_default();
                                        cx.stop_propagation();
                                        cx.notify();
                                    },
                                ),
                            )
                            .on_action(cx.listener(move |this, event: &ResizeKey, window, cx| {
                                let width = this.columns[index].width;
                                let target = match event.0 {
                                    "left" => width - px(8.0),
                                    "right" => width + px(8.0),
                                    "home" => px(48.0),
                                    "end" => px(640.0),
                                    _ => return,
                                };
                                this.resize_to(index, target, cx);
                                window.prevent_default();
                                cx.stop_propagation();
                            }))
                            .on_a11y_action(
                                AccessibleAction::Increment,
                                action_listener(cx, move |this, _, _, cx| {
                                    this.resize_to(index, this.columns[index].width + px(8.0), cx)
                                }),
                            )
                            .on_a11y_action(
                                AccessibleAction::Decrement,
                                action_listener(cx, move |this, _, _, cx| {
                                    this.resize_to(index, this.columns[index].width - px(8.0), cx)
                                }),
                            )
                            .on_a11y_action(
                                AccessibleAction::SetValue,
                                action_listener(cx, move |this, data, _, cx| {
                                    if let Some(ActionData::NumericValue(value)) = data
                                        && value.is_finite()
                                    {
                                        this.resize_to(index, px(*value as f32), cx);
                                    }
                                }),
                            ),
                    )
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, (), window, _| {
                        let moving = entity.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            moving.update(cx, |this, cx| {
                                if let Some((index, start, width)) = this.resize {
                                    if event.dragging() {
                                        this.resize_to(index, width + event.position.x - start, cx);
                                    } else {
                                        this.resize = None;
                                    }
                                    cx.stop_propagation();
                                }
                            });
                        });
                        let released = entity.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture || event.button != MouseButton::Left
                            {
                                return;
                            }
                            released.update(cx, |this, cx| {
                                if let Some((index, start, width)) = this.resize.take() {
                                    this.resize_to(index, width + event.position.x - start, cx);
                                    cx.stop_propagation();
                                }
                            });
                        });
                    },
                )
                .size_0(),
            )
    }

    fn resize_to(&mut self, index: usize, width: Pixels, cx: &mut Context<Self>) {
        let width = width.clamp(px(48.0), px(640.0));
        let column = &mut self.columns[index];
        if column.width != width {
            column.width = width;
            cx.emit(CollectionEvent::ColumnResized {
                column: column.id.clone(),
                width,
            });
            cx.notify();
        }
    }
}
