//! Final-pixel checks for sequential material captures and cached paint replay.
#![cfg(target_os = "macos")]

#[path = "support/render.rs"]
mod support;

use gpui::{Backdrop, Scene};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use support::{encoded, linear, material, quad, rect, render};

const BOUNDS: [[u32; 4]; 2] = [[8, 8, 32, 24], [24, 16, 32, 24]];
const TINTS: [[f32; 4]; 2] = [[0.75, 0.125, 0.25, 0.5], [0.125, 0.5, 0.875, 0.75]];
const OPACITIES: [f32; 2] = [0.75, 0.75];
// Allow half-float capture and two successive RGBA8 stores, fixed before measurement.
const MAX_CODE_ERROR: u8 = 2;

fn tinted(source: [u8; 4], index: usize) -> [u8; 4] {
    let alpha = f64::from(TINTS[index][3]) * f64::from(OPACITIES[index]);
    let mut result = source;
    for channel in 0..3 {
        result[channel] = encoded(
            linear(source[channel]) * (1.0 - alpha) + f64::from(TINTS[index][channel]) * alpha,
        );
    }
    result
}

fn paint_pair(scene: &mut Scene, effects: &[Backdrop; 2], order: [usize; 2]) {
    scene.push_layer(effects[0].bounds.union(&effects[1].bounds));
    for index in order {
        scene.insert_primitive(effects[index]);
    }
    scene.pop_layer();
}

#[test]
fn overlapping_materials_capture_prior_results_in_order_after_background_changes_and_replay() {
    let mut renderer = MetalHeadlessRenderer::new();
    for scale in [1, 2] {
        let dimensions = [64 * scale, 48 * scale];
        let frame = rect([0.0, 0.0, dimensions[0] as f32, dimensions[1] as f32]);
        let effects = std::array::from_fn(|index| Backdrop {
            tint: TINTS[index],
            opacity: OPACITIES[index],
            ..material(rect(BOUNDS[index].map(|v| (v * scale) as f32)), scale)
        });
        for order in [[0, 1], [1, 0]] {
            let mut cached = Scene::default();
            quad(&mut cached, frame, 0xff00ff);
            let start = cached.len();
            paint_pair(&mut cached, &effects, order);
            let end = cached.len();
            cached.finish();

            // Reuse both the renderer and cached operations across changed backgrounds.
            for background in [0x204080_u32, 0xc06020, 0x204080] {
                let scene_with_background = || {
                    let mut scene = Scene::default();
                    quad(&mut scene, frame, background);
                    scene
                };
                let mut fresh = scene_with_background();
                paint_pair(&mut fresh, &effects, order);
                let fresh = render(&mut renderer, fresh, dimensions);

                let mut replayed = scene_with_background();
                replayed.push_layer(frame);
                replayed.replay(start..end, &cached);
                replayed.pop_layer();
                let replayed = render(&mut renderer, replayed, dimensions);
                assert_eq!(
                    replayed, fresh,
                    "replayed captures differ: scale={scale}, order={order:?}, background={background:06x}"
                );

                let [_, red, green, blue] = background.to_be_bytes();
                for y in 0..dimensions[1] {
                    for x in 0..dimensions[0] {
                        let mut expected = [red, green, blue, 255];
                        for index in order {
                            let [left, top, width, height] = BOUNDS[index].map(|v| v * scale);
                            if (left..left + width).contains(&x) && (top..top + height).contains(&y)
                            {
                                // Each capture reads the preceding encoded framebuffer.
                                expected = tinted(expected, index);
                            }
                        }
                        let actual = fresh[(y * dimensions[0] + x) as usize];
                        assert!(
                            actual
                                .into_iter()
                                .zip(expected)
                                .all(|(a, b)| a.abs_diff(b) <= MAX_CODE_ERROR),
                            "sequential capture mismatch at ({x},{y}), scale={scale}, order={order:?}, background={background:06x}: actual={actual:?}, expected={expected:?}"
                        );
                    }
                }
            }
        }
    }
}
