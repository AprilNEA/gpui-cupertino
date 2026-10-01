//! Independent SDR reference checks, not comparisons with Apple's private shaders.
//!
//! Captured pixels represent constant unit cells. The reference convolves those
//! cells with a continuous Gaussian and samples at destination pixel centers.
//! Fixed tolerances allow RGBA8 readback and half-float storage; they are selected
//! before measuring the implementation, not fitted to its scale-bank algorithm.
//! Set `CUPERTINO_REFERENCE_OUTPUT` to an absolute CSV file path to export representative
//! horizontal impulse and step profiles for plotting.
#![cfg(target_os = "macos")]

#[path = "support/render.rs"]
mod support;

use gpui::{Backdrop, ContentMask, ScaledPixels, Scene};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use std::{
    fs::File,
    io::{BufWriter, Write},
};
use support::{encoded, linear, material, quad, rect, render};

const ENERGY_ERROR: f64 = 0.015;
const CENTROID_ERROR: f64 = 0.05;
const VARIANCE_ERROR: f64 = 0.05;
const KERNEL_L1_ERROR: f64 = 0.10;
const STEP_ERROR: f64 = 0.015;

// Composite Simpson integration of the normalized normal PDF. No shader kernel,
// sampling stride, LOD, scale variance, or implementation coefficients are reused.
fn normal_integral(start: f64, end: f64) -> f64 {
    let step = (end - start) / 64.0;
    let pdf = |x: f64| (-0.5 * x * x).exp() / std::f64::consts::TAU.sqrt();
    let mut sum = pdf(start) + pdf(end);
    for i in 1..64 {
        sum += if i % 2 == 0 { 2.0 } else { 4.0 } * pdf(start + f64::from(i) * step);
    }
    sum * step / 3.0
}

fn gaussian_cell(offset: f64, sigma: f64) -> f64 {
    normal_integral((offset - 0.5) / sigma, (offset + 0.5) / sigma)
}

fn normal_cdf(value: f64) -> f64 {
    if value.abs() >= 8.0 {
        return if value < 0.0 { 0.0 } else { 1.0 };
    }
    0.5 + value.signum() * normal_integral(0.0, value.abs())
}

#[derive(Clone, Copy, Debug)]
enum Axis {
    Horizontal,
    Vertical,
}

enum Signal {
    Impulse,
    Step,
}

#[derive(Clone, Copy)]
struct ProfileCase {
    extent: u32,
    scale: u32,
    axis: Axis,
    center: u32,
    sigma: f32,
}

impl ProfileCase {
    fn scene(self, signal: Signal) -> (Scene, [u32; 2]) {
        let Self {
            extent,
            scale,
            axis,
            center,
            sigma,
        } = self;
        let extent = extent * scale + scale - 1;
        let narrow = 35 * scale + scale - 1;
        let dimensions = match axis {
            Axis::Horizontal => [extent, narrow],
            Axis::Vertical => [narrow, extent],
        };
        let frame = rect([0.0, 0.0, dimensions[0] as f32, dimensions[1] as f32]);
        let mut scene = Scene::default();
        quad(&mut scene, frame, 0);
        let width = match signal {
            Signal::Step => extent - center,
            Signal::Impulse => 1,
        };
        let stripe = match axis {
            Axis::Horizontal => rect([center as f32, 0.0, width as f32, narrow as f32]),
            Axis::Vertical => rect([0.0, center as f32, narrow as f32, width as f32]),
        };
        quad(&mut scene, stripe, 0xffffff);
        scene.insert_primitive(Backdrop {
            blur_sigma: ScaledPixels(sigma * scale as f32),
            ..material(frame, scale)
        });
        (scene, dimensions)
    }
}

fn profile(image: &[[u8; 4]], [width, height]: [u32; 2], axis: Axis) -> Vec<f64> {
    match axis {
        Axis::Horizontal => (0..width)
            .map(|x| linear(image[(height / 2 * width + x) as usize][0]))
            .collect(),
        Axis::Vertical => (0..height)
            .map(|y| linear(image[(y * width + width / 2) as usize][0]))
            .collect(),
    }
}

fn moments(values: &[f64]) -> (f64, f64, f64) {
    let mass: f64 = values.iter().sum();
    let center = values
        .iter()
        .enumerate()
        .map(|(x, value)| x as f64 * value)
        .sum::<f64>()
        / mass;
    let variance = values
        .iter()
        .enumerate()
        .map(|(x, value)| (x as f64 - center).powi(2) * value)
        .sum::<f64>()
        / mass;
    (mass, center, variance)
}

fn export_profile(
    output: &mut Option<BufWriter<File>>,
    case: ProfileCase,
    signal: Signal,
    samples: (&[f64], &[f64]),
) {
    if case.extent != 257
        || case.scale != 1
        || case.center != 128
        || !matches!(case.axis, Axis::Horizontal)
        || ![0.5, 2.0, 8.0].contains(&case.sigma)
    {
        return;
    }
    if let Some(output) = output {
        let signal = match signal {
            Signal::Impulse => "impulse",
            Signal::Step => "step",
        };
        for (x, (actual, reference)) in samples.0.iter().zip(samples.1).enumerate() {
            writeln!(
                output,
                "{signal},{},{x},{actual:.12},{reference:.12}",
                case.sigma
            )
            .expect("write numerical reference CSV row");
        }
    }
}

#[test]
fn gaussian_impulses_and_steps_match_an_integrated_reference() {
    assert!((normal_integral(-1.0, 1.0) - 0.682_689_492_137).abs() < 1e-7);
    let mut renderer = MetalHeadlessRenderer::new();
    let mut failures = Vec::new();
    let mut csv = std::env::var_os("CUPERTINO_REFERENCE_OUTPUT").map(|path| {
        let mut output =
            BufWriter::new(File::create(path).expect("create numerical reference CSV"));
        writeln!(output, "signal,sigma,x,actual_linear,reference_linear")
            .expect("write numerical reference CSV header");
        output
    });
    for sigma in [0.5, 1.0, 2.0, 4.0, 8.0] {
        for scale in [1, 2] {
            for axis in [Axis::Horizontal, Axis::Vertical] {
                for extent in [257, 263] {
                    let pixels = extent * scale + scale - 1;
                    for phase in [0, 1, 3] {
                        let center = pixels / 2 + phase;
                        let case = ProfileCase {
                            extent,
                            scale,
                            axis,
                            center,
                            sigma,
                        };
                        let (scene, dimensions) = case.scene(Signal::Impulse);
                        let actual =
                            profile(&render(&mut renderer, scene, dimensions), dimensions, axis);
                        let reference: Vec<_> = (0..pixels)
                            .map(|x| {
                                gaussian_cell(
                                    f64::from(x) - f64::from(center),
                                    f64::from(sigma) * f64::from(scale),
                                )
                            })
                            .collect();
                        export_profile(&mut csv, case, Signal::Impulse, (&actual, &reference));
                        let (mass, centroid, variance) = moments(&actual);
                        let (reference_mass, reference_centroid, reference_variance) =
                            moments(&reference);
                        let energy = (mass / reference_mass - 1.0).abs();
                        let drift = (centroid - reference_centroid).abs();
                        let variance_error = (variance / reference_variance - 1.0).abs();
                        let l1: f64 = actual
                            .iter()
                            .zip(&reference)
                            .map(|(a, b)| (a / mass - b / reference_mass).abs())
                            .sum();
                        let metrics = format!(
                            "sigma={sigma} scale={scale} axis={axis:?} extent={pixels} phase={phase}: energy={energy:.5} centroid={drift:.5} variance={variance_error:.5} L1={l1:.5}"
                        );
                        println!("Gaussian {metrics}");
                        if !(energy <= ENERGY_ERROR
                            && drift <= CENTROID_ERROR
                            && variance_error <= VARIANCE_ERROR
                            && l1 <= KERNEL_L1_ERROR)
                        {
                            failures.push(metrics);
                        }
                    }
                    let center = pixels / 2;
                    let case = ProfileCase {
                        extent,
                        scale,
                        axis,
                        center,
                        sigma,
                    };
                    let (scene, dimensions) = case.scene(Signal::Step);
                    let actual =
                        profile(&render(&mut renderer, scene, dimensions), dimensions, axis);
                    let reference: Vec<_> = actual
                        .iter()
                        .enumerate()
                        .map(|(x, _)| {
                            normal_cdf(
                                (x as f64 + 0.5 - f64::from(center))
                                    / (f64::from(sigma) * f64::from(scale)),
                            )
                        })
                        .collect();
                    export_profile(&mut csv, case, Signal::Step, (&actual, &reference));
                    let max_error = actual
                        .iter()
                        .zip(&reference)
                        .map(|(actual, reference)| (actual - reference).abs())
                        .fold(0.0_f64, f64::max);
                    let metrics = format!(
                        "sigma={sigma} scale={scale} axis={axis:?} extent={pixels}: max_linear_error={max_error:.5}"
                    );
                    println!("Step {metrics}");
                    if max_error > STEP_ERROR {
                        failures.push(metrics);
                    }
                }
            }
        }
    }
    if let Some(mut csv) = csv {
        csv.flush().expect("flush numerical reference CSV");
    }
    assert!(
        failures.is_empty(),
        "Gaussian reference mismatches:\n{}",
        failures.join("\n")
    );
}

#[test]
fn color_and_opacity_compose_once_in_linear_light() {
    let mut renderer = MetalHeadlessRenderer::new();
    let source = [32, 96, 192].map(linear);
    let luminance = source[0] * 0.2126 + source[1] * 0.7152 + source[2] * 0.0722;
    let mut max_error = 0;
    for saturation in [0.0, 1.0, 1.8] {
        for tint_alpha in [0.0, 0.4, 1.0] {
            for opacity in [0.0, 0.25, 0.5, 1.0] {
                let mut scene = Scene::default();
                quad(&mut scene, rect([0.0, 0.0, 33.0, 31.0]), 0x2060c0);
                scene.insert_primitive(Backdrop {
                    tint: [0.1, 0.3, 0.7, tint_alpha],
                    saturation,
                    brightness: 0.07,
                    opacity,
                    ..material(rect([8.0, 8.0, 16.0, 16.0]), 1)
                });
                let actual = render(&mut renderer, scene, [33, 31])[16 * 33 + 16];
                let mut expected = [0, 0, 0, 255];
                for (channel, tint) in [0.1, 0.3, 0.7].into_iter().enumerate() {
                    let adjusted =
                        luminance + (source[channel] - luminance) * f64::from(saturation) + 0.07;
                    let tinted =
                        adjusted * (1.0 - f64::from(tint_alpha)) + tint * f64::from(tint_alpha);
                    expected[channel] = encoded(
                        source[channel] * (1.0 - f64::from(opacity)) + tinted * f64::from(opacity),
                    );
                }
                let error = actual
                    .into_iter()
                    .zip(expected)
                    .map(|(a, b)| a.abs_diff(b))
                    .max()
                    .unwrap();
                max_error = max_error.max(error);
                println!(
                    "Composition saturation={saturation} tint_alpha={tint_alpha} opacity={opacity}: actual={actual:?} reference={expected:?} max_code_error={error}"
                );
            }
        }
    }
    assert!(
        max_error <= 1,
        "linear composition differs by {max_error} encoded channel levels"
    );
}

#[test]
fn fractional_straight_edges_cover_intersected_pixel_cells() {
    let mut renderer = MetalHeadlessRenderer::new();
    let mut max_error = 0;
    for origin in [8.25_f32, 8.5, 8.75] {
        for opacity in [0.5, 1.0] {
            let frame = rect([0.0, 0.0, 35.0, 33.0]);
            let mut scene = Scene::default();
            quad(&mut scene, frame, 0);
            scene.insert_primitive(Backdrop {
                content_mask: ContentMask { bounds: frame },
                tint: [1.0; 4],
                opacity,
                ..material(rect([origin, 8.0, 16.0, 16.0]), 1)
            });
            let image = render(&mut renderer, scene, [35, 33]);
            let face = 255.0 * (1.055 * f64::from(opacity).powf(1.0 / 2.4) - 0.055);
            for x in 7..27 {
                let coverage = ((x as f64 + 1.0).min(f64::from(origin) + 16.0)
                    - (x as f64).max(f64::from(origin)))
                .clamp(0.0, 1.0);
                let expected = (coverage * face).round() as u8;
                let actual = image[16 * 35 + x][0];
                let error = actual.abs_diff(expected);
                max_error = max_error.max(error);
                if error > 1 {
                    println!(
                        "Coverage x={x} origin={origin} opacity={opacity}: actual={actual} reference={expected} error={error}"
                    );
                }
            }
        }
    }
    println!("Coverage max_code_error={max_error}");
    assert!(
        max_error <= 1,
        "fractional straight-edge coverage differs by {max_error} encoded levels"
    );
}
