//! End-to-end Metal timing including submission, GPU completion, and CPU readback.

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Instant;

    use gpui::{
        Backdrop, Bounds, ClearBackdrop, ContentMask, DevicePixels, PlatformHeadlessRenderer, Quad,
        ScaledPixels, Scene, WindowAppearance, point, rgb, size,
    };
    use gpui_apple::metal_renderer::MetalHeadlessRenderer;

    let initialization = Instant::now();
    let clear = std::env::args().any(|arg| arg == "--clear");
    let mut renderer = MetalHeadlessRenderer::new();
    println!(
        "Renderer initialization, including shader/pipeline creation: {:.3} ms",
        initialization.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "Host build: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "optimized"
        }
    );
    for scale in [1, 2] {
        let scale_factor = scale as f32;
        let rect = |x: f32, y: f32, width: f32, height: f32| {
            Bounds::new(
                point(
                    ScaledPixels(x * scale_factor),
                    ScaledPixels(y * scale_factor),
                ),
                size(
                    ScaledPixels(width * scale_factor),
                    ScaledPixels(height * scale_factor),
                ),
            )
        };
        let frame = rect(0.0, 0.0, 1024.0, 768.0);
        let output_size = size(DevicePixels(1024 * scale), DevicePixels(768 * scale));
        println!(
            "{}x{}, {scale}x, {} {} physical px; CPU submission + GPU completion + RGBA readback (not isolated GPU time)",
            1024 * scale,
            768 * scale,
            if clear {
                "Clear radius"
            } else {
                "Gaussian sigma"
            },
            if clear { 10 * scale } else { 6 * scale }
        );
        for count in 0..=2 {
            let mut scene = Scene::default();
            for y in (0..768).step_by(32) {
                for x in (0..1024).step_by(32) {
                    scene.insert_primitive(Quad {
                        bounds: rect(x as f32, y as f32, 32.0, 32.0),
                        content_mask: ContentMask { bounds: frame },
                        background: rgb(if (x + y) % 64 == 0 {
                            0x224466
                        } else {
                            0xbb9966
                        })
                        .into(),
                        ..Default::default()
                    });
                }
            }
            for index in 0..count {
                let bounds = if index == 0 {
                    rect(128.0, 200.0, 768.0, 96.0)
                } else {
                    rect(320.0, 260.0, 512.0, 160.0)
                };
                if clear {
                    scene.insert_primitive(ClearBackdrop {
                        order: 0,
                        scale_factor,
                        bounds,
                        content_mask: ContentMask { bounds: frame },
                        corner_radius: ScaledPixels(16.0 * scale_factor),
                        blur_radius: ScaledPixels(10.0 * scale_factor),
                        appearance: WindowAppearance::Light,
                        opacity: 1.0,
                    });
                } else {
                    scene.insert_primitive(Backdrop {
                        order: 0,
                        shape: 0,
                        bounds,
                        content_mask: ContentMask { bounds: frame },
                        corner_radius: ScaledPixels(16.0 * scale_factor),
                        blur_sigma: ScaledPixels(6.0 * scale_factor),
                        tint: [0.8, 0.8, 0.8, 0.12],
                        saturation: 1.1,
                        brightness: 0.0,
                        refraction_amount: ScaledPixels(6.0 * scale_factor),
                        refraction_width: ScaledPixels(12.0 * scale_factor),
                        direction_mix: 0.0,
                        dispersion: ScaledPixels(0.5 * scale_factor),
                        highlight: 0.2,
                        edge_bleed: 0.0,
                        scale_factor,
                        opacity: 1.0,
                    });
                }
            }
            scene.finish();
            for _ in 0..3 {
                renderer
                    .render_scene_to_image(&scene, output_size)
                    .expect("Metal warmup render failed");
            }
            let mut samples = Vec::with_capacity(20);
            for _ in 0..20 {
                let start = Instant::now();
                std::hint::black_box(
                    renderer
                        .render_scene_to_image(&scene, output_size)
                        .expect("Metal benchmark render failed"),
                );
                samples.push(start.elapsed());
            }
            samples.sort_unstable();
            println!(
                "{count} effects: median {:.3} ms, p95 {:.3} ms (20 frames)",
                samples[10].as_secs_f64() * 1000.0,
                samples[18].as_secs_f64() * 1000.0
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("the backdrop benchmark requires macOS and Metal");
}
