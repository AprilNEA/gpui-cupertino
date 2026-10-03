// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

//! Pixel regressions for the native Metal backdrop path on an opaque surface.
#![cfg(target_os = "macos")]

use gpui::{
    Backdrop, Bounds, ContentMask, DevicePixels, PlatformHeadlessRenderer, Quad, ScaledPixels,
    Scene, point, rgb, size,
};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;

fn bounds([x, y, width, height]: [f32; 4], scale: u32) -> Bounds<ScaledPixels> {
    let scaled = |value| ScaledPixels(value * scale as f32);
    Bounds::new(
        point(scaled(x), scaled(y)),
        size(scaled(width), scaled(height)),
    )
}

fn quad(scene: &mut Scene, bounds: Bounds<ScaledPixels>, color: u32) {
    scene.insert_primitive(Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: rgb(color).into(),
        ..Default::default()
    });
}

fn backdrop(bounds: Bounds<ScaledPixels>, scale: u32) -> Backdrop {
    Backdrop {
        order: 0,
        shape: 0,
        bounds,
        content_mask: ContentMask { bounds },
        corner_radius: ScaledPixels(0.0),
        blur_sigma: ScaledPixels(0.0),
        refraction_amount: ScaledPixels(0.0),
        refraction_width: ScaledPixels(scale as f32),
        dispersion: ScaledPixels(0.0),
        tint: [0.0; 4],
        saturation: 1.0,
        brightness: 0.0,
        direction_mix: 0.0,
        highlight: 0.0,
        edge_bleed: 0.0,
        scale_factor: scale as f32,
        opacity: 1.0,
    }
}

fn render(
    renderer: &mut MetalHeadlessRenderer,
    mut scene: Scene,
    [width, height]: [u32; 2],
) -> Vec<[u8; 4]> {
    scene.finish();
    renderer
        .render_scene_to_image(
            &scene,
            size(DevicePixels(width as i32), DevicePixels(height as i32)),
        )
        .expect("a working Metal device and compiled GPUI shaders are required")
        .pixels()
        .map(|pixel| pixel.0)
        .collect()
}

fn grid_scene([width, height]: [u32; 2], scale: u32, phase: usize, glass: bool) -> Scene {
    let mut scene = Scene::default();
    let colors = [0x224466, 0xcc8844, 0x44aa77, 0x663399];
    for y in (0..height).step_by(8) {
        for x in (0..width).step_by(8) {
            let color = colors[((x / 8 + y / 8) as usize + phase) % colors.len()];
            quad(
                &mut scene,
                bounds([x as f32, y as f32, 8.0, 8.0], scale),
                color,
            );
        }
    }
    if glass {
        let mut effect = backdrop(
            bounds([12.0, 12.0, width as f32 - 24.0, 32.0], scale),
            scale,
        );
        effect.content_mask.bounds = bounds([20.0, 16.0, width as f32 - 48.0, 24.0], scale);
        scene.insert_primitive(effect);
    }
    // Thin foreground strokes stand in for content painted after the material.
    for x in [24.0, 28.0, 34.0] {
        quad(&mut scene, bounds([x, 22.0, 1.0, 12.0], scale), 0xffffff);
    }
    scene
}

#[test]
fn identity_copy_preserves_grid_and_foreground_through_scale_clip_and_resize() {
    let mut renderer = MetalHeadlessRenderer::new();
    for scale in [1, 2] {
        for logical_size in [[96, 64], [128, 80], [96, 64]] {
            for phase in [0, 1] {
                let output_size = logical_size.map(|value| value * scale);
                let expected = render(
                    &mut renderer,
                    grid_scene(logical_size, scale, phase, false),
                    output_size,
                );
                let actual = render(
                    &mut renderer,
                    grid_scene(logical_size, scale, phase, true),
                    output_size,
                );
                for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                    assert!(
                        actual
                            .iter()
                            .zip(expected)
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "identity mismatch at pixel {index}, scale {scale}, phase {phase}: {actual:?} != {expected:?}"
                    );
                }
            }
        }
    }
}

fn edge_scene(scale: u32, foreground: bool) -> Scene {
    let mut scene = Scene::default();
    quad(&mut scene, bounds([0.0, 0.0, 64.0, 48.0], scale), 0x000000);
    quad(&mut scene, bounds([32.0, 0.0, 32.0, 48.0], scale), 0xffffff);
    let mut effect = backdrop(bounds([16.0, 8.0, 32.0, 32.0], scale), scale);
    effect.content_mask.bounds = bounds([24.0, 12.0, 20.0, 24.0], scale);
    effect.blur_sigma = ScaledPixels(2.0 * scale as f32);
    scene.insert_primitive(effect);
    if foreground {
        quad(&mut scene, bounds([32.0, 18.0, 1.0, 12.0], scale), 0xff0000);
    }
    scene
}

#[test]
fn blur_reads_only_prior_background_and_obeys_output_clip() {
    let mut renderer = MetalHeadlessRenderer::new();
    for scale in [1, 2] {
        let width = 64 * scale;
        let blurred = render(&mut renderer, edge_scene(scale, false), [width, 48 * scale]);
        let with_content = render(&mut renderer, edge_scene(scale, true), [width, 48 * scale]);
        let at = |x: u32, y: u32| ((y * scale) * width + x * scale) as usize;
        let gray = blurred[at(31, 24)];
        assert!(gray[0] > 0 && gray[0] < 255, "edge did not blur: {gray:?}");
        assert_eq!(blurred[at(20, 24)], [0, 0, 0, 255]);
        assert_eq!(blurred[at(46, 24)], [255; 4]);
        assert_eq!(blurred[at(31, 10)], [0, 0, 0, 255]);
        for x in 26..38 {
            assert!(blurred[at(x, 24)][0] <= blurred[at(x + 1, 24)][0]);
        }
        for y in 0..48 * scale {
            for x in 0..width {
                let index = (y * width + x) as usize;
                if (32 * scale..33 * scale).contains(&x) && (18 * scale..30 * scale).contains(&y) {
                    assert_eq!(with_content[index], [255, 0, 0, 255]);
                } else {
                    assert_eq!(
                        with_content[index], blurred[index],
                        "foreground leaked into its own backdrop at ({x},{y})"
                    );
                }
            }
        }
    }
}

#[test]
fn blur_preserves_constant_fields_and_spreads_an_impulse_symmetrically() {
    let mut renderer = MetalHeadlessRenderer::new();
    for scale in [1, 2] {
        let frame = bounds([0.0, 0.0, 64.0, 48.0], scale);
        let mut effect = backdrop(frame, scale);
        effect.blur_sigma = ScaledPixels(2.0 * scale as f32);
        let mut constant = Scene::default();
        quad(&mut constant, frame, 0x4080c0);
        constant.insert_primitive(effect);
        let output_size = [64 * scale, 48 * scale];
        let image = render(&mut renderer, constant, output_size);
        for pixel in image {
            assert!(
                pixel
                    .into_iter()
                    .zip([64, 128, 192, 255])
                    .all(|(a, b)| a.abs_diff(b) <= 1),
                "constant field drifted: {pixel:?}"
            );
        }

        let mut impulse = Scene::default();
        quad(&mut impulse, frame, 0x000000);
        quad(
            &mut impulse,
            bounds([32.0, 0.0, 1.0, 48.0], scale),
            0xffffff,
        );
        impulse.insert_primitive(effect);
        let image = render(&mut renderer, impulse, output_size);
        let at = |x: u32| image[(24 * scale * output_size[0] + x) as usize][0];
        assert!(at(32 * scale) > 0 && at(32 * scale) < 255);
        assert!(at(31 * scale) > 0, "impulse did not spread");
        for offset in 0..6 * scale {
            let left = at(32 * scale - offset);
            let right = at(33 * scale - 1 + offset);
            assert!(
                left.abs_diff(right) <= 2,
                "asymmetric impulse at scale {scale}, offset {offset}: {left} != {right}"
            );
        }
    }
}

#[test]
fn opaque_tint_replaces_background_without_changing_outside_pixels() {
    let mut renderer = MetalHeadlessRenderer::new();
    for shape in [0, 1, 2] {
        let mut scene = Scene::default();
        quad(&mut scene, bounds([0.0, 0.0, 48.0, 32.0], 1), 0x0000ff);
        let mut effect = backdrop(bounds([8.0, 8.0, 32.0, 16.0], 1), 1);
        effect.shape = shape;
        effect.corner_radius = ScaledPixels(6.0);
        effect.tint = [0.25, 0.5, 0.75, 1.0];
        scene.insert_primitive(effect);
        let image = render(&mut renderer, scene, [48, 32]);
        assert!(
            image[16 * 48 + 24]
                .into_iter()
                .zip([137, 188, 225, 255])
                .all(|(a, b)| a.abs_diff(b) <= 1),
            "linear tint was not encoded to sRGB: {:?}",
            image[16 * 48 + 24]
        );
        assert_eq!(image[16 * 48 + 4], [0, 0, 255, 255]);
        assert_eq!(image[8 * 48 + 8], [0, 0, 255, 255]);
        for y in 8..24 {
            for x in 8..24 {
                assert!(
                    image[y * 48 + x]
                        .iter()
                        .zip(image[y * 48 + 47 - x])
                        .all(|(a, b)| a.abs_diff(b) <= 1),
                    "asymmetric outline for shape {shape} at ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn optical_controls_displace_split_and_highlight_background_edges() {
    let mut renderer = MetalHeadlessRenderer::new();
    let base = backdrop(bounds([8.0, 8.0, 48.0, 32.0], 1), 1);
    let scene = |effect: Backdrop| {
        let mut scene = Scene::default();
        for x in (0..64).step_by(4) {
            quad(
                &mut scene,
                bounds([x as f32, 0.0, 4.0, 48.0], 1),
                if x % 8 == 0 { 0 } else { 0xffffff },
            );
        }
        scene.insert_primitive(effect);
        scene
    };
    let reference = render(&mut renderer, scene(base), [64, 48]);
    let refracted = render(
        &mut renderer,
        scene(Backdrop {
            refraction_amount: ScaledPixels(6.0),
            refraction_width: ScaledPixels(8.0),
            direction_mix: 0.5,
            ..base
        }),
        [64, 48],
    );
    assert!(
        refracted
            .iter()
            .zip(&reference)
            .filter(|(a, b)| a[0].abs_diff(b[0]) > 16)
            .count()
            > 20,
        "refraction did not move the coordinate stripes"
    );
    let dispersed = render(
        &mut renderer,
        scene(Backdrop {
            dispersion: ScaledPixels(4.0),
            refraction_width: ScaledPixels(8.0),
            ..base
        }),
        [64, 48],
    );
    assert!(
        dispersed.iter().filter(|p| p[0].abs_diff(p[2]) > 8).count() > 10,
        "spectral sampling did not separate red and blue at grayscale edges"
    );
    let highlighted = render(
        &mut renderer,
        scene(Backdrop {
            highlight: 1.0,
            ..base
        }),
        [64, 48],
    );
    assert!(
        highlighted
            .iter()
            .zip(&reference)
            .filter(|(a, b)| a[0] > b[0].saturating_add(8))
            .count()
            > 5,
        "directional highlight did not brighten the outline"
    );
    for y in 0..48 {
        for x in 0..64 {
            if !(8..56).contains(&x) || !(8..40).contains(&y) {
                let index = y * 64 + x;
                for actual in [&refracted, &dispersed, &highlighted] {
                    assert_eq!(
                        actual[index], reference[index],
                        "optical effect escaped its outline at ({x},{y})"
                    );
                }
            }
        }
    }
}

#[test]
fn edge_bleed_reads_neighbor_color_and_preserves_interior_and_constant_fields() {
    let mut renderer = MetalHeadlessRenderer::new();
    let scene = |edge_bleed, background, neighbor| {
        let mut scene = Scene::default();
        quad(&mut scene, bounds([0.0, 0.0, 64.0, 48.0], 1), background);
        quad(&mut scene, bounds([8.0, 0.0, 8.0, 48.0], 1), neighbor);
        scene.insert_primitive(Backdrop {
            edge_bleed,
            refraction_width: ScaledPixels(8.0),
            ..backdrop(bounds([16.0, 8.0, 32.0, 32.0], 1), 1)
        });
        scene
    };
    let original = render(&mut renderer, scene(0.0, 0x000000, 0xff0000), [64, 48]);
    let bleeding = render(&mut renderer, scene(1.0, 0x000000, 0xff0000), [64, 48]);
    assert_eq!(original[24 * 64 + 16], [0, 0, 0, 255]);
    let edge = bleeding[24 * 64 + 16];
    assert!(
        edge[0] > 32 && edge[1] == 0 && edge[2] == 0,
        "neighbor color did not enter inner edge: {edge:?}"
    );
    assert_eq!(bleeding[24 * 64 + 32], original[24 * 64 + 32]);
    assert_eq!(bleeding[24 * 64 + 12], original[24 * 64 + 12]);
    let constant = render(&mut renderer, scene(1.0, 0x306090, 0x306090), [64, 48]);
    assert!(
        constant.iter().all(|pixel| pixel
            .iter()
            .zip([48, 96, 144, 255])
            .all(|(a, b)| a.abs_diff(b) <= 1)),
        "edge bleed changed a constant field"
    );
}

#[test]
fn highlight_keeps_its_logical_width_on_retina() {
    let mut renderer = MetalHeadlessRenderer::new();
    let energy = [1, 2].map(|scale| {
        let mut scene = Scene::default();
        quad(&mut scene, bounds([0.0, 0.0, 64.0, 48.0], scale), 0);
        scene.insert_primitive(Backdrop {
            highlight: 1.0,
            ..backdrop(bounds([8.0, 8.0, 48.0, 32.0], scale), scale)
        });
        let image = render(&mut renderer, scene, [64 * scale, 48 * scale]);
        let mut sum = 0.0_f32;
        for y in 8 * scale..12 * scale {
            for x in 24 * scale..40 * scale {
                let c = f32::from(image[(y * 64 * scale + x) as usize][0]) / 255.0;
                sum += if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                };
            }
        }
        sum / (scale * scale) as f32
    });
    // A 1.5 logical-pixel band is discretized differently at 1x and 2x. The
    // tolerance permits that sampling difference but rejects a halved Retina width.
    assert!(
        energy[0] > 0.0 && (0.75..1.25).contains(&(energy[1] / energy[0])),
        "highlight changed logical width: 1x={}, 2x={}",
        energy[0],
        energy[1]
    );
}
