//! Deterministic isolated inputs for measuring public AppKit glass.

use super::comparison::{HEIGHT, WIDTH};
use anyhow::{Context, Result, bail, ensure};
use gpui::{DevicePixels, PlatformHeadlessRenderer, Scene, size};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use std::path::Path;

enum Background {
    Solid(u32),
    Step { vertical: bool },
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
            ["step", "v"] => Ok(Self::Step { vertical: true }),
            ["step", "h"] => Ok(Self::Step { vertical: false }),
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
                bail!("background must be solid:RRGGBB, step:h, step:v, ramp, or checker:N[:phase]")
            }
        }
    }

    fn name(&self) -> String {
        match self {
            Self::Solid(color) => format!("solid-{color:06x}"),
            Self::Step { vertical } => format!("step-{}", if *vertical { "v" } else { "h" }),
            Self::Ramp => "ramp".into(),
            Self::Checker { cell, phase } => format!("checker-{cell}-phase{phase}"),
        }
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
            Self::Step { vertical } => {
                let light = if *vertical {
                    x >= width / 2
                } else {
                    y >= height / 2
                };
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
            Self::Step { vertical } => format!(
                r#"{{"kind":"step","edge":"{}","low_srgb":[0,0,0],"high_srgb":[255,255,255],"split_fraction":0.5}}"#,
                if *vertical { "vertical" } else { "horizontal" }
            ),
            Self::Ramp => r#"{"kind":"ramp","red":"x/(device_width-1)","green":"y/(device_height-1)","blue":128,"encoding":"sRGB code values, rounded to nearest 8-bit value"}"#.into(),
            Self::Checker { cell, phase } => format!(
                r#"{{"kind":"checker","cell_logical":{cell},"phase_logical":{phase},"colors_srgb":{:?}}}"#,
                palette(dark)
            ),
        }
    }
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
    pub(super) dark: bool,
    pub(super) clear: bool,
}

impl Probe {
    pub(super) fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let (mut background, mut shape, mut dark, mut clear) = (None, None, None, None);
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
        Ok(Self {
            background: background.context("probe requires --background")?,
            shape: shape.context("probe requires --shape")?,
            dark: dark.context("probe requires --appearance")?,
            clear: clear.context("probe requires --style")?,
        })
    }

    pub(super) fn name(&self) -> String {
        format!(
            "probe-{}-{}-{}-{}",
            if self.dark { "dark" } else { "light" },
            if self.clear { "clear" } else { "regular" },
            self.shape.name(),
            self.background.name()
        )
    }

    pub(super) fn geometry(&self) -> [f32; 5] {
        match self.shape {
            Shape::Roundrect => [64.0, 96.0, 256.0, 128.0, 20.0],
            Shape::Capsule => [64.0, 96.0, 256.0, 128.0, 64.0],
            Shape::Circle => [128.0, 96.0, 128.0, 128.0, 64.0],
        }
    }

    pub(super) fn save_background(
        &self,
        renderer: &mut MetalHeadlessRenderer,
        scale: f32,
        path: &Path,
    ) -> Result<()> {
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
