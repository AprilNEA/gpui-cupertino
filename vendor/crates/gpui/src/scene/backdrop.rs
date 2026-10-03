// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

use super::{DrawOrder, Primitive, Scene};
use crate::{Bounds, ContentMask, Pixels, ScaledPixels, WindowAppearance};

/// An inactive Clear material over earlier opaque SDR window contents.
///
/// The blur radius controls the encoded-RGB mip filter, not a Gaussian sigma.
/// Parameters must be finite, radii nonnegative, and scale positive. Texture
/// dimensions and shader coordinates must fit the Metal device's limits.
#[derive(Clone, Copy, Debug)]
pub struct ClearBackdrop {
    /// Draw order assigned by the scene.
    pub order: DrawOrder,
    /// Device pixels per logical pixel.
    pub scale_factor: f32,
    /// Material bounds in device pixels.
    pub bounds: Bounds<ScaledPixels>,
    /// Output clipping; source sampling is independent of this mask.
    pub content_mask: ContentMask<ScaledPixels>,
    /// Continuous corner radius in device pixels.
    pub corner_radius: ScaledPixels,
    /// Filter radius in device pixels.
    pub blur_radius: ScaledPixels,
    /// Appearance used for the inactive Clear face color.
    pub appearance: WindowAppearance,
    /// Effective opacity of the element and its ancestors.
    pub opacity: f32,
}

/// Logical-pixel inputs for [`crate::Window::paint_clear_backdrop`].
///
/// Requires an Apple GPU and an opaque SDR window. The material uses the
/// window's appearance and the inactive Clear recipe, regardless of focus.
#[derive(Clone, Copy, Debug)]
pub struct PaintClearBackdrop {
    /// Material bounds in logical pixels.
    pub bounds: Bounds<Pixels>,
    /// Finite, nonnegative continuous corner radius.
    pub corner_radius: Pixels,
    /// Finite, nonnegative filter radius; this is not a Gaussian sigma.
    pub blur_radius: Pixels,
}

impl From<ClearBackdrop> for Primitive {
    fn from(backdrop: ClearBackdrop) -> Self {
        Self::ClearBackdrop(backdrop)
    }
}

/// A material sampling the window contents painted before this primitive.
///
/// Distance parameters use device pixels. Callers must supply finite values,
/// nonnegative radii, and positive refraction width and scale. This is a scene record,
/// not a GPU buffer layout.
#[derive(Clone, Copy, Debug)]
pub struct Backdrop {
    /// Draw order assigned by the scene.
    pub order: DrawOrder,
    /// Positive device pixels per logical pixel, used for edge details.
    pub scale_factor: f32,
    /// Continuous shape: 0 for rounded rectangle, 1 for capsule, 2 for circle.
    pub shape: u32,
    /// Bounds of the material within the window.
    pub bounds: Bounds<ScaledPixels>,
    /// Output clipping; background sampling can extend beyond this region.
    pub content_mask: ContentMask<ScaledPixels>,
    /// Rounded rectangle corner radius.
    pub corner_radius: ScaledPixels,
    /// Standard deviation of a pixel-integrated Gaussian, truncated at four sigma.
    pub blur_sigma: ScaledPixels,
    /// Straight linear RGBA tint.
    pub tint: [f32; 4],
    /// Saturation multiplier.
    pub saturation: f32,
    /// Linear brightness offset.
    pub brightness: f32,
    /// Refraction displacement at the shape boundary.
    pub refraction_amount: ScaledPixels,
    /// Distance over which refraction falls off inside the shape.
    pub refraction_width: ScaledPixels,
    /// Blend from the shape's optical direction to radial direction.
    pub direction_mix: f32,
    /// Chromatic dispersion displacement.
    pub dispersion: ScaledPixels,
    /// Edge highlight strength.
    pub highlight: f32,
    /// Neighboring background color mixed into the inner edge, in `[0, 1]`.
    pub edge_bleed: f32,
    /// Effective opacity of the element and its ancestors.
    pub opacity: f32,
}

/// Logical-pixel parameters for [`crate::Window::paint_backdrop`].
///
/// Values must be finite, radii nonnegative, and refraction width positive.
/// The material is restricted to the current content mask, while its source
/// consists of earlier window contents. Window-backdrop and desktop sampling
/// are not supported. The captured contents must be opaque SDR; transparent
/// window alpha and HDR/EDR composition are not supported.
#[derive(Clone, Copy, Debug)]
pub struct PaintBackdrop {
    /// Continuous shape: 0 for rounded rectangle, 1 for capsule, 2 for circle.
    pub shape: u32,
    /// Bounds of the material within the window.
    pub bounds: Bounds<Pixels>,
    /// Rounded rectangle corner radius.
    pub corner_radius: Pixels,
    /// Standard deviation of a pixel-integrated Gaussian, truncated at four sigma.
    pub blur_sigma: Pixels,
    /// Straight linear RGBA tint.
    pub tint: [f32; 4],
    /// Saturation multiplier.
    pub saturation: f32,
    /// Linear brightness offset.
    pub brightness: f32,
    /// Refraction displacement at the shape boundary.
    pub refraction_amount: Pixels,
    /// Distance over which refraction falls off inside the shape.
    pub refraction_width: Pixels,
    /// Blend from the shape's optical direction to radial direction.
    pub direction_mix: f32,
    /// Chromatic dispersion displacement.
    pub dispersion: Pixels,
    /// Edge highlight strength.
    pub highlight: f32,
    /// Neighboring background color mixed into the inner edge, in `[0, 1]`.
    pub edge_bleed: f32,
}

impl From<Backdrop> for Primitive {
    fn from(backdrop: Backdrop) -> Self {
        Self::Backdrop(backdrop)
    }
}

impl Scene {
    pub(super) fn start_material_epoch(&mut self) -> DrawOrder {
        self.order_base = self.max_order + 1;
        self.max_order = self.order_base;
        self.primitive_bounds.clear();

        // A material splits even an open paint_layer. Rebase its descendants
        // without losing the bounds needed for ordering subsequent siblings.
        for (bounds, order) in &mut self.layer_stack {
            *order = self.order_base + self.primitive_bounds.insert(*bounds);
            self.max_order = self.max_order.max(*order);
        }
        self.order_base
    }
}
