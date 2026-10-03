use gpui::{
    AccessibleAction, AnyElement, App, BoxShadow, Context, EntityId, IntoElement, Modifiers,
    Render, Role, SharedString, Window,
    accesskit::{ActionData, CustomAction},
    div,
    prelude::*,
    px, uniform_list,
};

use super::{CollectionEvent, CollectionView, ReorderDirection, SelectionMode};

pub(super) fn action_listener(
    cx: &Context<CollectionView>,
    listener: impl Fn(
        &mut CollectionView,
        Option<&ActionData>,
        &mut Window,
        &mut Context<CollectionView>,
    ) + 'static,
) -> impl FnMut(Option<&ActionData>, &mut Window, &mut App) + 'static {
    let entity = cx.entity();
    move |data, window, cx| entity.update(cx, |view, cx| listener(view, data, window, cx))
}
use crate::theme::Theme;

#[derive(Clone)]
struct RowDrag {
    owner: EntityId,
    source: SharedString,
    label: SharedString,
}

impl Render for RowDrag {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        div()
            .px(px(12.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .bg(theme.accent)
            .text_color(theme.accent_foreground)
            .child(self.label.clone())
    }
}

impl CollectionView {
    fn row(&self, index: usize, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::for_window(window);
        let row = &self.rows[self.visible[index]];
        let id = row.id.clone();
        let selected = self.selection.selected.contains(&id);
        let focused = self.selection.focused.as_ref() == Some(&id);
        let table = !self.columns.is_empty();
        let mut element = div()
            .id(id.clone())
            .role(if table {
                Role::Row
            } else {
                Role::ListBoxOption
            })
            .aria_selected(selected)
            .aria_position_in_set(index + 1)
            .aria_size_of_set(self.visible.len())
            .when(table, |row| row.aria_row_index(index + 1))
            .when(!table, |element| element.aria_label(row.cells[0].clone()))
            .when(focused, |row| row.aria_active_descendant())
            .h(px(32.0))
            .w_full()
            .flex()
            .items_center()
            .flex_shrink_0()
            .text_size(px(13.0))
            .line_height(px(18.0))
            .bg(if selected {
                theme.selection
            } else {
                theme.control_background
            })
            .when(focused && self.focus.is_focused(window), |row| {
                row.border_1().border_color(theme.focus_ring)
            })
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    this.select_index(index, event.modifiers(), window, cx);
                    if event.click_count() == 2 {
                        cx.emit(CollectionEvent::Activated(
                            this.rows[this.visible[index]].id.clone(),
                        ));
                    }
                }),
            )
            .on_a11y_action(
                AccessibleAction::Focus,
                action_listener(cx, move |this, _, window, cx| {
                    this.focus_index(index, window, cx);
                }),
            )
            .on_a11y_action(
                AccessibleAction::ScrollIntoView,
                action_listener(cx, move |this, _, _, cx| {
                    this.scroll
                        .scroll_to_item(index, gpui::ScrollStrategy::Nearest);
                    cx.notify();
                }),
            );
        if self.selection.mode == SelectionMode::Multiple {
            element = element
                .a11y_synthetic_children(|builder| {
                    builder.parent_node().set_custom_actions(vec![CustomAction {
                        id: 0,
                        description: "Toggle selection".into(),
                    }])
                })
                .on_a11y_action(
                    AccessibleAction::CustomAction,
                    action_listener(cx, move |this, data, window, cx| {
                        if matches!(data, Some(ActionData::CustomAction(0))) {
                            this.select_index(index, Modifiers::secondary_key(), window, cx);
                        }
                    }),
                );
        }
        if table {
            for (column, cell) in self.columns.iter().zip(&row.cells).enumerate() {
                let (definition, value) = cell;
                element = element.child(
                    div()
                        .id(("cell", column))
                        .role(Role::Cell)
                        .aria_label(value.clone())
                        .aria_column_index(column)
                        .w(definition.width)
                        .flex_shrink_0()
                        .px(px(8.0))
                        .truncate()
                        .child(value.clone()),
                );
            }
        } else {
            element = element.child(div().px(px(8.0)).truncate().child(row.cells[0].clone()));
        }
        if self.sort.is_none() {
            let payload = RowDrag {
                owner: cx.entity_id(),
                source: id.clone(),
                label: row.cells[0].clone(),
            };
            let view = cx.entity();
            let owner = cx.entity_id();
            element = element
                .on_drag(payload, move |payload, _, window, cx| {
                    view.update(cx, |this, cx| {
                        if !selected {
                            this.select_index(index, Modifiers::none(), window, cx);
                        }
                    });
                    cx.new(|_| payload.clone())
                })
                .can_drop(move |payload, _, _| {
                    payload
                        .downcast_ref::<RowDrag>()
                        .is_some_and(|drag| drag.owner == owner)
                })
                .drag_over::<RowDrag>(move |style, _, _, _| {
                    style.border_t_2().border_color(theme.accent)
                })
                .on_drop(cx.listener(move |this, payload: &RowDrag, _, cx| {
                    this.drop_rows(payload, Some(&id), cx);
                }));
        }
        element.into_any_element()
    }

    fn drop_rows(&mut self, drag: &RowDrag, before: Option<&SharedString>, cx: &mut Context<Self>) {
        if drag.owner != cx.entity_id() || !self.visible_ids().any(|id| *id == drag.source) {
            return;
        }
        let moving = if self.selection.selected.contains(&drag.source) {
            self.visible_ids()
                .filter(|id| self.selection.selected.contains(*id))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            vec![drag.source.clone()]
        };
        self.reorder(&moving, before, cx);
    }

    fn scroll_accessibility(
        &mut self,
        direction: ReorderDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.visible.is_empty() {
            return;
        }
        let current = self
            .visible_ids()
            .position(|id| Some(id) == self.selection.focused.as_ref())
            .unwrap_or(0);
        let page = (self
            .scroll
            .0
            .borrow()
            .base_handle
            .bounds()
            .size
            .height
            .as_f32()
            / 32.0)
            .floor()
            .max(1.0) as usize;
        let index = match direction {
            ReorderDirection::Up => current.saturating_sub(page),
            ReorderDirection::Down => (current + page).min(self.visible.len() - 1),
        };
        self.focus_index(index, window, cx);
    }
}

impl Render for CollectionView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let table = !self.columns.is_empty();
        let multiple = self.selection.mode == SelectionMode::Multiple;
        let reorder = self.sort.is_none();
        let mut root = div()
            .id("collection")
            .role(if table { Role::Grid } else { Role::ListBox })
            .aria_label(self.label.clone())
            .aria_row_count(self.visible.len() + usize::from(table))
            .when(table, |root| root.aria_column_count(self.columns.len()))
            .track_focus(&self.focus)
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.control_background)
            .text_color(theme.foreground)
            .border_1()
            .border_color(theme.control_border)
            .rounded(px(6.0))
            .focus_visible(move |style| {
                style.shadow(vec![
                    BoxShadow::new(px(0.0), px(0.0), theme.focus_ring).spread_radius(px(2.0)),
                ])
            })
            .when(self.focus.is_focused(window), |root| {
                root.key_context("CupertinoCollection")
            })
            .on_action(cx.listener(Self::command))
            .on_a11y_action(
                AccessibleAction::ScrollUp,
                action_listener(cx, |this, _, window, cx| {
                    this.scroll_accessibility(ReorderDirection::Up, window, cx)
                }),
            )
            .on_a11y_action(
                AccessibleAction::ScrollDown,
                action_listener(cx, |this, _, window, cx| {
                    this.scroll_accessibility(ReorderDirection::Down, window, cx)
                }),
            )
            .a11y_synthetic_children(move |builder| {
                let node = builder.parent_node();
                if multiple {
                    node.set_multiselectable();
                }
                if reorder {
                    node.set_custom_actions(vec![
                        CustomAction {
                            id: 1,
                            description: "Move selected rows up".into(),
                        },
                        CustomAction {
                            id: 2,
                            description: "Move selected rows down".into(),
                        },
                    ]);
                }
            });
        if reorder {
            root = root.on_a11y_action(
                AccessibleAction::CustomAction,
                action_listener(cx, |this, data, _, cx| match data {
                    Some(ActionData::CustomAction(1)) => {
                        this.move_selected(ReorderDirection::Up, cx);
                    }
                    Some(ActionData::CustomAction(2)) => {
                        this.move_selected(ReorderDirection::Down, cx);
                    }
                    _ => {}
                }),
            );
        }
        let list = uniform_list(
            "rows",
            self.visible.len(),
            cx.processor(|this, range: std::ops::Range<usize>, window, cx| {
                range
                    .map(|index| this.row(index, window, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .size_full()
        .track_scroll(&self.scroll);
        let mut content = div().h_full().min_h_0().min_w_full().flex().flex_col();
        if table {
            content = content
                .w(self
                    .columns
                    .iter()
                    .map(|column| column.width)
                    .sum::<gpui::Pixels>())
                .child(self.header(window, cx));
        }
        content = content.child(div().min_h_0().flex_1().overflow_hidden().child(list));
        if reorder {
            let owner = cx.entity_id();
            content = content.child(
                div()
                    .id("drop-last")
                    .h(px(8.0))
                    .flex_shrink_0()
                    .aria_label("Move to end")
                    .can_drop(move |payload, _, _| {
                        payload
                            .downcast_ref::<RowDrag>()
                            .is_some_and(|drag| drag.owner == owner)
                    })
                    .drag_over::<RowDrag>(move |style, _, _, _| style.bg(theme.accent))
                    .on_drop(cx.listener(|this, payload: &RowDrag, _, cx| {
                        this.drop_rows(payload, None, cx);
                    })),
            );
        }
        root.child(
            div()
                .id("horizontal-scroll")
                .size_full()
                .min_h_0()
                .overflow_x_scroll()
                .child(content),
        )
    }
}
