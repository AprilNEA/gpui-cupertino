//! Native inactive Clear anchors and scene integration regression checks.
#![cfg(target_os = "macos")]

#[path = "fixtures/clear.rs"]
mod fixtures;
#[path = "support/render.rs"]
#[allow(
    dead_code,
    reason = "shared render helpers also serve Gaussian reference tests"
)]
mod support;

use gpui::{
    AtlasKey, ClearBackdrop, ContentMask, DevicePixels, ImageId, PlatformHeadlessRenderer,
    PolychromeSprite, RenderImageParams, ScaledPixels, Scene, WindowAppearance, size,
};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use std::borrow::Cow;

#[test]
fn clear_matches_native_chromatic_spatial_and_edge_anchors() {
    let mut renderer = MetalHeadlessRenderer::new();
    let frame = support::rect([0.0, 0.0, 1536.0, 640.0]);
    for (index, case) in fixtures::CASES.iter().enumerate() {
        let mut source = Vec::with_capacity(768 * 640 * 4);
        for y in 0..640 {
            for x in 0..768 {
                let parity =
                    ((x / 2 + case.phase) / case.cell + (y / 2 + case.phase) / case.cell) % 2;
                let [_, r, g, b] = case.colors[parity as usize].to_be_bytes();
                source.extend_from_slice(&[b, g, r, 255]);
            }
        }
        let tile = renderer
            .sprite_atlas()
            .get_or_insert_with(
                AtlasKey::Image(RenderImageParams {
                    image_id: ImageId(index),
                    frame_index: 0,
                }),
                &mut || {
                    Ok(Some((
                        size(DevicePixels(768), DevicePixels(640)),
                        Cow::Borrowed(&source),
                    )))
                },
            )
            .unwrap()
            .unwrap();
        let mut scene = Scene::default();
        for x in [0.0, 768.0] {
            scene.insert_primitive(PolychromeSprite {
                order: 0,
                pad: 0,
                bounds: support::rect([x, 0.0, 768.0, 640.0]),
                content_mask: ContentMask { bounds: frame },
                corner_radii: Default::default(),
                tile,
                grayscale: false.into(),
                opacity: 1.0,
            });
        }
        scene.insert_primitive(ClearBackdrop {
            order: 0,
            scale_factor: 2.0,
            bounds: support::rect(case.bounds.map(|v| v * 2.0)),
            content_mask: ContentMask { bounds: frame },
            corner_radius: ScaledPixels(case.corner * 2.0),
            blur_radius: ScaledPixels(case.blur * 2.0),
            appearance: if case.dark {
                WindowAppearance::Dark
            } else {
                WindowAppearance::Light
            },
            opacity: 1.0,
        });
        let pixels = support::render(&mut renderer, scene, [1536, 640]);
        for &[x, y, r, g, b] in case.samples {
            let actual = pixels[(y * 1536 + x) as usize];
            for (actual, expected) in actual[..3].iter().zip([r, g, b]) {
                assert!(
                    u32::from(*actual).abs_diff(expected) <= 1,
                    "case {index}, ({x}, {y}): {actual} != {expected}"
                );
            }
        }
        for y in 0..640 {
            for x in 0..768 {
                let offset = (y * 768 + x) * 4;
                let [b, g, r, a] = source[offset..offset + 4] else {
                    unreachable!()
                };
                assert_eq!(
                    pixels[y * 1536 + x + 768],
                    [r, g, b, a],
                    "control changed in case {index}"
                );
            }
        }
    }
}

#[test]
fn small_radii_keep_constant_face_and_clipping_at_both_scales() {
    let mut renderer = MetalHeadlessRenderer::new();
    for scale in [1, 2] {
        let factor = scale as f32;
        let frame = support::rect([0.0, 0.0, 160.0 * factor, 160.0 * factor]);
        for radius in [0.0, 0.1, 1.0, 1.25, 2.0, 10.0] {
            let mut scene = Scene::default();
            support::quad(&mut scene, frame, 0);
            scene.insert_primitive(ClearBackdrop {
                order: 0,
                scale_factor: factor,
                bounds: support::rect([
                    20.0 * factor,
                    20.0 * factor,
                    120.0 * factor,
                    120.0 * factor,
                ]),
                content_mask: ContentMask {
                    bounds: support::rect([0.0, 0.0, 100.0 * factor, 160.0 * factor]),
                },
                corner_radius: ScaledPixels(20.0 * factor),
                blur_radius: ScaledPixels(radius * factor),
                appearance: WindowAppearance::Light,
                opacity: 1.0,
            });
            let pixels = support::render(&mut renderer, scene, [160 * scale, 160 * scale]);
            // The native inactive Clear face on black is 71; the clip excludes the second point.
            assert_eq!(
                pixels[((80 * scale) * (160 * scale) + 80 * scale) as usize],
                [71, 71, 71, 255]
            );
            assert_eq!(
                pixels[((80 * scale) * (160 * scale) + 120 * scale) as usize],
                [0, 0, 0, 255]
            );
        }
    }
}
