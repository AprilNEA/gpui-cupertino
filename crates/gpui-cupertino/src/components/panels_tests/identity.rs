use super::*;

struct SiblingPanels {
    panels: [Entity<PanelState>; 2],
    clicks: [usize; 2],
}

impl Render for SiblingPanels {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panels = self
            .panels
            .iter()
            .enumerate()
            .map(|(index, state)| {
                let trigger = Button::new("trigger", format!("Trigger {index}"))
                    .track_focus(state.read(cx).trigger_focus_handle());
                let action = Button::new("action", format!("Action {index}"))
                    .on_click(cx.listener(move |view, _, _, _| view.clicks[index] += 1));
                Popover::new(
                    state,
                    trigger,
                    format!("Panel {index}"),
                    div().w(px(120.)).h(px(60.)).child(action),
                )
            })
            .collect::<Vec<_>>();
        div().flex().gap(px(100.)).children(panels)
    }
}

#[gpui::test]
fn sibling_panels_keep_distinct_child_identity_and_accessibility(cx: &mut TestAppContext) {
    cx.update(super::super::init);
    let (view, cx) = cx.add_window_view(|_, cx| SiblingPanels {
        panels: std::array::from_fn(|_| cx.new(|cx| PanelState::new(cx))),
        clicks: [0; 2],
    });
    let panels = view.read_with(cx, |view, _| view.panels.clone());
    cx.update(|window, cx| {
        for panel in &panels {
            panel.update(cx, |state, cx| state.open(window, cx));
        }
    });
    let update = tree(cx);
    let ids = [
        "Trigger 0",
        "Trigger 1",
        "Panel 0",
        "Panel 1",
        "Action 0",
        "Action 1",
    ]
    .map(|label| {
        update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap_or_else(|| panic!("missing accessible node: {label}"))
            .0
    });
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[2], ids[3]);
    assert_ne!(ids[4], ids[5]);
    let handle = cx.update(|window, _| window.window_handle());
    cx.test_window(handle)
        .simulate_accessibility_action(gpui::accesskit::ActionRequest {
            action: gpui::AccessibleAction::Click,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: ids[5],
            data: None,
        });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.clicks), [0, 1]);
    assert!(
        panels
            .iter()
            .all(|panel| panel.read_with(cx, |state, _| state.is_open()))
    );
}
