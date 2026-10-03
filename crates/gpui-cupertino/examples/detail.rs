//! Project details with retained drafts and nested modal ownership.

use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, Render, Role, SharedString,
    Subscription, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_cupertino::{
    components::{
        Button, Dialog, Form, FormField, FormSection, Menu, MenuEvent, MenuItem, MenuState,
        ModalHost, PanelState, ScrollArea, Sheet, SheetEdge, TextInput, Toolbar, Tooltip,
        TooltipState,
    },
    theme::Theme,
};

actions!(
    detail_example,
    [
        /// Move focus to the next ordinary detail control.
        NextControl,
        /// Move focus to the previous ordinary detail control.
        PreviousControl,
        /// Close the detail application.
        Quit,
    ]
);

fn init(cx: &mut App) {
    gpui_cupertino::init(cx);
    cx.bind_keys([
        KeyBinding::new("tab", NextControl, Some("DetailShowcase")),
        KeyBinding::new("shift-tab", PreviousControl, Some("DetailShowcase")),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
}

struct Detail {
    saved_name: String,
    saved_summary: String,
    name: Entity<TextInput>,
    summary: Entity<TextInput>,
    sheet: Entity<PanelState>,
    confirmation: Entity<PanelState>,
    keep_editing: FocusHandle,
    menu: Entity<MenuState>,
    hint: Entity<TooltipState>,
    status: SharedString,
    _subscriptions: [Subscription; 4],
}

impl Detail {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| TextInput::new("Project name", window, cx));
        let summary = cx.new(|cx| TextInput::new("Summary", window, cx));
        let sheet = cx.new(|cx| PanelState::new(cx).initial_focus(&name.focus_handle(cx)));
        let keep_editing = cx.focus_handle().tab_stop(true);
        let confirmation = cx.new(|cx| PanelState::new(cx).initial_focus(&keep_editing));
        let edit_focus = sheet.read(cx).trigger_focus_handle().clone();
        let hint = cx.new(|cx| {
            TooltipState::new("Edit the project name and summary", &edit_focus, window, cx)
        });
        let menu = cx.new(|cx| {
            MenuState::new(
                [
                    MenuItem::new("edit", "Edit project…"),
                    MenuItem::new("archive", "Archive project").disabled(true),
                ],
                cx,
            )
        });
        let name_changes = cx.subscribe(&name, |_, _, _, cx| cx.notify());
        let summary_changes = cx.subscribe(&summary, |_, _, _, cx| cx.notify());
        let menu_actions = cx.subscribe_in(
            &menu,
            window,
            |this: &mut Self, _, event: &MenuEvent, window, cx| {
                if event.id == "edit" {
                    this.edit(window, cx);
                }
            },
        );
        let sheet_changes = cx.observe_in(&sheet, window, |this: &mut Self, panel, _, cx| {
            if !panel.read(cx).is_open() {
                this.reset_draft(cx);
                cx.notify();
            }
        });
        let mut detail = Self {
            saved_name: "Orchard".into(),
            saved_summary: "A shared place to plan a calm launch.".into(),
            name,
            summary,
            sheet,
            confirmation,
            keep_editing,
            menu,
            hint,
            status: "Changes are saved in this window.".into(),
            _subscriptions: [name_changes, summary_changes, menu_actions, sheet_changes],
        };
        detail.reset_draft(cx);
        detail
    }

    fn reset_draft(&mut self, cx: &mut Context<Self>) {
        self.name
            .update(cx, |input, cx| input.set_value(&self.saved_name, cx));
        self.summary
            .update(cx, |input, cx| input.set_value(&self.saved_summary, cx));
    }

    fn dirty(&self, cx: &App) -> bool {
        self.name.read(cx).value() != self.saved_name
            || self.summary.read(cx).value() != self.saved_summary
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_draft(cx);
        self.sheet.update(cx, |state, cx| state.open(window, cx));
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.saved_name = self.name.read(cx).value().to_owned();
        self.saved_summary = self.summary.read(cx).value().to_owned();
        self.status = "Project details saved.".into();
        self.sheet.update(cx, |state, cx| state.close(window, cx));
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dirty(cx) {
            self.confirmation
                .update(cx, |state, cx| state.open(window, cx));
        } else {
            self.sheet.update(cx, |state, cx| state.close(window, cx));
        }
    }

    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_draft(cx);
        self.status = "Draft changes discarded.".into();
        self.sheet.update(cx, |state, cx| state.close(window, cx));
        cx.notify();
    }
}

impl Render for Detail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let edit = Button::new("edit", "Edit details")
            .primary()
            .track_focus(self.sheet.read(cx).trigger_focus_handle())
            .aria_expanded(self.sheet.read(cx).is_open())
            .on_click(cx.listener(|this, _, window, cx| this.edit(window, cx)));
        let menu = Button::new("actions", "Actions")
            .track_focus(self.menu.read(cx).trigger_focus_handle(cx))
            .aria_expanded(self.menu.read(cx).is_open(cx))
            .on_click(cx.listener(|this, _, window, cx| {
                this.menu.update(cx, |menu, cx| menu.toggle(window, cx));
            }));
        let background = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_action(|_: &NextControl, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousControl, window, cx| window.focus_prev(cx))
            .child(
                Toolbar::new("toolbar", "Project actions")
                    .child(Tooltip::new(&self.hint, edit))
                    .child(Menu::new(&self.menu, menu, "Project actions menu")),
            )
            .child(
                ScrollArea::new("details", "Project details")
                    .flex_1()
                    .p(px(24.))
                    .child(
                        Form::new("overview", "Saved project")
                            .child(
                                FormSection::new("project", self.saved_name.clone())
                                    .description("Project overview · Design team")
                                    .child(
                                        div()
                                            .id("saved-summary")
                                            .role(Role::Paragraph)
                                            .aria_label(self.saved_summary.clone())
                                            .child(self.saved_summary.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .id("status")
                                    .role(Role::Status)
                                    .aria_label(self.status.clone())
                                    .a11y_synthetic_children({
                                        let status = self.status.clone();
                                        move |builder| {
                                            let node = builder.parent_node();
                                            node.set_value(status.to_string());
                                            node.set_live(gpui::accesskit::Live::Polite);
                                        }
                                    })
                                    .text_color(theme.secondary_foreground)
                                    .child(self.status.clone()),
                            ),
                    ),
            );
        let editor = Form::new("editor", "Edit project details")
            .w(px(360.))
            .child(
                FormSection::new("fields", "Project details")
                    .child(FormField::new("name", "Project name", self.name.clone()))
                    .child(FormField::new("summary", "Summary", self.summary.clone()))
                    .description("Save applies the draft to the project overview."),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("cancel", "Cancel")
                            .track_focus(self.confirmation.read(cx).trigger_focus_handle())
                            .on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx))),
                    )
                    .child(
                        Button::new("save", "Save")
                            .primary()
                            .disabled(self.name.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            );
        let confirmation = div()
            .w(px(330.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Discard draft changes?"),
            )
            .child(
                div()
                    .id("discard-description")
                    .role(Role::Paragraph)
                    .aria_label("Your saved project will stay unchanged.")
                    .child("Your saved project will stay unchanged."),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("keep", "Keep editing")
                            .track_focus(&self.keep_editing)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirmation
                                    .update(cx, |state, cx| state.close(window, cx));
                            })),
                    )
                    .child(
                        Button::new("discard", "Discard changes")
                            .on_click(cx.listener(|this, _, window, cx| this.discard(window, cx))),
                    ),
            );
        div().key_context("DetailShowcase").size_full().child(
            ModalHost::new(
                ModalHost::new(background).sheet(
                    Sheet::new(&self.sheet, "Edit project", editor).edge(SheetEdge::Trailing),
                ),
            )
            .dialog(Dialog::new(
                &self.confirmation,
                "Discard draft changes?",
                confirmation,
            )),
        )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        init(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(760.), px(520.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Cupertino Project Details");
                cx.new(|cx| Detail::new(window, cx))
            },
        )
        .expect("the detail example requires a graphical session");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        AccessibleAction, TestAppContext,
        accesskit::{ActionData, ActionRequest, TreeId},
    };

    #[gpui::test]
    fn edit_save_and_nested_cancel_preserve_the_project_and_restore_focus(cx: &mut TestAppContext) {
        cx.update(init);
        let (detail, cx) = cx.add_window_view(Detail::new);
        let handle = cx.update(|window, _| {
            window.activate_window();
            window.window_handle()
        });
        let platform = cx.test_window(handle);
        platform.activate_accessibility();
        cx.run_until_parked();
        let action = |label: &str, action, data| {
            let target = platform
                .accessibility_tree_update()
                .unwrap()
                .nodes
                .into_iter()
                .find(|(_, node)| {
                    node.label() == Some(label)
                        && matches!(node.role(), Role::Button | Role::TextInput | Role::MenuItem)
                })
                .unwrap_or_else(|| panic!("missing control: {label}"));
            platform.simulate_accessibility_action(ActionRequest {
                action,
                data,
                target_node: target.0,
                target_tree: TreeId::ROOT,
            });
        };
        action("Actions", AccessibleAction::Click, None);
        cx.run_until_parked();
        action("Edit project…", AccessibleAction::Click, None);
        cx.run_until_parked();
        cx.update(|window, cx| {
            let detail = detail.read(cx);
            assert!(detail.sheet.read(cx).is_open());
            assert!(!detail.menu.read(cx).is_open(cx));
            assert!(detail.name.focus_handle(cx).is_focused(window));
        });
        for (label, value) in [
            ("Project name", "Orchard Studio"),
            ("Summary", "Ready for review."),
        ] {
            action(
                label,
                AccessibleAction::SetValue,
                Some(ActionData::Value(value.into())),
            );
            cx.run_until_parked();
        }
        action("Save", AccessibleAction::Click, None);
        cx.run_until_parked();
        cx.update(|window, cx| {
            let detail = detail.read(cx);
            assert_eq!(detail.saved_name, "Orchard Studio");
            assert_eq!(detail.saved_summary, "Ready for review.");
            assert!(!detail.sheet.read(cx).is_open());
            assert!(
                detail
                    .sheet
                    .read(cx)
                    .trigger_focus_handle()
                    .is_focused(window)
            );
        });
        assert!(
            platform
                .accessibility_tree_update()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Status
                    && node.value() == Some("Project details saved.")
                    && node.live() == Some(gpui::accesskit::Live::Polite))
        );
        action("Edit details", AccessibleAction::Click, None);
        cx.run_until_parked();
        action(
            "Project name",
            AccessibleAction::SetValue,
            Some(ActionData::Value("Unsaved draft".into())),
        );
        cx.run_until_parked();
        action("Cancel", AccessibleAction::Click, None);
        cx.run_until_parked();
        assert!(
            !platform
                .accessibility_tree_update()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::TextInput)
        );
        cx.simulate_keystrokes("tab");
        cx.update(|window, cx| assert!(!detail.read(cx).keep_editing.is_focused(window)));
        cx.simulate_keystrokes("tab");
        cx.update(|window, cx| assert!(detail.read(cx).keep_editing.is_focused(window)));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        cx.update(|window, cx| {
            let detail = detail.read(cx);
            assert!(detail.sheet.read(cx).is_open());
            assert!(!detail.confirmation.read(cx).is_open());
            assert_eq!(detail.name.read(cx).value(), "Unsaved draft");
            assert!(
                detail
                    .confirmation
                    .read(cx)
                    .trigger_focus_handle()
                    .is_focused(window)
            );
        });
        action("Cancel", AccessibleAction::Click, None);
        cx.run_until_parked();
        action("Discard changes", AccessibleAction::Click, None);
        cx.run_until_parked();
        cx.update(|window, cx| {
            let detail = detail.read(cx);
            assert!(!detail.sheet.read(cx).is_open());
            assert!(!detail.confirmation.read(cx).is_open());
            assert_eq!(detail.saved_name, "Orchard Studio");
            assert_eq!(detail.name.read(cx).value(), detail.saved_name);
            assert_eq!(detail.summary.read(cx).value(), detail.saved_summary);
            assert!(
                detail
                    .sheet
                    .read(cx)
                    .trigger_focus_handle()
                    .is_focused(window)
            );
        });
    }
}
