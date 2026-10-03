// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    macos::run()
}

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
mod macos {
    use accesskit::{Action, ActionHandler, ActionRequest, ActivationHandler, Node, NodeId, Role};
    use anyhow::{Context as _, Result, ensure};
    use gpui::{Bounds, Empty, Platform, WindowHandle, WindowKind, WindowParams, point, px, size};
    use gpui_macos::MacPlatform;
    use objc2::{MainThreadMarker, msg_send, rc::Retained, runtime::AnyObject};
    use objc2_app_kit::{NSApplication, NSView};
    use raw_window_handle::RawWindowHandle;
    use std::ptr;

    const ROOT: NodeId = NodeId(0);
    const INPUT: NodeId = NodeId(1);

    struct Handlers;

    impl ActivationHandler for Handlers {
        fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
            Some(tree(INPUT))
        }
    }

    impl ActionHandler for Handlers {
        fn do_action(&mut self, action: ActionRequest) {
            panic!("A focus query must not dispatch an action: {action:?}");
        }
    }

    fn tree(focus: NodeId) -> accesskit::TreeUpdate {
        let mut root = Node::new(Role::Window);
        root.set_children([INPUT]);
        let mut input = Node::new(Role::TextInput);
        input.set_label("Focus regression");
        input.add_action(Action::Focus);
        accesskit::TreeUpdate {
            nodes: vec![(ROOT, root), (INPUT, input)],
            tree: Some(accesskit::Tree::new(ROOT)),
            tree_id: accesskit::TreeId::ROOT,
            focus,
        }
    }

    fn focus(object: &AnyObject) -> Option<Retained<AnyObject>> {
        // SAFETY: Each caller supplies an AppKit responder or an AccessKit node.
        unsafe { msg_send![object, accessibilityFocusedUIElement] }
    }

    fn same_object(
        left: &Option<Retained<AnyObject>>,
        right: &Option<Retained<AnyObject>>,
    ) -> bool {
        left.as_deref().map(ptr::from_ref) == right.as_deref().map(ptr::from_ref)
    }

    pub fn run() -> Result<()> {
        let marker =
            MainThreadMarker::new().context("Run the native harness on the AppKit main thread")?;
        let _application = NSApplication::sharedApplication(marker);
        let platform = MacPlatform::new(true);
        for (kind, class_name) in [
            (WindowKind::Normal, "GPUIWindow"),
            (WindowKind::Floating, "GPUIPanel"),
        ] {
            let window = platform.open_window(
                WindowHandle::<Empty>::new(1.into()).into(),
                WindowParams {
                    bounds: Bounds::new(point(px(0.), px(0.)), size(px(200.), px(100.))),
                    titlebar: None,
                    kind,
                    is_movable: false,
                    app_owns_titlebar_drag: false,
                    is_resizable: false,
                    is_minimizable: false,
                    focus: false,
                    show: false,
                    icon: None,
                    display_id: None,
                    app_id: None,
                    window_min_size: None,
                    tabbing_identifier: None,
                },
            )?;
            let handle = window
                .window_handle()
                .map_err(|error| anyhow::anyhow!("Read the native window handle: {error}"))?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                anyhow::bail!("Expected an AppKit window handle");
            };
            // SAFETY: The platform window owns this live GPUIView for the entire case.
            let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
            let native_window = view.window().context("GPUIView must belong to a window")?;
            let content = native_window
                .contentView()
                .context("Missing content view")?;
            ensure!(!native_window.isVisible());
            ensure!(!native_window.isKeyWindow());

            eprintln!("{class_name}: before adapter");
            let initial_content_focus = focus(&content);
            let initial_window_focus = focus(&native_window);
            ensure!(same_object(&initial_window_focus, &initial_content_focus));

            // The adapter owns only AX state; the native window stays hidden and non-key.
            // SAFETY: The retained native window has a live content view on the main thread.
            let mut adapter = unsafe {
                accesskit_macos::SubclassingAdapter::for_window(
                    Retained::as_ptr(&native_window).cast_mut().cast(),
                    Handlers,
                    Handlers,
                )
            };
            if let Some(events) = adapter.update_view_focus_state(true) {
                events.raise();
            }

            eprintln!("{class_name}: focused input");
            let content_focus = focus(&content);
            let node = content_focus.as_ref().context("Missing focused input")?;
            // SAFETY: AccessKit nodes implement the AppKit accessibility focused getter.
            let is_focused: bool = unsafe { msg_send![&**node, isAccessibilityFocused] };
            ensure!(is_focused);
            ensure!(
                same_object(&focus(&native_window), &content_focus),
                "{class_name} must forward the content view's focused input"
            );
            eprintln!("{class_name}: no focused control");
            if let Some(events) = adapter.update_if_active(|| tree(ROOT)) {
                events.raise();
            }
            ensure!(focus(&content).is_none());
            ensure!(focus(&native_window).is_none());

            eprintln!("{class_name}: host unfocused");
            if let Some(events) = adapter.update_if_active(|| tree(INPUT)) {
                events.raise();
            }
            if let Some(events) = adapter.update_view_focus_state(false) {
                events.raise();
            }
            ensure!(focus(&content).is_none());
            ensure!(focus(&native_window).is_none());

            drop(adapter);
            eprintln!("{class_name}: after adapter");
            ensure!(same_object(&focus(&content), &initial_content_focus));
            ensure!(same_object(&focus(&native_window), &initial_window_focus));
            ensure!(!native_window.isVisible());
            ensure!(!native_window.isKeyWindow());
        }
        eprintln!("Native accessibility focus regression passed");
        Ok(())
    }
}
