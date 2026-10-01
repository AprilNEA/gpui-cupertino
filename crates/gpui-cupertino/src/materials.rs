//! Window-local backdrop effects. Available only with the macOS Metal backend.
//!
//! A glass captures earlier drawing commands and paints its content afterward.
//! It cannot sample the desktop or another application's windows. Siblings are
//! separate effects in paint order, not a shared capture or a fused glass group.

use crate::platform::accessibility_preferences;
use cupertino::materials::{ClearGlassMaterial, GlassMaterial, GlassShape};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, PaintBackdrop, PaintClearBackdrop, Pixels, Window, WindowAppearance, fill, point, px,
    rgb, size,
};

/// Additional accessibility preferences provided by the host application.
///
/// System preferences always apply. Either option also removes background
/// sampling and uses an opaque surface when enabled explicitly by the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlassAccessibility {
    /// Replace the translucent material with an opaque surface.
    pub reduce_transparency: bool,
    /// Use an opaque surface and a contrasting outline.
    pub increase_contrast: bool,
}

/// A material behind an existing GPUI element, preserving the child's layout.
///
/// Set dimensions and padding on the content element; leave its background
/// transparent to reveal the glass. Put ancestor clipping or group opacity on
/// an enclosing element so it applies to both the glass and its content.
/// Sampling requires an opaque SDR window background; transparent windows and
/// HDR/EDR composition are not supported.
pub struct Glass {
    material: Material,
    content: AnyElement,
    accessibility: GlassAccessibility,
}

enum Material {
    Gaussian(GlassMaterial),
    Clear(ClearGlassMaterial),
}

impl Glass {
    /// Wrap content with a validated glass material.
    pub fn new(material: GlassMaterial, content: impl IntoElement) -> Self {
        Self {
            material: Material::Gaussian(material),
            content: content.into_any_element(),
            accessibility: GlassAccessibility::default(),
        }
    }

    /// Wrap content with inactive Clear, using the host window's appearance.
    ///
    /// Requires the macOS Metal backend and an Apple GPU.
    pub fn clear(material: ClearGlassMaterial, content: impl IntoElement) -> Self {
        Self {
            material: Material::Clear(material),
            content: content.into_any_element(),
            accessibility: GlassAccessibility::default(),
        }
    }

    /// Apply host-provided transparency and contrast preferences.
    pub fn accessibility(mut self, accessibility: GlassAccessibility) -> Self {
        self.accessibility = accessibility;
        self
    }
}

impl IntoElement for Glass {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Glass {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        accessibility_preferences(cx);
        (self.content.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let outline_shape = match self.material {
            Material::Gaussian(material) => material.options().shape,
            Material::Clear(material) => material.shape(),
        };
        let (shape, bounds, corner_radius) = outline(outline_shape, bounds);
        let system = accessibility_preferences(cx);
        let increase_contrast = self.accessibility.increase_contrast || system.increase_contrast;
        if self.accessibility.reduce_transparency || system.reduce_transparency || increase_contrast
        {
            let dark = matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            );
            let mut surface = fill(bounds, rgb(if dark { 0x202024 } else { 0xf3f3f6 }))
                .corner_radii(corner_radius);
            if increase_contrast {
                surface = surface.border_widths(px(1.0)).border_color(rgb(if dark {
                    0xffffff
                } else {
                    0x000000
                }));
            }
            window.paint_continuous_quad(surface);
        } else if let Material::Gaussian(material) = self.material {
            let options = material.options();
            window.paint_backdrop(PaintBackdrop {
                shape,
                bounds,
                corner_radius,
                blur_sigma: px(options.blur_sigma),
                tint: options.tint,
                saturation: options.saturation,
                brightness: options.brightness,
                refraction_amount: px(options.refraction.amount),
                refraction_width: px(options.refraction.width),
                direction_mix: options.refraction.direction_mix,
                dispersion: px(options.dispersion),
                highlight: options.highlight,
                edge_bleed: options.edge_bleed,
            });
        } else if let Material::Clear(material) = self.material {
            window.paint_clear_backdrop(PaintClearBackdrop {
                bounds,
                corner_radius,
                blur_radius: px(material.blur_radius()),
            });
        }
        self.content.paint(window, cx);
    }
}

fn outline(shape: GlassShape, bounds: Bounds<Pixels>) -> (u32, Bounds<Pixels>, Pixels) {
    let diameter = bounds.size.width.min(bounds.size.height);
    let half = diameter / 2.0;
    match shape {
        GlassShape::Circle => (
            2,
            Bounds::new(
                bounds.center() - point(half, half),
                size(diameter, diameter),
            ),
            half,
        ),
        GlassShape::Capsule => (1, bounds, half),
        GlassShape::RoundedRectangle { corner_radius } => (0, bounds, px(corner_radius).min(half)),
    }
}

#[cfg(test)]
#[path = "materials/tests.rs"]
mod element_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_fit_nonsquare_bounds_for_glass_and_opaque_accessibility() {
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(40.0)));
        let (_, circle, radius) = outline(GlassShape::Circle, bounds);
        assert_eq!(circle.origin, point(px(40.0), px(20.0)));
        assert_eq!(circle.size, size(px(40.0), px(40.0)));
        assert_eq!(radius, px(20.0));
        assert_eq!(outline(GlassShape::Capsule, bounds), (1, bounds, px(20.0)));
        assert_eq!(
            outline(
                GlassShape::RoundedRectangle {
                    corner_radius: 30.0
                },
                bounds
            ),
            (0, bounds, px(20.0))
        );
    }
}
