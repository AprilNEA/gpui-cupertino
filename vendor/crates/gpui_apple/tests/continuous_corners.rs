// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

//! Native edge samples and scene regressions for continuous glass outlines.
#![cfg(target_os = "macos")]

#[path = "fixtures/continuous_corners.rs"]
mod native;
#[path = "support/render.rs"]
mod support;

use gpui::{Backdrop, ContentMask, Corners, Edges, Primitive, Quad, ScaledPixels, Scene, rgb};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use support::{encoded, linear, material, quad, rect, render};

#[test]
fn native_edges_and_opaque_surfaces_share_continuous_geometry_at_both_scales() {
    let mut renderer = MetalHeadlessRenderer::new();
    for &([width, height, radius, face], samples) in native::SAMPLES {
        for scale in [1, 2] {
            let frame = rect([0.0, 0.0, (384 * scale) as f32, (320 * scale) as f32]);
            let bounds = rect([
                ((384 - width) * scale) as f32 / 2.0,
                ((320 - height) * scale) as f32 / 2.0,
                (width * scale) as f32,
                (height * scale) as f32,
            ]);
            let color = linear(face as u8) as f32;
            let [glass, opaque] = [false, true].map(|opaque| {
                let mut scene = Scene::default();
                quad(&mut scene, frame, 0);
                if opaque {
                    scene.insert_primitive(Primitive::ContinuousQuad(Quad {
                        bounds,
                        content_mask: ContentMask { bounds: frame },
                        background: rgb(face * 0x010101).into(),
                        corner_radii: Corners::all(ScaledPixels((radius * scale) as f32)),
                        ..Default::default()
                    }));
                } else {
                    scene.insert_primitive(Backdrop {
                        corner_radius: ScaledPixels((radius * scale) as f32),
                        content_mask: ContentMask { bounds: frame },
                        tint: [color, color, color, 1.0],
                        ..material(bounds, scale)
                    });
                }
                render(&mut renderer, scene, [384 * scale, 320 * scale])
            });
            for (index, (a, b)) in glass.iter().zip(&opaque).enumerate() {
                assert_eq!(a[3], 255);
                assert_eq!(b[3], 255);
                assert!(
                    a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 1),
                    "glass/opaque edge differs: {width}x{height}/r{radius}, scale {scale}, pixel {index}: {a:?}/{b:?}"
                );
            }
            if scale == 2 {
                for &[x, y, expected] in samples {
                    for pixels in [&glass, &opaque] {
                        let actual = pixels[(y * 768 + x) as usize];
                        assert!(
                            actual[..3]
                                .iter()
                                .all(|&channel| channel.abs_diff(expected as u8) <= 1),
                            "native edge differs: {width}x{height}/r{radius} at ({x},{y}): {actual:?}, expected {expected}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn continuous_border_keeps_foreground_order_clip_and_cached_replay() {
    let mut renderer = MetalHeadlessRenderer::new();
    for scale in [1, 2] {
        let scaled = |values: [f32; 4]| rect(values.map(|v| v * scale as f32));
        let frame = scaled([0.0, 0.0, 128.0, 96.0]);
        let outline = scaled([16.0, 16.0, 96.0, 64.0]);
        let make_scene = || {
            let mut scene = Scene::default();
            scene.push_layer(frame);
            quad(&mut scene, frame, 0);
            scene.insert_primitive(Primitive::ContinuousQuad(Quad {
                bounds: outline,
                content_mask: ContentMask {
                    bounds: scaled([24.0, 0.0, 104.0, 96.0]),
                },
                background: rgb(0x474747).into(),
                border_color: rgb(0xffffff).into(),
                border_widths: Edges::all(ScaledPixels(scale as f32)),
                corner_radii: Corners::all(ScaledPixels(20.0 * scale as f32)),
                ..Default::default()
            }));
            quad(&mut scene, scaled([64.0, 30.0, 2.0, 36.0]), 0xffffff);
            scene.pop_layer();
            scene
        };
        let mut cached = make_scene();
        cached.finish();
        let mut replayed = Scene::default();
        replayed.replay(0..cached.len(), &cached);
        let dimensions = [128 * scale, 96 * scale];
        let fresh = render(&mut renderer, make_scene(), dimensions);
        assert_eq!(render(&mut renderer, replayed, dimensions), fresh);
        let at = |x: u32, y: u32| fresh[(y * scale * dimensions[0] + x * scale) as usize];
        assert_eq!(
            at(64, 40),
            [255; 4],
            "continuous batch covered foreground content"
        );
        assert_eq!(
            at(20, 48),
            [0, 0, 0, 255],
            "continuous surface escaped its clip"
        );
        assert!(at(64, 16)[0] >= 254, "contrast border is missing");
        assert_eq!(at(60, 40), [71, 71, 71, 255]);
        cached.clear();
        assert!(cached.continuous_quads.is_empty());
    }
}

#[test]
fn tiny_radii_stay_finite_and_ancestor_opacity_remains_linear() {
    let mut renderer = MetalHeadlessRenderer::new();
    let frame = rect([0.0, 0.0, 64.0, 48.0]);
    let bounds = rect([8.25, 8.25, 48.0, 32.0]);
    let scene = |radius, opacity| {
        let mut scene = Scene::default();
        quad(&mut scene, frame, 0);
        scene.insert_primitive(Backdrop {
            corner_radius: ScaledPixels(radius),
            content_mask: ContentMask { bounds: frame },
            tint: [0.5, 0.5, 0.5, 1.0],
            opacity,
            ..material(bounds, 1)
        });
        scene
    };
    let square = render(&mut renderer, scene(0.0, 1.0), [64, 48]);
    for radius in [f32::from_bits(1), f32::MIN_POSITIVE, 0.0001] {
        let actual = render(&mut renderer, scene(radius, 1.0), [64, 48]);
        assert!(
            actual
                .iter()
                .zip(&square)
                .all(|(a, b)| a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 1)),
            "a subpixel radius lost rectangular coverage: {radius}"
        );
    }
    let actual = render(&mut renderer, scene(12.0, 0.4), [64, 48]);
    let expected = encoded(0.5 * 0.4);
    assert!(
        actual[24 * 64 + 32][..3]
            .iter()
            .all(|&channel| channel.abs_diff(expected) <= 1)
    );
}
