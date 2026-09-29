//! Deterministic isolated inputs for measuring public AppKit glass.

use super::comparison::{HEIGHT, WIDTH};
use anyhow::{Context, Result, bail, ensure};
use gpui::{DevicePixels, PlatformHeadlessRenderer, Scene, size};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use std::path::Path;

enum Background {
    Solid(u32),
    Step { vertical: bool, phase: f32 },
    Ramp,
    Checker { cell: u32, phase: u32 },
}

impl Background {
    fn parse(value: &str) -> Result<Self> {
        let parts: Vec<_> = value.split(':').collect();
        match parts.as_slice() {
            ["solid", hex] => {
                ensure!(
                    hex.len() == 6,
                    "solid color must have six hexadecimal digits"
                );
                Ok(Self::Solid(
                    u32::from_str_radix(hex, 16).context("invalid solid RGB color")?,
                ))
            }
            ["step", axis] | ["step", axis, _] => {
                let vertical = match *axis {
                    "v" => true,
                    "h" => false,
                    _ => bail!("step direction must be h or v"),
                };
                let phase: f32 = parts
                    .get(2)
                    .map_or(Ok(0.0), |phase| phase.parse())
                    .context("step phase must be a number of logical pixels")?;
                ensure!(
                    phase.is_finite() && (phase * 2.0).fract() == 0.0,
                    "step phase must be finite and use half-logical-pixel increments"
                );
                let extent = if vertical { WIDTH } else { HEIGHT };
                ensure!(
                    phase > -extent / 2.0 && phase < extent / 2.0,
                    "step boundary must be strictly inside the panel"
                );
                Ok(Self::Step { vertical, phase })
            }
            ["ramp"] => Ok(Self::Ramp),
            ["checker", cell] | ["checker", cell, _] => {
                let cell = cell
                    .parse()
                    .context("checker cell must be a positive integer")?;
                ensure!(cell > 0, "checker cell must be positive");
                let phase = parts
                    .get(2)
                    .map_or(Ok(0), |phase| phase.parse())
                    .context("checker phase must be a nonnegative integer")?;
                Ok(Self::Checker { cell, phase })
            }
            _ => {
                bail!(
                    "background must be solid:RRGGBB, step:h[:phase], step:v[:phase], ramp, or checker:N[:phase]"
                )
            }
        }
    }

    fn name(&self) -> String {
        match self {
            Self::Solid(color) => format!("solid-{color:06x}"),
            Self::Step { vertical, phase } => {
                let axis = if *vertical { "v" } else { "h" };
                if *phase == 0.0 {
                    format!("step-{axis}")
                } else {
                    format!("step-{axis}-phase{phase}")
                }
            }
            Self::Ramp => "ramp".into(),
            Self::Checker { cell, phase } => format!("checker-{cell}-phase{phase}"),
        }
    }

    fn validate_scale(&self, scale: f32) -> Result<()> {
        if let Self::Step { vertical, phase } = self {
            ensure!(
                (step_split_logical(*vertical, *phase) * scale).fract() == 0.0,
                "step boundary cannot be represented exactly at display scale {scale}"
            );
        }
        Ok(())
    }

    fn pixel(
        &self,
        [x, y]: [u32; 2],
        [width, height]: [u32; 2],
        scale: f32,
        dark: bool,
    ) -> [u8; 4] {
        let rgb = match self {
            Self::Solid(color) => {
                let [_, r, g, b] = color.to_be_bytes();
                [r, g, b]
            }
            Self::Step { vertical, phase } => {
                let coordinate = if *vertical { x } else { y };
                let split = step_split_logical(*vertical, *phase) * scale;
                let light = f64::from(coordinate) >= f64::from(split);
                [if light { 255 } else { 0 }; 3]
            }
            Self::Ramp => [
                (f64::from(x) * 255.0 / f64::from(width - 1)).round() as u8,
                (f64::from(y) * 255.0 / f64::from(height - 1)).round() as u8,
                128,
            ],
            Self::Checker { cell, phase } => {
                let column = ((f64::from(x) + 0.5) / f64::from(scale)).floor() as u64;
                let row = ((f64::from(y) + 0.5) / f64::from(scale)).floor() as u64;
                let parity = ((column + u64::from(*phase)) / u64::from(*cell)
                    + (row + u64::from(*phase)) / u64::from(*cell))
                    % 2;
                palette(dark)[parity as usize]
            }
        };
        [rgb[0], rgb[1], rgb[2], 255]
    }

    fn metadata(&self, dark: bool) -> String {
        match self {
            Self::Solid(color) => {
                let [_, r, g, b] = color.to_be_bytes();
                format!(r#"{{"kind":"solid","srgb":[{r},{g},{b}]}}"#)
            }
            Self::Step { vertical, phase } => {
                let split = step_split_logical(*vertical, *phase);
                let extent = if *vertical { WIDTH } else { HEIGHT };
                let fraction = f64::from(split) / f64::from(extent);
                format!(
                    r#"{{"kind":"step","edge":"{}","low_srgb":[0,0,0],"high_srgb":[255,255,255],"phase_logical":{phase},"split_logical":{split},"split_fraction":{fraction}}}"#,
                    if *vertical { "vertical" } else { "horizontal" }
                )
            }
            Self::Ramp => r#"{"kind":"ramp","red":"x/(device_width-1)","green":"y/(device_height-1)","blue":128,"encoding":"sRGB code values, rounded to nearest 8-bit value"}"#.into(),
            Self::Checker { cell, phase } => format!(
                r#"{{"kind":"checker","cell_logical":{cell},"phase_logical":{phase},"colors_srgb":{:?}}}"#,
                palette(dark)
            ),
        }
    }
}

fn step_split_logical(vertical: bool, phase: f32) -> f32 {
    (if vertical { WIDTH } else { HEIGHT }) / 2.0 + phase
}

fn palette(dark: bool) -> [[u8; 3]; 2] {
    if dark {
        [[0x16, 0x2e, 0x46], [0x74, 0x42, 0x3a]]
    } else {
        [[0x76, 0xbc, 0xed], [0xf1, 0xa6, 0x84]]
    }
}

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
    pub(super) dark: bool,
    pub(super) clear: bool,
}

impl Probe {
    pub(super) fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let (mut background, mut shape, mut dark, mut clear) = (None, None, None, None);
        let mut dimensions = None;
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
        Ok(Self {
            background: background.context("probe requires --background")?,
            shape,
            dimensions,
            dark: dark.context("probe requires --appearance")?,
            clear: clear.context("probe requires --style")?,
        })
    }

    pub(super) fn name(&self) -> String {
        let size = if self.dimensions == self.shape.default_size() {
            String::new()
        } else {
            format!("-{}x{}", self.dimensions[0], self.dimensions[1])
        };
        format!(
            "probe-{}-{}-{}{size}-{}",
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
            (WIDTH - width) / 2.0,
            (HEIGHT - height) / 2.0,
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
        let [x, y, w, h, radius] = self.geometry();
        let metadata = format!(
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
  "captures": ["{name}-capture0.png", "{name}-capture1.png"]
}}
"#,
            if self.dark { "dark" } else { "light" },
            if self.clear { "clear" } else { "regular" },
            self.background.metadata(self.dark),
            titlebar_height * scale,
            self.shape.name()
        );
        println!("{metadata}");
        std::fs::write(directory.join(format!("{name}.json")), metadata)
            .context("saving probe parameters")
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
