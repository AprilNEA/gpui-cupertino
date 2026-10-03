use super::*;

struct NestedModal {
    outer: Entity<PanelState>,
    inner: Entity<PanelState>,
    first: FocusHandle,
}
impl Render for NestedModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = Button::new("outer-trigger", "Outer trigger")
            .track_focus(self.outer.read(cx).trigger_focus_handle());
        let lower_content = div()
            .w(px(260.))
            .h(px(160.))
            .child(Button::new("first", "Initial field").track_focus(&self.first))
            .child(
                Button::new("inner-trigger", "Inner trigger")
                    .track_focus(self.inner.read(cx).trigger_focus_handle()),
            );
        let nested = ModalHost::new(lower_content).sheet(Sheet::new(
            &self.inner,
            "Child sheet",
            Button::new("done", "Done"),
        ));
        ModalHost::new(background).dialog(Dialog::new(&self.outer, "Parent dialog", nested))
    }
}

#[gpui::test]
fn nested_modal_restores_the_actual_trigger_after_rebuilding_the_lower_tree(
    cx: &mut TestAppContext,
) {
    cx.update(super::super::init);
    let (view, cx) = cx.add_window_view(|_, cx| {
        let first = cx.focus_handle().tab_stop(true);
        NestedModal {
            outer: cx.new(|cx| PanelState::new(cx).initial_focus(&first)),
            inner: cx.new(|cx| PanelState::new(cx)),
            first,
        }
    });
    let (outer, inner) = view.read_with(cx, |view, _| (view.outer.clone(), view.inner.clone()));
    cx.update(|window, cx| outer.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        inner
            .read(cx)
            .trigger_focus_handle()
            .clone()
            .focus(window, cx);
        inner.update(cx, |state, cx| state.open(window, cx));
    });
    cx.run_until_parked();
    let update = tree(cx);
    assert!(
        update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Child sheet"))
    );
    assert!(
        !update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Initial field"))
    );
    press(cx, "escape");
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(outer.read(cx).is_open());
        assert!(!inner.read(cx).is_open());
        assert!(inner.read(cx).trigger_focus_handle().is_focused(window));
    });
    press(cx, "escape");
    cx.update(|window, cx| assert!(outer.read(cx).trigger_focus_handle().is_focused(window)));
}

struct ModalHint {
    panel: Entity<PanelState>,
    hint: Entity<TooltipState>,
    trigger: FocusHandle,
}
impl Render for ModalHint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let background = div()
            .id("hint-target")
            .debug_selector(|| "hint-target".into())
            .w(px(100.))
            .h(px(30.))
            .child(Tooltip::new(
                &self.hint,
                Button::new("help", "Help").track_focus(&self.trigger),
            ));
        ModalHost::new(background).dialog(Dialog::new(
            &self.panel,
            "Modal",
            Button::new("done", "Done"),
        ))
    }
}

#[gpui::test]
fn an_inert_trigger_cannot_paint_a_floating_hint_above_the_modal(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let trigger = cx.focus_handle().tab_stop(true);
        ModalHint {
            panel: cx.new(|cx| PanelState::new(cx)),
            hint: cx.new(|cx| TooltipState::new("Background hint", &trigger, window, cx)),
            trigger,
        }
    });
    let target = cx.debug_bounds("hint-target").unwrap().center();
    cx.simulate_mouse_move(target, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip-hint").is_some());
    let panel = view.read_with(cx, |view, _| view.panel.clone());
    cx.update(|window, cx| panel.update(cx, |state, cx| state.open(window, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("modal-panel").is_some());
    assert!(cx.debug_bounds("tooltip-hint").is_none());
}
