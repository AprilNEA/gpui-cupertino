//! Independent refraction-coordinate checks against quantized linear ramps.
#![cfg(target_os = "macos")]

#[path = "support/render.rs"]
mod support;

use gpui::{Backdrop, ScaledPixels, Scene};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use support::{encoded, linear, material, quad, rect, render};

// Fixed before measuring: linear filtering through half storage and RGBA8 output.
const MAX_CODE_ERROR: u8 = 2;

#[derive(Clone, Copy, Debug)]
enum Axis {
    Horizontal,
    Vertical,
}

fn sampled_ramp(ramp: &[u8], coordinate: f64) -> u8 {
    let index = (coordinate - 0.5).clamp(0.0, (ramp.len() - 1) as f64);
    let left = index.floor() as usize;
    let right = (left + 1).min(ramp.len() - 1);
    let fraction = index.fract();
    encoded(linear(ramp[left]) * (1.0 - fraction) + linear(ramp[right]) * fraction)
}

#[test]
fn signed_refraction_samples_the_expected_source_coordinate() {
    let mut renderer = MetalHeadlessRenderer::new();
    let mut failures = Vec::new();
    for scale in [1_u32, 2] {
        let dimensions = [129 * scale + 1, 113 * scale + 1];
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let extent = match axis {
                Axis::Horizontal => dimensions[0],
                Axis::Vertical => dimensions[1],
            };
            let ramp: Vec<_> = (0..extent)
                .map(|i| encoded(f64::from(i) / f64::from(extent - 1)))
                .collect();
            for width in [4.0_f32, 12.0] {
                for amount in [-8.0_f32, 8.0] {
                    let mut scene = Scene::default();
                    for (i, value) in ramp.iter().copied().enumerate() {
                        let stripe = match axis {
                            Axis::Horizontal => [i as f32, 0.0, 1.0, dimensions[1] as f32],
                            Axis::Vertical => [0.0, i as f32, dimensions[0] as f32, 1.0],
                        };
                        quad(&mut scene, rect(stripe), u32::from(value) * 0x010101);
                    }
                    scene.insert_primitive(Backdrop {
                        refraction_amount: ScaledPixels(amount * scale as f32),
                        refraction_width: ScaledPixels(width * scale as f32),
                        ..material(
                            rect([24.0, 24.0, 80.0, 64.0].map(|v| v * scale as f32)),
                            scale,
                        )
                    });
                    let image = render(&mut renderer, scene, dimensions);
                    let mut max_error = 0;
                    let mut max_shift = 0;
                    for normal in [-1.0_f64, 1.0] {
                        for offset in [0, scale, 3 * scale, 7 * scale, 13 * scale] {
                            let end = match axis {
                                Axis::Horizontal => 104,
                                Axis::Vertical => 88,
                            };
                            let along = if normal < 0.0 {
                                24 * scale + offset
                            } else {
                                end * scale - 1 - offset
                            };
                            let [x, y] = match axis {
                                Axis::Horizontal => [along, 56 * scale],
                                Axis::Vertical => [64 * scale, along],
                            };
                            // A straight edge has a known normal and distance; no SDF
                            // or texture-coordinate calculation is imported from the GPU.
                            let depth = f64::from(offset) + 0.5;
                            let fraction = (depth / (f64::from(width) * f64::from(scale))).min(1.0);
                            let displacement = f64::from(amount)
                                * f64::from(scale)
                                * (1.0 - (fraction * (2.0 - fraction)).sqrt());
                            let coordinate = f64::from(along) + 0.5 + normal * displacement;
                            let expected = sampled_ramp(&ramp, coordinate);
                            let actual = image[(y * dimensions[0] + x) as usize];
                            let error = actual[..3]
                                .iter()
                                .map(|channel| channel.abs_diff(expected))
                                .max()
                                .unwrap();
                            max_error = max_error.max(error);
                            max_shift = max_shift.max(expected.abs_diff(ramp[along as usize]));
                            if error > MAX_CODE_ERROR || actual[3] != 255 {
                                failures.push(format!(
                                    "scale={scale} axis={axis:?} width={width} amount={amount} normal={normal} depth={depth}: coordinate={coordinate:.6} actual={actual:?} expected={expected} error={error}"
                                ));
                            }
                        }
                    }
                    println!(
                        "Refraction scale={scale} axis={axis:?} width={width} amount={amount}: max_code_error={max_error} max_reference_shift={max_shift}"
                    );
                    assert!(
                        max_shift > MAX_CODE_ERROR * 2,
                        "ramp must expose missing refraction"
                    );
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Refraction coordinate mismatches:\n{}",
        failures.join("\n")
    );
}
