//! Renderer-independent material parameters in logical pixels and linear color.
//!
//! These are independent calibration controls, not Apple private material recipes.

/// An analytic glass outline, fitted inside its element's bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlassShape {
    /// A centered circle whose diameter is the shorter bound dimension.
    Circle,
    /// A rounded rectangle with radius equal to half its shorter dimension.
    Capsule,
    /// A rounded rectangle, with the radius clamped to half its shorter dimension.
    RoundedRectangle {
        /// Finite, nonnegative corner radius in logical pixels.
        corner_radius: f32,
    },
}

/// Distance-driven optical displacement, independent of blur and geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Refraction {
    /// Signed maximum sampling displacement in logical pixels.
    pub amount: f32,
    /// Positive width of the inward edge band in logical pixels.
    pub width: f32,
    /// Blend from contour normal (`0`) to center-to-edge radial direction (`1`).
    pub direction_mix: f32,
}

impl Default for Refraction {
    fn default() -> Self {
        Self {
            amount: 0.0,
            width: 1.0,
            direction_mix: 0.0,
        }
    }
}

/// Unvalidated material settings; construct a [`GlassMaterial`] before rendering.
///
/// The default is an identity material: it leaves the captured background unchanged
/// inside a rectangular outline. Blur specifies a target Gaussian standard deviation, not an
/// uncalibrated platform blur radius; renderers may approximate the kernel. All lengths use logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassMaterialOptions {
    /// Analytic outline used for coverage and optical direction.
    pub shape: GlassShape,
    /// Finite, nonnegative target Gaussian standard deviation in logical pixels.
    pub blur_sigma: f32,
    /// Straight linear RGBA tint, with every component in `[0, 1]`.
    pub tint: [f32; 4],
    /// Finite, nonnegative saturation multiplier; `1` preserves saturation.
    pub saturation: f32,
    /// Finite additive linear RGB offset; `0` preserves brightness.
    pub brightness: f32,
    /// Edge displacement and optical direction settings.
    pub refraction: Refraction,
    /// Finite, nonnegative spectral sampling displacement in logical pixels.
    pub dispersion: f32,
    /// White directional edge-highlight opacity in `[0, 1]`.
    pub highlight: f32,
    /// Neighboring background color mixed into the inner edge, in `[0, 1]`.
    /// The falloff uses the refraction width; `0` disables color bleeding.
    pub edge_bleed: f32,
}

impl Default for GlassMaterialOptions {
    fn default() -> Self {
        Self {
            shape: GlassShape::RoundedRectangle { corner_radius: 0.0 },
            blur_sigma: 0.0,
            tint: [0.0; 4],
            saturation: 1.0,
            brightness: 0.0,
            refraction: Refraction::default(),
            dispersion: 0.0,
            highlight: 0.0,
            edge_bleed: 0.0,
        }
    }
}

/// A validated glass material with immutable, finite parameters.
///
/// Rendering also requires valid element bounds, device scale, and texture limits.
/// Material validation does not validate the subsequent logical-to-device conversion.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlassMaterial(GlassMaterialOptions);

/// A material parameter outside its documented range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid glass material parameter: {0}")]
pub struct MaterialError(pub &'static str);

impl GlassMaterial {
    /// Read the validated material settings.
    pub const fn options(&self) -> &GlassMaterialOptions {
        &self.0
    }
}

impl TryFrom<GlassMaterialOptions> for GlassMaterial {
    type Error = MaterialError;

    fn try_from(options: GlassMaterialOptions) -> Result<Self, Self::Error> {
        for (name, value) in [
            ("blur_sigma", options.blur_sigma),
            ("saturation", options.saturation),
            ("dispersion", options.dispersion),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(MaterialError(name));
            }
        }
        if let GlassShape::RoundedRectangle { corner_radius } = options.shape
            && (!corner_radius.is_finite() || corner_radius < 0.0)
        {
            return Err(MaterialError("corner_radius"));
        }
        for (name, value) in [
            ("brightness", options.brightness),
            ("refraction.amount", options.refraction.amount),
        ] {
            if !value.is_finite() {
                return Err(MaterialError(name));
            }
        }
        if !options.refraction.width.is_finite() || options.refraction.width <= 0.0 {
            return Err(MaterialError("refraction.width"));
        }
        for (name, values) in [
            ("tint", options.tint.as_slice()),
            ("highlight", std::slice::from_ref(&options.highlight)),
            ("edge_bleed", std::slice::from_ref(&options.edge_bleed)),
            (
                "refraction.direction_mix",
                std::slice::from_ref(&options.refraction.direction_mix),
            ),
        ] {
            if values.iter().any(|v| !(0.0..=1.0).contains(v)) {
                return Err(MaterialError(name));
            }
        }
        Ok(Self(options))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_preserves_independent_controls_and_rejects_invalid_numbers() {
        let options = GlassMaterialOptions {
            shape: GlassShape::Capsule,
            blur_sigma: 12.0,
            tint: [0.2, 0.4, 0.8, 0.3],
            refraction: Refraction {
                amount: -8.0,
                width: 16.0,
                direction_mix: 0.5,
            },
            dispersion: 1.5,
            ..Default::default()
        };
        assert_eq!(
            *GlassMaterial::try_from(options).unwrap().options(),
            options
        );
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
            assert_eq!(
                GlassMaterial::try_from(GlassMaterialOptions {
                    blur_sigma: invalid,
                    ..options
                }),
                Err(MaterialError("blur_sigma"))
            );
        }
        for width in [0.0, -1.0, f32::INFINITY, f32::NAN] {
            assert_eq!(
                GlassMaterial::try_from(GlassMaterialOptions {
                    refraction: Refraction {
                        width,
                        ..options.refraction
                    },
                    ..options
                }),
                Err(MaterialError("refraction.width"))
            );
        }
        assert_eq!(
            GlassMaterial::try_from(GlassMaterialOptions {
                shape: GlassShape::RoundedRectangle {
                    corner_radius: -1.0
                },
                ..options
            }),
            Err(MaterialError("corner_radius"))
        );
        for tint in [[f32::NAN, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.01]] {
            assert_eq!(
                GlassMaterial::try_from(GlassMaterialOptions { tint, ..options }),
                Err(MaterialError("tint"))
            );
        }
        for edge_bleed in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert_eq!(
                GlassMaterial::try_from(GlassMaterialOptions {
                    edge_bleed,
                    ..options
                }),
                Err(MaterialError("edge_bleed"))
            );
        }
    }
}
