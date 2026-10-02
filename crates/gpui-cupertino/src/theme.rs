//! Semantic colors shared by Cupertino components.

use gpui::{Hsla, Window, WindowAppearance, rgb};

/// An opaque light or dark palette for application surfaces and controls.
///
/// These colors are Cupertino defaults, not a snapshot of the system accent color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The application background.
    pub background: Hsla,
    /// Primary text on application and control backgrounds.
    pub foreground: Hsla,
    /// Secondary text, placeholders, and disabled labels.
    pub secondary_foreground: Hsla,
    /// The background of ordinary buttons and text fields.
    pub control_background: Hsla,
    /// The boundary of ordinary buttons and text fields.
    pub control_border: Hsla,
    /// The background of a primary action.
    pub accent: Hsla,
    /// Text on the accent background.
    pub accent_foreground: Hsla,
    /// The visible keyboard focus indicator.
    pub focus_ring: Hsla,
    /// A translucent text selection background.
    pub selection: Hsla,
}

impl Theme {
    /// Resolves the palette from the window's current appearance.
    pub fn for_window(window: &Window) -> Self {
        window.appearance().into()
    }
}

impl From<WindowAppearance> for Theme {
    fn from(appearance: WindowAppearance) -> Self {
        let dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let accent = Hsla::from(rgb(if dark { 0x0a84ff } else { 0x0066cc }));
        Self {
            background: rgb(if dark { 0x1c1c1e } else { 0xf5f5f7 }).into(),
            foreground: rgb(if dark { 0xf5f5f7 } else { 0x1d1d1f }).into(),
            secondary_foreground: rgb(if dark { 0xb9b9bf } else { 0x626267 }).into(),
            control_background: rgb(if dark { 0x3a3a3c } else { 0xffffff }).into(),
            control_border: rgb(if dark { 0x8e8e93 } else { 0x86868b }).into(),
            accent,
            accent_foreground: rgb(if dark { 0x000000 } else { 0xffffff }).into(),
            focus_ring: rgb(if dark { 0x64aaff } else { 0x0066cc }).into(),
            selection: accent.opacity(0.25),
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::Rgba;

    use super::*;

    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let luminance = |color: Hsla| {
            let Rgba { r, g, b, .. } = color.into();
            [r, g, b]
                .into_iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(channel, weight)| {
                    let linear = if channel <= 0.04045 {
                        channel / 12.92
                    } else {
                        ((channel + 0.055) / 1.055).powf(2.4)
                    };
                    linear * weight
                })
                .sum::<f32>()
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn palettes_preserve_readable_text_and_visible_control_boundaries() {
        for appearance in [
            WindowAppearance::Light,
            WindowAppearance::VibrantLight,
            WindowAppearance::Dark,
            WindowAppearance::VibrantDark,
        ] {
            let theme = Theme::from(appearance);
            for surface in [theme.background, theme.control_background] {
                for text in [theme.foreground, theme.secondary_foreground] {
                    assert!(contrast(text, surface) >= 4.5, "{appearance:?}");
                }
                for boundary in [theme.control_border, theme.focus_ring] {
                    assert!(contrast(boundary, surface) >= 3.0, "{appearance:?}");
                }
            }
            assert!(contrast(theme.accent_foreground, theme.accent) >= 4.5);
            assert!(
                contrast(
                    theme.foreground,
                    theme.control_background.blend(theme.selection)
                ) >= 4.5
            );
        }
    }
}
