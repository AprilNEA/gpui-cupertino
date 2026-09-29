//! Deterministic background inputs and their capture metadata.

use super::comparison::{HEIGHT, WIDTH};
use anyhow::{Context, Result, bail, ensure};

pub(super) enum Background {
    Solid(u32),
    Step {
        vertical: bool,
        phase: f32,
    },
    Ramp,
    Checker {
        cell: u32,
        phase: u32,
        colors: Option<[u32; 2]>,
    },
}

impl Background {
    pub(super) fn parse(value: &str) -> Result<Self> {
        let parts: Vec<_> = value.split(':').collect();
        match parts.as_slice() {
            ["solid", hex] => Ok(Self::Solid(parse_rgb(hex)?)),
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
            ["checker", cell] | ["checker", cell, _] | ["checker", cell, _, _, _] => {
                let cell = cell
                    .parse()
                    .context("checker cell must be a positive integer")?;
                ensure!(cell > 0, "checker cell must be positive");
                let phase = parts
                    .get(2)
                    .map_or(Ok(0), |phase| phase.parse())
                    .context("checker phase must be a nonnegative integer")?;
                let colors = match parts.as_slice() {
                    [_, _, _, first, second] => Some([parse_rgb(first)?, parse_rgb(second)?]),
                    _ => None,
                };
                Ok(Self::Checker {
                    cell,
                    phase,
                    colors,
                })
            }
            _ => {
                bail!(
                    "background must be solid:RRGGBB, step:h[:phase], step:v[:phase], ramp, checker:N[:phase], or checker:N:phase:RRGGBB:RRGGBB"
                )
            }
        }
    }

    pub(super) fn name(&self) -> String {
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
            Self::Checker {
                cell,
                phase,
                colors,
            } => match colors {
                Some([first, second]) => {
                    format!("checker-{cell}-phase{phase}-{first:06x}-{second:06x}")
                }
                None => format!("checker-{cell}-phase{phase}"),
            },
        }
    }

    pub(super) fn validate_scale(&self, scale: f32) -> Result<()> {
        if let Self::Step { vertical, phase } = self {
            ensure!(
                (step_split_logical(*vertical, *phase) * scale).fract() == 0.0,
                "step boundary cannot be represented exactly at display scale {scale}"
            );
        }
        Ok(())
    }

    pub(super) fn pixel(
        &self,
        [x, y]: [u32; 2],
        [width, height]: [u32; 2],
        scale: f32,
        dark: bool,
    ) -> [u8; 4] {
        let rgb = match self {
            Self::Solid(color) => rgb_bytes(*color),
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
            Self::Checker {
                cell,
                phase,
                colors,
            } => {
                let column = ((f64::from(x) + 0.5) / f64::from(scale)).floor() as u64;
                let row = ((f64::from(y) + 0.5) / f64::from(scale)).floor() as u64;
                let parity = ((column + u64::from(*phase)) / u64::from(*cell)
                    + (row + u64::from(*phase)) / u64::from(*cell))
                    % 2;
                palette(dark, *colors)[parity as usize]
            }
        };
        [rgb[0], rgb[1], rgb[2], 255]
    }

    pub(super) fn metadata(&self, dark: bool) -> String {
        match self {
            Self::Solid(color) => {
                let [r, g, b] = rgb_bytes(*color);
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
            Self::Checker { cell, phase, colors } => format!(
                r#"{{"kind":"checker","cell_logical":{cell},"phase_logical":{phase},"colors_srgb":{:?}}}"#,
                palette(dark, *colors)
            ),
        }
    }
}

fn step_split_logical(vertical: bool, phase: f32) -> f32 {
    (if vertical { WIDTH } else { HEIGHT }) / 2.0 + phase
}

fn parse_rgb(hex: &str) -> Result<u32> {
    ensure!(
        hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "RGB color must have six hexadecimal digits"
    );
    u32::from_str_radix(hex, 16).context("invalid hexadecimal RGB color")
}

fn rgb_bytes(color: u32) -> [u8; 3] {
    let [_, r, g, b] = color.to_be_bytes();
    [r, g, b]
}

fn palette(dark: bool, colors: Option<[u32; 2]>) -> [[u8; 3]; 2] {
    if let Some(colors) = colors {
        colors.map(rgb_bytes)
    } else if dark {
        [[0x16, 0x2e, 0x46], [0x74, 0x42, 0x3a]]
    } else {
        [[0x76, 0xbc, 0xed], [0xf1, 0xa6, 0x84]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_checker_colors_match_phase_pixels_and_metadata() -> Result<()> {
        for (input, colors) in [
            ("checker:32:7:000000:ffffff", [[0, 0, 0], [255, 255, 255]]),
            ("checker:32:7:010203:c0deFF", [[1, 2, 3], [192, 222, 255]]),
        ] {
            let background = Background::parse(input)?;
            for dark in [false, true] {
                for (scale, before, after) in [(1.0, 24, 25), (2.0, 49, 50)] {
                    let dimensions = [(WIDTH * scale) as u32, (HEIGHT * scale) as u32];
                    for (point, color) in [
                        ([before, 0], colors[0]),
                        ([after, 0], colors[1]),
                        ([after, after], colors[0]),
                    ] {
                        assert_eq!(
                            background.pixel(point, dimensions, scale, dark),
                            [color[0], color[1], color[2], 255]
                        );
                    }
                }
            }
        }
        let gray = Background::parse("checker:32:7:000000:FFFFFF")?;
        assert_eq!(gray.name(), "checker-32-phase7-000000-ffffff");
        assert_eq!(
            gray.metadata(false),
            r#"{"kind":"checker","cell_logical":32,"phase_logical":7,"colors_srgb":[[0, 0, 0], [255, 255, 255]]}"#
        );
        assert_eq!(gray.metadata(true), gray.metadata(false));
        let legacy = Background::parse("checker:32")?;
        assert_eq!(legacy.name(), "checker-32-phase0");
        assert_eq!(
            legacy.metadata(false),
            r#"{"kind":"checker","cell_logical":32,"phase_logical":0,"colors_srgb":[[118, 188, 237], [241, 166, 132]]}"#
        );
        assert_eq!(
            legacy.metadata(true),
            r#"{"kind":"checker","cell_logical":32,"phase_logical":0,"colors_srgb":[[22, 46, 70], [116, 66, 58]]}"#
        );
        for input in [
            "checker:32:0:ffffff",
            "checker:32:0:00000:ffffff",
            "checker:32:0:gggggg:ffffff",
            "checker:32:0:+00000:ffffff",
            "checker:0:0:000000:ffffff",
            "checker:32:-1:000000:ffffff",
            "checker:32:0:000000:ffffff:123456",
        ] {
            assert!(Background::parse(input).is_err(), "accepted {input}");
        }
        Ok(())
    }
}
