//! Export constant-color continuous outlines for geometry comparisons.

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use anyhow::{Context, ensure};
    use gpui::{
        Backdrop, Bounds, ContentMask, Corners, DevicePixels, PlatformHeadlessRenderer, Primitive,
        Quad, ScaledPixels, Scene, point, rgb, size,
    };
    use gpui_apple::metal_renderer::MetalHeadlessRenderer;
    use std::{env, fs, path::PathBuf};

    let mut args = env::args_os().skip(1);
    let output = PathBuf::from(args.next().context("usage: contours OUTPUT_DIRECTORY")?);
    ensure!(args.next().is_none(), "usage: contours OUTPUT_DIRECTORY");
    fs::create_dir(&output)?;
    let mut renderer = MetalHeadlessRenderer::new();
    for (width, height, radius, shape) in [
        (256, 128, 20, 0),
        (256, 128, 64, 1),
        (128, 128, 64, 2),
        (336, 72, 20, 0),
        (200, 64, 32, 1),
        (96, 96, 48, 2),
        (48, 128, 20, 0),
        (128, 48, 20, 0),
        (48, 48, 20, 0),
    ] {
        for scale in [1, 2] {
            let scaled = |value: f32| ScaledPixels(value * scale as f32);
            let frame = Bounds::new(
                point(scaled(0.0), scaled(0.0)),
                size(scaled(384.0), scaled(320.0)),
            );
            let bounds = Bounds::new(
                point(
                    scaled((384 - width) as f32 / 2.0),
                    scaled((320 - height) as f32 / 2.0),
                ),
                size(scaled(width as f32), scaled(height as f32)),
            );
            for face in [25_u32, 71] {
                let code = face as f32 / 255.0;
                let linear = if code <= 0.04045 {
                    code / 12.92
                } else {
                    ((code + 0.055) / 1.055).powf(2.4)
                };
                for opaque in [false, true] {
                    let mut scene = Scene::default();
                    scene.insert_primitive(Quad {
                        bounds: frame,
                        content_mask: ContentMask { bounds: frame },
                        background: rgb(0).into(),
                        ..Default::default()
                    });
                    if opaque {
                        scene.insert_primitive(Primitive::ContinuousQuad(Quad {
                            bounds,
                            content_mask: ContentMask { bounds: frame },
                            corner_radii: Corners::all(scaled(radius as f32)),
                            background: rgb(face * 0x010101).into(),
                            ..Default::default()
                        }));
                    } else {
                        scene.insert_primitive(Backdrop {
                            order: 0,
                            scale_factor: scale as f32,
                            shape,
                            bounds,
                            content_mask: ContentMask { bounds: frame },
                            corner_radius: scaled(radius as f32),
                            blur_sigma: scaled(0.0),
                            tint: [linear, linear, linear, 1.0],
                            saturation: 1.0,
                            brightness: 0.0,
                            refraction_amount: scaled(0.0),
                            refraction_width: scaled(1.0),
                            direction_mix: 0.0,
                            dispersion: scaled(0.0),
                            highlight: 0.0,
                            edge_bleed: 0.0,
                            opacity: 1.0,
                        });
                    }
                    scene.finish();
                    let mode = if opaque { "opaque" } else { "glass" };
                    renderer
                        .render_scene_to_image(
                            &scene,
                            size(DevicePixels(384 * scale), DevicePixels(320 * scale)),
                        )?
                        .save(output.join(format!(
                            "{mode}-{width}x{height}-r{radius}-c{face}-s{scale}.png"
                        )))?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("contour export requires macOS and Metal");
}
