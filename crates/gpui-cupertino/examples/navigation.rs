//! Workspace navigation with retained tabs, resizable panes, and loading states.

use gpui::{
    App, Bounds, Context, Entity, KeyBinding, Render, SharedString, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_cupertino::{
    components::{
        Button, ChoiceOption, EmptyState, Form, FormField, Progress, Sidebar, SplitView, Tab, Tabs,
        TextInput, Toggle, Toolbar, ValueRange,
    },
    theme::Theme,
};

actions!(
    navigation_showcase,
    [
        /// Focus the next control.
        Next,
        /// Focus the previous control.
        Previous,
        /// Close the showcase.
        Quit,
    ]
);

struct Workspace {
    section: SharedString,
    tab: SharedString,
    name: Entity<TextInput>,
    ratio: f64,
    stacked: bool,
    downloading: bool,
}

impl Workspace {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            section: "workspace".into(),
            tab: "overview".into(),
            name: cx.new(|cx| {
                let mut name = TextInput::new("Workspace name", window, cx);
                name.set_value("Orchard", cx);
                name
            }),
            ratio: 0.3,
            stacked: false,
            downloading: true,
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let sidebar = Sidebar::new(
            "sidebar",
            "Workspace sections",
            [
                ChoiceOption::new("workspace", "Workspace"),
                ChoiceOption::new("shared", "Shared with me").disabled(true),
                ChoiceOption::new("archive", "Archive"),
            ],
        )
        .selected(self.section.clone())
        .p(px(12.))
        .size_full()
        .bg(theme.control_background)
        .on_change(cx.listener(|this, selected: &SharedString, _, cx| {
            this.section = selected.clone();
            cx.notify();
        }));
        let content = if self.section == "workspace" {
            Tabs::new(
                "workspace-tabs",
                "Workspace details",
                self.tab.clone(),
                [
                    Tab::new(
                        "overview",
                        "Overview",
                        Form::new("overview", "Workspace overview").child(FormField::new(
                            "name",
                            "Workspace name",
                            self.name.clone(),
                        )),
                    ),
                    Tab::new("sharing", "Sharing", div()).disabled(true),
                    Tab::new(
                        "activity",
                        "Activity",
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(20.))
                            .child(
                                Toggle::new("download", "Download in progress", self.downloading)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        this.downloading = *value;
                                        cx.notify();
                                    })),
                            )
                            .when(self.downloading, |content| {
                                content.child(
                                    Progress::new("download-progress", "Downloading workspace")
                                        .w_full(),
                                )
                            }),
                    ),
                ],
            )
            .size_full()
            .on_change(cx.listener(|this, selected: &SharedString, _, cx| {
                this.tab = selected.clone();
                cx.notify();
            }))
            .into_any_element()
        } else {
            EmptyState::new(
                "archive-empty",
                "No archived projects",
                "Projects you archive will appear here.",
            )
            .size_full()
            .child(
                Button::new("return", "Return to workspace").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.section = "workspace".into();
                        cx.notify();
                    },
                )),
            )
            .into_any_element()
        };
        let split = SplitView::new(
            "workspace-split",
            "Workspace pane size",
            sidebar,
            div().size_full().p(px(16.)).child(content),
        )
        .range(ValueRange::new(0., 1., 0.05).expect("constant split range is valid"))
        .expect("split bounds are fractions")
        .ratio(self.ratio)
        .expect("split callbacks preserve the finite ratio")
        .when(self.stacked, SplitView::vertical)
        .on_change(cx.listener(|this, ratio: &f64, _, cx| {
            this.ratio = *ratio;
            cx.notify();
        }));
        div()
            .key_context("NavigationShowcase")
            .size_full()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_action(|_: &Next, window, cx| window.focus_next(cx))
            .on_action(|_: &Previous, window, cx| window.focus_prev(cx))
            .child(
                Toolbar::new("layout", "Workspace layout")
                    .child(
                        Toggle::new("stacked", "Stack panes", self.stacked).on_change(cx.listener(
                            |this, value: &bool, _, cx| {
                                this.stacked = *value;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        Button::new("restore", "Restore pane sizes").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.ratio = 0.3;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(div().flex_1().min_h_0().child(split))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        gpui_cupertino::init(cx);
        cx.bind_keys([
            KeyBinding::new("tab", Next, Some("NavigationShowcase")),
            KeyBinding::new("shift-tab", Previous, Some("NavigationShowcase")),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(860.), px(560.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Cupertino Workspace");
                cx.new(|cx| Workspace::new(window, cx))
            },
        )
        .expect("the navigation example requires a graphical session");
        cx.activate(true);
    });
}
