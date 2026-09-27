//! Observe public AppKit accessibility settings without loading private frameworks.

use std::ptr::NonNull;

use block2::RcBlock;
use futures::{StreamExt, channel::mpsc};
use gpui::{App, Global, Task};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSWorkspace, NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue};

/// Current system accessibility preferences relevant to materials and motion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessibilityPreferences {
    /// Use an opaque surface instead of sampling the background.
    pub reduce_transparency: bool,
    /// Use an opaque surface and a contrasting outline.
    pub increase_contrast: bool,
    /// Complete decorative animations immediately.
    pub reduce_motion: bool,
}

impl AccessibilityPreferences {
    fn read() -> Self {
        let workspace = NSWorkspace::sharedWorkspace();
        Self {
            reduce_transparency: workspace.accessibilityDisplayShouldReduceTransparency(),
            increase_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
            reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        }
    }
}

struct Observer {
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for Observer {
    fn drop(&mut self) {
        // SAFETY: this token was returned by this center's observer registration.
        unsafe { self.center.removeObserver((*self.token).as_ref()) };
    }
}

struct AccessibilityState {
    preferences: AccessibilityPreferences,
    // Unregister before dropping the receiving foreground task.
    _observer: Observer,
    _task: Task<()>,
}

impl Global for AccessibilityState {}

/// Read system settings and subscribe to changes for the lifetime of the GPUI app.
///
/// Call once during startup when using springs without a [`crate::materials::Glass`].
/// Glass also initializes this automatically. Changes refresh all windows and
/// update [`App::reduce_motion`]; no polling or continuous animation is required.
pub fn accessibility_preferences(cx: &mut App) -> AccessibilityPreferences {
    if let Some(state) = cx.try_global::<AccessibilityState>() {
        return state.preferences;
    }

    let (sender, mut receiver) = mpsc::unbounded();
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        // A notification already queued on the main queue can outlive removal.
        // The receiver also drops on the main thread, so it cannot close between
        // this check and the synchronous send below.
        if sender.is_closed() {
            return;
        }
        sender
            .unbounded_send(())
            .expect("the main-thread accessibility receiver remains alive during this callback");
    });
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: this immutable notification-name constant is exported by AppKit.
    let notification = unsafe { NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification };
    // SAFETY: the callback captures only a Send channel sender; notifications
    // arrive on the main queue and their payload is not dereferenced or retained.
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(notification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &block,
        )
    };
    let preferences = AccessibilityPreferences::read();
    cx.set_reduce_motion(preferences.reduce_motion);
    let task = cx.spawn(async move |cx| {
        while receiver.next().await.is_some() {
            cx.update(|cx| {
                let preferences = AccessibilityPreferences::read();
                cx.global_mut::<AccessibilityState>().preferences = preferences;
                cx.set_reduce_motion(preferences.reduce_motion);
                cx.refresh_windows();
            });
        }
    });
    cx.set_global(AccessibilityState {
        preferences,
        _observer: Observer { center, token },
        _task: task,
    });
    preferences
}
