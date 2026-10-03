//! Apple-inspired components, materials, and motion for GPUI.
//!
//! This crate is the home for GPUI-specific rendering and interaction.

/// Framework-independent foundations shared by Cupertino integrations.
pub use cupertino;

/// Reusable Cupertino controls and containers.
pub mod components;
pub mod theme;

/// Initialize component bindings and system preferences before creating windows.
///
/// Call once during startup. Bind ordinary Tab navigation in an ancestor key context.
/// The application owns global actions; modal panels constrain their own Tab traversal.
/// On macOS, initialization synchronizes reduced motion and observes accessibility changes.
pub fn init(cx: &mut gpui::App) {
    components::init(cx);
    #[cfg(target_os = "macos")]
    platform::accessibility_preferences(cx);
}

/// GPUI spring animation builders using Cupertino's validated design parameters.
pub mod motion;

/// Window-local glass materials backed by the patched Metal renderer.
#[cfg(target_os = "macos")]
pub mod materials;

/// Public macOS preferences and their change notifications.
#[cfg(target_os = "macos")]
pub mod platform;
