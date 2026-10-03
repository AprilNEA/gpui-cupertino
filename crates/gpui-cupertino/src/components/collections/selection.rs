use std::collections::{BTreeSet, HashSet};

use gpui::{App, Context, KeyBinding, Modifiers, SharedString, Window};

use super::{CollectionEvent, CollectionView, ReorderDirection, SelectionMode};

#[derive(Clone, PartialEq, gpui::Action)]
#[action(no_json, no_register)]
pub(super) struct Command {
    key: &'static str,
    modifiers: Modifiers,
}

pub(super) fn init(cx: &mut App) {
    let context = Some("CupertinoCollection");
    let primary = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    for key in ["up", "down", "home", "end", "space"] {
        for (prefix, modifiers) in [
            (String::new(), Modifiers::none()),
            (
                "shift-".to_owned(),
                Modifiers {
                    shift: true,
                    ..Modifiers::none()
                },
            ),
            (format!("{primary}-"), Modifiers::secondary_key()),
            (
                format!("{primary}-shift-"),
                Modifiers {
                    shift: true,
                    ..Modifiers::secondary_key()
                },
            ),
        ] {
            cx.bind_keys([KeyBinding::new(
                &format!("{prefix}{key}"),
                Command { key, modifiers },
                context,
            )]);
        }
    }
    for key in ["enter", "escape"] {
        cx.bind_keys([KeyBinding::new(
            key,
            Command {
                key,
                modifiers: Modifiers::none(),
            },
            context,
        )]);
    }
    cx.bind_keys([KeyBinding::new(
        &format!("{primary}-a"),
        Command {
            key: "a",
            modifiers: Modifiers::secondary_key(),
        },
        context,
    )]);
    for key in ["up", "down"] {
        cx.bind_keys([KeyBinding::new(
            &format!("alt-{key}"),
            Command {
                key,
                modifiers: Modifiers::alt(),
            },
            context,
        )]);
    }
}

#[derive(Default)]
pub(super) struct Selection {
    pub mode: SelectionMode,
    pub selected: BTreeSet<SharedString>,
    pub focused: Option<SharedString>,
    pub anchor: Option<SharedString>,
}

impl Selection {
    fn select(&mut self, visible: &[SharedString], index: usize, modifiers: Modifiers) {
        let id = visible[index].clone();
        if self.mode == SelectionMode::Multiple && modifiers.shift {
            let anchor = self
                .anchor
                .as_ref()
                .and_then(|id| visible.iter().position(|item| item == id))
                .or_else(|| {
                    self.focused
                        .as_ref()
                        .and_then(|id| visible.iter().position(|item| item == id))
                })
                .unwrap_or(index);
            self.anchor = Some(visible[anchor].clone());
            if !modifiers.secondary() {
                let visible_ids: HashSet<_> = visible.iter().collect();
                self.selected.retain(|id| !visible_ids.contains(id));
            }
            self.selected.extend(
                visible[anchor.min(index)..=anchor.max(index)]
                    .iter()
                    .cloned(),
            );
        } else if self.mode == SelectionMode::Multiple && modifiers.secondary() {
            if !self.selected.remove(&id) {
                self.selected.insert(id.clone());
            }
            self.anchor = Some(id.clone());
        } else {
            self.selected.clear();
            self.selected.insert(id.clone());
            self.anchor = Some(id.clone());
        }
        self.focused = Some(id);
    }
}

impl CollectionView {
    pub(super) fn select_index(
        &mut self,
        index: usize,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.selection.selected.clone();
        let visible: Vec<_> = self.visible_ids().cloned().collect();
        self.selection.select(&visible, index, modifiers);
        self.focus.focus(window, cx);
        self.scroll_to_focus();
        self.selection_changed(before, cx);
        cx.notify();
    }

    pub(super) fn focus_index(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection.focused = Some(self.rows[self.visible[index]].id.clone());
        self.focus.focus(window, cx);
        self.scroll_to_focus();
        cx.notify();
    }

    pub(super) fn command(&mut self, event: &Command, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus.is_focused(window) || (self.visible.is_empty() && event.key != "escape") {
            return;
        }
        let key = event.key;
        let modifiers = event.modifiers;
        if modifiers.alt
            && !modifiers.shift
            && !modifiers.secondary()
            && matches!(key, "up" | "down")
        {
            self.move_selected(
                if key == "up" {
                    ReorderDirection::Up
                } else {
                    ReorderDirection::Down
                },
                cx,
            );
        } else if modifiers.alt
            || modifiers.function
            || (modifiers.control && cfg!(target_os = "macos"))
            || (modifiers.platform && !cfg!(target_os = "macos"))
        {
            return;
        } else {
            let current = self
                .visible_ids()
                .position(|id| Some(id) == self.selection.focused.as_ref())
                .unwrap_or(0);
            let target = match key {
                "up" => Some(current.saturating_sub(1)),
                "down" => Some((current + 1).min(self.visible.len() - 1)),
                "home" => Some(0),
                "end" => Some(self.visible.len() - 1),
                _ => None,
            };
            if let Some(index) = target {
                if modifiers.secondary() && !modifiers.shift {
                    self.focus_index(index, window, cx);
                } else {
                    self.select_index(index, modifiers, window, cx);
                }
            } else {
                match key {
                    "a" if modifiers.secondary()
                        && !modifiers.shift
                        && self.selection.mode == SelectionMode::Multiple =>
                    {
                        let before = self.selection.selected.clone();
                        self.selection.selected.extend(
                            self.visible
                                .iter()
                                .map(|index| self.rows[*index].id.clone()),
                        );
                        self.selection_changed(before, cx);
                        cx.notify();
                    }
                    "space" => self.select_index(current, modifiers, window, cx),
                    "enter" if !modifiers.modified() => cx.emit(CollectionEvent::Activated(
                        self.rows[self.visible[current]].id.clone(),
                    )),
                    "escape" if !modifiers.modified() => {
                        let before = std::mem::take(&mut self.selection.selected);
                        self.selection.anchor = None;
                        self.selection_changed(before, cx);
                        cx.notify();
                    }
                    _ => return,
                }
            }
        }
        window.prevent_default();
        cx.stop_propagation();
    }
}
