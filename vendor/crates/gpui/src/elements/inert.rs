use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

/// Paint a subtree while excluding its input, focus, and accessibility registrations.
///
/// Use this boundary for content behind a modal panel. Descendant deferred elements
/// remain inert. Application-global actions and persistent application subscriptions
/// remain the application's responsibility.
pub fn inert(child: impl IntoElement, disabled: bool) -> Inert {
    Inert {
        child: child.into_any_element(),
        disabled,
    }
}

/// A paint-preserving input boundary created by [`inert`].
pub struct Inert {
    child: AnyElement,
    disabled: bool,
}

impl IntoElement for Inert {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Inert {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (
            window.with_inert(self.disabled, |window| {
                self.child.request_layout(window, cx)
            }),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_inert(self.disabled, |window| self.child.prepaint(window, cx));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_inert(self.disabled, |window| self.child.paint(window, cx));
    }
}
