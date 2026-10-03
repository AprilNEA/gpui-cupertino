//! A small settings form that combines Cupertino controls and a focused popover.

use gpui::{
    App, Bounds, Context, Entity, Focusable, KeyBinding, Render, Role, SharedString, Subscription,
    Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_cupertino::{
    components::{
        Button, CheckState, Checkbox, ChoiceOption, Form, FormField, FormSection, Popover,
        PopoverState, Progress, RadioGroup, ScrollArea, SegmentedControl, Slider, Stepper,
        TextInput, TextInputEvent, Toggle, Toolbar, ValueRange,
    },
    theme::Theme,
};

actions!(
    component_showcase,
    [
        /// Move focus to the next enabled control.
        NextField,
        /// Move focus to the previous enabled control.
        PreviousField,
        /// Close the showcase application.
        Quit,
    ]
);

#[derive(Clone, PartialEq)]
struct Preferences {
    notifications: bool,
    backups: bool,
    sync: SharedString,
    density: SharedString,
    volume: f64,
    copies: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            notifications: true,
            backups: false,
            sync: "idle".into(),
            density: "comfortable".into(),
            volume: 50.0,
            copies: 3.0,
        }
    }
}

struct Settings {
    name: Entity<TextInput>,
    note: Entity<TextInput>,
    popover: Entity<PopoverState>,
    saved_name: String,
    saved_note: String,
    preferences: Preferences,
    saved_preferences: Preferences,
    status: SharedString,
    _subscriptions: [Subscription; 2],
}

impl Settings {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| {
            let mut input = TextInput::new("Display name", window, cx).placeholder("Your name");
            input.set_value("Cupertino", cx);
            input
        });
        let note =
            cx.new(|cx| TextInput::new("Profile note", window, cx).placeholder("Add a short note"));
        let popover = cx.new(|cx| PopoverState::new(cx).initial_focus(&note.focus_handle(cx)));
        let name_subscription =
            cx.subscribe_in(&name, window, |this: &mut Self, _, event, _, cx| {
                if matches!(event, TextInputEvent::Submitted(_))
                    && !this.name.read(cx).value().trim().is_empty()
                {
                    this.save(cx);
                }
                cx.notify();
            });
        let note_subscription =
            cx.subscribe_in(&note, window, |this: &mut Self, _, event, window, cx| {
                if matches!(event, TextInputEvent::Submitted(_)) {
                    this.popover
                        .update(cx, |popover, cx| popover.close(window, cx));
                }
                cx.notify();
            });
        window.focus(&name.focus_handle(cx), cx);
        Self {
            name,
            note,
            popover,
            saved_name: "Cupertino".into(),
            saved_note: String::new(),
            preferences: Preferences::default(),
            saved_preferences: Preferences::default(),
            status: "Changes are kept in this window.".into(),
            _subscriptions: [name_subscription, note_subscription],
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.saved_name = self.name.read(cx).value().to_owned();
        self.saved_note = self.note.read(cx).value().to_owned();
        self.saved_preferences = self.preferences.clone();
        self.status = format!("Saved for {}.", self.saved_name).into();
        cx.notify();
    }

    fn preference_controls(&self, cx: &Context<Self>) -> FormSection {
        FormSection::new("behavior", "Workspace behavior")
            .description("Save changes to keep these preferences in this window.")
            .child(
                Toggle::new(
                    "notifications",
                    "Notifications",
                    self.preferences.notifications,
                )
                .on_change(cx.listener(|this, value: &bool, _, cx| {
                    this.preferences.notifications = *value;
                    cx.notify();
                })),
            )
            .child(
                Checkbox::new("backups", "Keep local backups", self.preferences.backups).on_change(
                    cx.listener(|this, value: &CheckState, _, cx| {
                        this.preferences.backups = *value == CheckState::Checked;
                        cx.notify();
                    }),
                ),
            )
            .child(FormField::new(
                "sync-field",
                "Synchronization",
                RadioGroup::new(
                    "sync",
                    "Synchronization",
                    [
                        ChoiceOption::new("manual", "Manually"),
                        ChoiceOption::new("idle", "When idle"),
                        ChoiceOption::new("always", "Continuously"),
                    ],
                )
                .selected(self.preferences.sync.clone())
                .on_change(cx.listener(|this, value: &SharedString, _, cx| {
                    this.preferences.sync = value.clone();
                    cx.notify();
                })),
            ))
            .child(FormField::new(
                "density-field",
                "Content density",
                SegmentedControl::new(
                    "density",
                    "Content density",
                    [
                        ChoiceOption::new("comfortable", "Comfortable"),
                        ChoiceOption::new("compact", "Compact"),
                    ],
                )
                .selected(self.preferences.density.clone())
                .on_change(cx.listener(|this, value: &SharedString, _, cx| {
                    this.preferences.density = value.clone();
                    cx.notify();
                })),
            ))
            .child(FormField::new(
                "volume-field",
                "Alert volume",
                Slider::new(
                    "volume",
                    "Alert volume",
                    ValueRange::new(0., 100., 5.).expect("constant volume range is valid"),
                )
                .value(self.preferences.volume)
                .expect("slider changes preserve the finite range")
                .on_change(cx.listener(|this, value: &f64, _, cx| {
                    this.preferences.volume = *value;
                    cx.notify();
                })),
            ))
            .child(FormField::new(
                "copies-field",
                "Backup copies",
                Stepper::new(
                    "copies",
                    "Backup copies",
                    ValueRange::new(1., 10., 1.).expect("constant copy range is valid"),
                )
                .value(self.preferences.copies)
                .expect("stepper changes preserve the finite range")
                .disabled(!self.preferences.backups)
                .on_change(cx.listener(|this, value: &f64, _, cx| {
                    this.preferences.copies = *value;
                    cx.notify();
                })),
            ))
    }
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let dirty = self.name.read(cx).value() != self.saved_name
            || self.note.read(cx).value() != self.saved_note
            || self.preferences != self.saved_preferences;
        let empty_name = self.name.read(cx).value().trim().is_empty();
        let popover = self.popover.read(cx);
        let trigger = Button::new("edit-note", "Edit note…")
            .track_focus(popover.trigger_focus_handle())
            .aria_expanded(popover.is_open())
            .on_click(cx.listener(|this, _, window, cx| {
                this.popover
                    .update(cx, |popover, cx| popover.toggle(window, cx));
            }));
        let panel = div()
            .w(px(300.0))
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .bg(theme.control_background)
            .text_color(theme.foreground)
            .border_1()
            .border_color(theme.control_border)
            .rounded(px(10.0))
            .child("Profile note")
            .child(self.note.clone())
            .child(
                Button::new("done", "Done").on_click(cx.listener(|this, _, window, cx| {
                    this.popover
                        .update(cx, |popover, cx| popover.close(window, cx));
                })),
            );
        div()
            .key_context("ComponentShowcase")
            .size_full()
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(24.0))
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(px(14.0))
            .on_action(|_: &NextField, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousField, window, cx| window.focus_prev(cx))
            .child(div().text_size(px(24.0)).child("Workspace preferences"))
            .child(
                ScrollArea::new("settings-scroll", "Workspace preferences")
                    .flex_1()
                    .child(
                        Form::new("settings", "Workspace preferences")
                            .child(
                                FormSection::new("profile", "Profile")
                                    .child(
                                        FormField::new(
                                            "name-field",
                                            "Display name",
                                            self.name.clone(),
                                        )
                                        .description("Use a name that others can recognize.")
                                        .when(empty_name, |field| {
                                            field.error("Enter a display name.")
                                        }),
                                    )
                                    .child(Popover::new(
                                        &self.popover,
                                        trigger,
                                        "Edit profile note",
                                        panel,
                                    ))
                                    .child(
                                        Progress::new("profile-progress", "Profile completion")
                                            .value(if empty_name {
                                                0.0
                                            } else if self.note.read(cx).value().is_empty() {
                                                0.5
                                            } else {
                                                1.0
                                            })
                                            .expect("profile completion uses fixed fractions"),
                                    ),
                            )
                            .child(self.preference_controls(cx)),
                    ),
            )
            .child(
                Toolbar::new("settings-actions", "Settings actions")
                    .child(
                        Button::new("save", "Save changes")
                            .primary()
                            .disabled(!dirty || empty_name)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    )
                    .child(
                        Button::new("revert", "Revert")
                            .disabled(!dirty)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.name
                                    .update(cx, |input, cx| input.set_value(&this.saved_name, cx));
                                this.note
                                    .update(cx, |input, cx| input.set_value(&this.saved_note, cx));
                                this.preferences = this.saved_preferences.clone();
                                this.status = "Changes reverted.".into();
                                cx.notify();
                            })),
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
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        gpui_cupertino::init(cx);
        cx.bind_keys([
            KeyBinding::new("tab", NextField, Some("ComponentShowcase")),
            KeyBinding::new("shift-tab", PreviousField, Some("ComponentShowcase")),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(640.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Cupertino Components");
                cx.new(|cx| Settings::new(window, cx))
            },
        )
        .expect("the component showcase requires a graphical session");
        cx.activate(true);
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        AccessibleAction, KeyUpEvent, Keystroke, TestAppContext,
        accesskit::{ActionData, ActionRequest, TreeId},
    };

    #[gpui::test]
    fn note_submission_save_and_revert_use_the_showcase_entities(cx: &mut TestAppContext) {
        cx.update(gpui_cupertino::init);
        let (settings, cx) = cx.add_window_view(Settings::new);
        let handle = cx.update(|window, _| window.window_handle());
        let platform = cx.test_window(handle);
        platform.activate_accessibility();
        cx.run_until_parked();

        let node = |label: &str| {
            platform
                .accessibility_tree_update()
                .unwrap()
                .nodes
                .into_iter()
                .find(|(_, node)| {
                    node.label() == Some(label)
                        && matches!(node.role(), Role::Button | Role::TextInput | Role::Slider)
                })
                .unwrap()
        };
        let action = |label: &str, action, data| {
            platform.simulate_accessibility_action(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: node(label).0,
                data,
            });
        };
        let assert_dirty = |dirty: bool| {
            for label in ["Save changes", "Revert"] {
                assert_eq!(node(label).1.is_disabled(), !dirty, "{label}");
            }
        };

        assert_dirty(false);
        action("Edit note…", AccessibleAction::Click, None);
        cx.run_until_parked();
        cx.update(|window, cx| {
            let settings = settings.read(cx);
            assert!(settings.popover.read(cx).is_open());
            assert!(settings.note.focus_handle(cx).is_focused(window));
        });

        cx.simulate_input("Ready");
        cx.simulate_keystrokes("enter");
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        });
        cx.update(|window, cx| {
            let settings = settings.read(cx);
            let popover = settings.popover.read(cx);
            assert!(!popover.is_open());
            assert!(popover.trigger_focus_handle().is_focused(window));
            assert_eq!(settings.note.read(cx).value(), "Ready");
            assert!(settings.saved_note.is_empty());
        });
        assert_dirty(true);

        action(
            "Display name",
            AccessibleAction::SetValue,
            Some(ActionData::Value("Ada".into())),
        );
        cx.run_until_parked();
        action("Save changes", AccessibleAction::Click, None);
        cx.run_until_parked();
        settings.read_with(cx, |settings, cx| {
            assert_eq!(settings.saved_name, "Ada");
            assert_eq!(settings.saved_note, "Ready");
            assert_eq!(settings.name.read(cx).value(), settings.saved_name);
            assert_eq!(settings.note.read(cx).value(), settings.saved_note);
            assert_eq!(settings.status, "Saved for Ada.");
        });
        assert_dirty(false);

        action(
            "Display name",
            AccessibleAction::SetValue,
            Some(ActionData::Value("Draft".into())),
        );
        cx.run_until_parked();
        action("Edit note…", AccessibleAction::Click, None);
        cx.run_until_parked();
        cx.simulate_input("!");
        action("Done", AccessibleAction::Click, None);
        cx.run_until_parked();
        settings.read_with(cx, |settings, cx| {
            assert_eq!(settings.name.read(cx).value(), "Draft");
            assert_eq!(settings.note.read(cx).value(), "Ready!");
        });
        assert_dirty(true);

        action("Revert", AccessibleAction::Click, None);
        cx.run_until_parked();
        settings.read_with(cx, |settings, cx| {
            assert_eq!(settings.name.read(cx).value(), "Ada");
            assert_eq!(settings.note.read(cx).value(), "Ready");
            assert_eq!(settings.status, "Changes reverted.");
        });
        assert_dirty(false);

        action(
            "Alert volume",
            AccessibleAction::SetValue,
            Some(ActionData::NumericValue(80.0)),
        );
        cx.run_until_parked();
        assert_dirty(true);
        action("Save changes", AccessibleAction::Click, None);
        cx.run_until_parked();
        assert_dirty(false);
        action(
            "Alert volume",
            AccessibleAction::SetValue,
            Some(ActionData::NumericValue(20.0)),
        );
        cx.run_until_parked();
        assert_dirty(true);
        action("Revert", AccessibleAction::Click, None);
        cx.run_until_parked();
        settings.read_with(cx, |settings, _| {
            assert_eq!(settings.preferences.volume, 80.0);
            assert_eq!(settings.saved_preferences.volume, 80.0);
        });
        assert!(
            platform
                .accessibility_tree_update()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Status
                    && node.value() == Some("Changes reverted.")
                    && node.live() == Some(gpui::accesskit::Live::Polite))
        );
        assert_dirty(false);
    }
}
