//! Deterministic isolated inputs for measuring public AppKit glass.

use super::{
    background::Background,
    comparison::{HEIGHT, WIDTH},
};
use anyhow::{Context, Result, bail, ensure};
use gpui::{DevicePixels, PlatformHeadlessRenderer, Scene, size};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use std::path::Path;

enum Shape {
    Roundrect,
    Capsule,
    Circle,
}

impl Shape {
    fn default_size(&self) -> [u32; 2] {
        match self {
            Self::Roundrect | Self::Capsule => [256, 128],
            Self::Circle => [128, 128],
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Roundrect => "roundrect",
            Self::Capsule => "capsule",
            Self::Circle => "circle",
        }
    }
}

pub(super) struct Probe {
    background: Background,
    shape: Shape,
    dimensions: [u32; 2],
    offset: [i32; 2],
    pub(super) dark: bool,
    pub(super) clear: bool,
    pub(super) capture_count: u32,
}

impl Probe {
    pub(super) fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let (mut background, mut shape, mut dark, mut clear) = (None, None, None, None);
        let (mut dimensions, mut offset) = (None, None);
        let mut capture_count = None;
        while let Some(flag) = args.next() {
            let value = args
                .next()
                .with_context(|| format!("{flag} needs a value"))?;
            match flag.as_str() {
                "--background" => ensure!(
                    background.replace(Background::parse(&value)?).is_none(),
                    "duplicate --background"
                ),
                "--shape" => {
                    let value = match value.as_str() {
                        "roundrect" => Shape::Roundrect,
                        "capsule" => Shape::Capsule,
                        "circle" => Shape::Circle,
                        _ => bail!("shape must be roundrect, capsule, or circle"),
                    };
                    ensure!(shape.replace(value).is_none(), "duplicate --shape");
                }
                "--size" => {
                    let (width, height) = value
                        .split_once('x')
                        .context("size must be WIDTHxHEIGHT in integer logical pixels")?;
                    let width: u32 = width.parse().context("invalid size width")?;
                    let height: u32 = height.parse().context("invalid size height")?;
                    ensure!(
                        width > 0 && width <= WIDTH as u32 && height > 0 && height <= HEIGHT as u32,
                        "size must be positive and fit inside the 384x320 panel"
                    );
                    ensure!(
                        dimensions.replace([width, height]).is_none(),
                        "duplicate --size"
                    );
                }
                "--offset" => {
                    let (dx, dy) = value
                        .split_once(',')
                        .context("offset must be DX,DY in integer logical pixels")?;
                    let dx = dx.parse().context("invalid horizontal offset")?;
                    let dy = dy.parse().context("invalid vertical offset")?;
                    ensure!(offset.replace([dx, dy]).is_none(), "duplicate --offset");
                }
                "--capture-count" => {
                    let count: u32 = value.parse().context("invalid capture count")?;
                    ensure!(
                        (2..=32).contains(&count),
                        "capture count must be 2 through 32"
                    );
                    ensure!(
                        capture_count.replace(count).is_none(),
                        "duplicate --capture-count"
                    );
                }
                "--appearance" => {
                    let value = match value.as_str() {
                        "light" => false,
                        "dark" => true,
                        _ => bail!("appearance must be light or dark"),
                    };
                    ensure!(dark.replace(value).is_none(), "duplicate --appearance");
                }
                "--style" => {
                    let value = match value.as_str() {
                        "regular" => false,
                        "clear" => true,
                        _ => bail!("style must be regular or clear"),
                    };
                    ensure!(clear.replace(value).is_none(), "duplicate --style");
                }
                _ => bail!("unknown probe option {flag}"),
            }
        }
        let shape = shape.context("probe requires --shape")?;
        let dimensions = dimensions.unwrap_or_else(|| shape.default_size());
        ensure!(
            !matches!(shape, Shape::Circle) || dimensions[0] == dimensions[1],
            "circle size must have equal width and height"
        );
        let probe = Self {
            background: background.context("probe requires --background")?,
            shape,
            dimensions,
            offset: offset.unwrap_or([0, 0]),
            dark: dark.context("probe requires --appearance")?,
            clear: clear.context("probe requires --style")?,
            capture_count: capture_count.unwrap_or(2),
        };
        let [x, y, width, height, _] = probe.geometry();
        ensure!(
            x >= 0.0 && y >= 0.0 && x + width <= WIDTH && y + height <= HEIGHT,
            "offset shape must fit inside the 384x320 panel"
        );
        Ok(probe)
    }

    pub(super) fn name(&self) -> String {
        let size = if self.dimensions == self.shape.default_size() {
            String::new()
        } else {
            format!("-{}x{}", self.dimensions[0], self.dimensions[1])
        };
        let offset = if self.offset == [0, 0] {
            String::new()
        } else {
            format!("-offset{}x{}", self.offset[0], self.offset[1])
        };
        let count = if self.capture_count == 2 {
            String::new()
        } else {
            format!("-count{}", self.capture_count)
        };
        format!(
            "probe-{}-{}-{}{size}{offset}-{}{count}",
            if self.dark { "dark" } else { "light" },
            if self.clear { "clear" } else { "regular" },
            self.shape.name(),
            self.background.name()
        )
    }

    pub(super) fn geometry(&self) -> [f32; 5] {
        let [width, height] = self.dimensions.map(|value| value as f32);
        let half_short_side = width.min(height) / 2.0;
        let radius = match self.shape {
            Shape::Roundrect => half_short_side.min(20.0),
            Shape::Capsule | Shape::Circle => half_short_side,
        };
        [
            (WIDTH - width) / 2.0 + self.offset[0] as f32,
            (HEIGHT - height) / 2.0 + self.offset[1] as f32,
            width,
            height,
            radius,
        ]
    }

    pub(super) fn save_background(
        &self,
        renderer: &mut MetalHeadlessRenderer,
        scale: f32,
        path: &Path,
    ) -> Result<()> {
        self.background.validate_scale(scale)?;
        let dimensions = [(WIDTH * scale) as u32, (HEIGHT * scale) as u32];
        let mut image = renderer.render_scene_to_image(
            &Scene::default(),
            size(
                DevicePixels(dimensions[0] as i32),
                DevicePixels(dimensions[1] as i32),
            ),
        )?;
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            pixel.0 = self.background.pixel([x, y], dimensions, scale, self.dark);
        }
        image.save(path).context("saving exact probe input")
    }

    pub(super) fn save_metadata(
        &self,
        directory: &Path,
        scale: f64,
        titlebar_height: f64,
    ) -> Result<()> {
        let name = self.name();
        let metadata = self.metadata(scale, titlebar_height);
        println!("{metadata}");
        std::fs::write(directory.join(format!("{name}.json")), metadata)
            .context("saving probe parameters")
    }

    fn metadata(&self, scale: f64, titlebar_height: f64) -> String {
        let name = self.name();
        let captures = (0..self.capture_count)
            .map(|repeat| format!("\"{name}-capture{repeat}.png\""))
            .collect::<Vec<_>>()
            .join(", ");
        let [x, y, w, h, radius] = self.geometry();
        format!(
            r#"{{
  "case": "{name}",
  "mode": "probe",
  "appearance": "{}",
  "style": "{}",
  "background": {},
  "coordinate_system": "top-left logical pixels inside window content",
  "panel_logical_size": [384, 320],
  "content_logical_size": [768, 320],
  "native_panel_origin_logical": [0, 0],
  "control_panel_origin_logical": [384, 0],
  "capture_content_origin_device": [0, {}],
  "scale": {scale},
  "glass": {{"shape": "{}", "bounds_logical": [{x}, {y}, {w}, {h}], "corner_radius_logical": {radius}}},
  "required_window_state": {{"visible": true, "active": false, "key": false}},
  "background_png": "{name}-background.png",
  "captures": [{captures}]
}}
"#,
            if self.dark { "dark" } else { "light" },
            if self.clear { "clear" } else { "regular" },
            self.background.metadata(self.dark),
            titlebar_height * scale,
            self.shape.name()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(shape: &str, background: &str, options: &[&str]) -> Result<Probe> {
        Probe::parse(
            [
                "--background",
                background,
                "--shape",
                shape,
                "--appearance",
                "light",
                "--style",
                "clear",
            ]
            .into_iter()
            .chain(options.iter().copied())
            .map(str::to_owned),
        )
    }

    #[test]
    fn step_phase_preserves_exact_pixel_boundary_and_metadata() -> Result<()> {
        for (input, scale, before, after) in [
            ("step:v", 1.0, [191, 20], [192, 20]),
            ("step:v:7", 1.0, [198, 20], [199, 20]),
            ("step:v:7", 2.0, [397, 40], [398, 40]),
            ("step:h:-7", 1.0, [20, 152], [20, 153]),
            ("step:h:-7", 2.0, [40, 305], [40, 306]),
            ("step:v:0.5", 2.0, [384, 40], [385, 40]),
            ("step:h:-0.5", 2.0, [40, 318], [40, 319]),
        ] {
            let background = Background::parse(input)?;
            background.validate_scale(scale)?;
            let dimensions = [(WIDTH * scale) as u32, (HEIGHT * scale) as u32];
            assert_eq!(
                background.pixel(before, dimensions, scale, false),
                [0, 0, 0, 255],
                "{input} at {scale}x"
            );
            assert_eq!(
                background.pixel(after, dimensions, scale, false),
                [255; 4],
                "{input} at {scale}x"
            );
        }
        let half_pixel = Background::parse("step:v:0.5")?;
        assert!(half_pixel.validate_scale(1.0).is_err());
        assert_eq!(
            half_pixel.metadata(false),
            r#"{"kind":"step","edge":"vertical","low_srgb":[0,0,0],"high_srgb":[255,255,255],"phase_logical":0.5,"split_logical":192.5,"split_fraction":0.5013020833333334}"#
        );
        Ok(())
    }

    #[test]
    fn size_centers_shapes_and_case_names_identify_effective_parameters() -> Result<()> {
        for (shape, options, expected) in [
            ("roundrect", vec![], [64.0, 96.0, 256.0, 128.0, 20.0]),
            ("capsule", vec![], [64.0, 96.0, 256.0, 128.0, 64.0]),
            ("circle", vec![], [128.0, 96.0, 128.0, 128.0, 64.0]),
            (
                "roundrect",
                vec!["--size", "20x10"],
                [182.0, 155.0, 20.0, 10.0, 5.0],
            ),
            (
                "capsule",
                vec!["--size", "64x256"],
                [160.0, 32.0, 64.0, 256.0, 32.0],
            ),
            (
                "circle",
                vec!["--size", "64x64"],
                [160.0, 128.0, 64.0, 64.0, 32.0],
            ),
        ] {
            assert_eq!(probe(shape, "step:v", &options)?.geometry(), expected);
        }
        let default = probe("roundrect", "step:v", &[])?;
        assert_eq!(default.name(), "probe-light-clear-roundrect-step-v");
        assert_eq!(
            probe("roundrect", "step:v:-0", &["--size", "256x128"])?.name(),
            default.name()
        );
        assert_eq!(
            probe("roundrect", "step:v:0.5", &["--size", "128x96"])?.name(),
            "probe-light-clear-roundrect-128x96-step-v-phase0.5"
        );
        Ok(())
    }

    #[test]
    fn offset_moves_only_glass_and_names_identify_effective_translation() -> Result<()> {
        for (offset, expected) in [
            ("-8,0", [64.0, 96.0, 240.0, 128.0, 20.0]),
            ("0,8", [72.0, 104.0, 240.0, 128.0, 20.0]),
            ("-72,-96", [0.0, 0.0, 240.0, 128.0, 20.0]),
            ("72,96", [144.0, 192.0, 240.0, 128.0, 20.0]),
        ] {
            let translated = probe(
                "roundrect",
                "checker:32:13:000000:ffffff",
                &["--offset", offset, "--size", "240x128"],
            )?;
            assert_eq!(translated.geometry(), expected);
            assert_eq!(
                translated.background.pixel([37, 0], [768, 640], 2.0, false),
                [0, 0, 0, 255]
            );
            assert_eq!(
                translated.background.pixel([38, 0], [768, 640], 2.0, false),
                [255; 4]
            );
        }
        assert_eq!(
            probe(
                "roundrect",
                "checker:32:13:000000:ffffff",
                &["--size", "240x128", "--offset", "-8,0"],
            )?
            .name(),
            "probe-light-clear-roundrect-240x128-offset-8x0-checker-32-phase13-000000-ffffff"
        );
        let original = probe("roundrect", "step:v", &[])?;
        let zero = probe("roundrect", "step:v", &["--offset", "-0,0"])?;
        assert_eq!(zero.geometry(), original.geometry());
        assert_eq!(zero.name(), original.name());
        Ok(())
    }

    #[test]
    fn invalid_offsets_fail_instead_of_clamping() {
        for value in [
            "1",
            "1,2,3",
            "0.5,0",
            "NaN,0",
            "0,inf",
            "2147483648,0",
            "-65,0",
            "65,0",
            "0,-97",
            "0,97",
        ] {
            assert!(
                probe("roundrect", "step:v", &["--offset", value]).is_err(),
                "accepted {value}"
            );
        }
        assert!(
            probe(
                "roundrect",
                "step:v",
                &["--offset", "0,0", "--offset", "0,0"]
            )
            .is_err()
        );
    }

    #[test]
    fn capture_count_names_and_metadata_preserve_default_and_list_all_frames() -> Result<()> {
        let default = probe("roundrect", "step:v", &[])?;
        assert_eq!(
            probe("roundrect", "step:v", &["--capture-count", "2"])?.metadata(2.0, 28.0),
            default.metadata(2.0, 28.0)
        );
        let repeated = probe("roundrect", "step:v", &["--capture-count", "4"])?;
        assert_eq!(repeated.name(), "probe-light-clear-roundrect-step-v-count4");
        let captures = (0..4)
            .map(|i| format!("\"probe-light-clear-roundrect-step-v-count4-capture{i}.png\""))
            .collect::<Vec<_>>()
            .join(", ");
        assert!(
            repeated
                .metadata(2.0, 28.0)
                .contains(&format!("\"captures\": [{captures}]"))
        );
        assert!(probe("circle", "ramp", &["--capture-count", "32"]).is_ok());
        for value in ["0", "1", "33", "-1", "2.5", "NaN", "4294967296"] {
            assert!(
                probe("roundrect", "step:v", &["--capture-count", value]).is_err(),
                "accepted {value}"
            );
        }
        assert!(
            probe(
                "roundrect",
                "step:v",
                &["--capture-count", "2", "--capture-count", "2"]
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn invalid_phase_and_size_fail_at_the_input_boundary() {
        for value in [
            "step:v:NaN",
            "step:v:inf",
            "step:h:0.25",
            "step:v:192",
            "step:h:-160",
            "step:z:1",
            "step:v:1:2",
        ] {
            assert!(Background::parse(value).is_err(), "accepted {value}");
        }
        for value in [
            "0x64", "64x0", "385x64", "64x321", "-1x64", "64.5x64", "64x64x64",
        ] {
            assert!(
                probe("roundrect", "step:v", &["--size", value]).is_err(),
                "accepted {value}"
            );
        }
        assert!(probe("circle", "step:v", &["--size", "128x96"]).is_err());
        assert!(probe("capsule", "step:v", &["--size", "64x64", "--size", "64x64"]).is_err());
    }
}
