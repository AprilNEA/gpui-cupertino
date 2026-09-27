//! Delegate spring state, retargeting, and reduced motion to GPUI.

use cupertino::motion::Spring;
use gpui::{SpringAnimation, SpringConfig};

/// Build a GPUI spring from validated Cupertino parameters.
///
/// Use the result with [`gpui::AnimationExt::with_spring`] and a stable element
/// identifier to preserve velocity when the target changes. GPUI completes the
/// animation immediately when [`gpui::App::reduce_motion`] is enabled.
pub fn spring(parameters: Spring) -> SpringAnimation {
    SpringAnimation::new(SpringConfig::new(
        parameters.stiffness(),
        parameters.damping(),
        parameters.mass(),
    ))
}
