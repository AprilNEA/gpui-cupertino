//! Apple-inspired components, materials, and motion for GPUI.
//!
//! This crate is the home for GPUI-specific rendering and interaction.

/// Framework-independent foundations shared by Cupertino integrations.
pub use cupertino;

/// GPUI spring animation builders using Cupertino's validated design parameters.
pub mod motion;

/// Window-local glass materials backed by the patched Metal renderer.
#[cfg(target_os = "macos")]
pub mod materials;

/// Public macOS preferences and their change notifications.
#[cfg(target_os = "macos")]
pub mod platform;
