use gpui::{
    Backdrop, Bounds, ContentMask, DevicePixels, PlatformHeadlessRenderer, Quad, ScaledPixels,
    Scene, point, rgb, size,
};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;

pub(crate) fn rect([x, y, width, height]: [f32; 4]) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(width), ScaledPixels(height)),
    )
}

pub(crate) fn quad(scene: &mut Scene, bounds: Bounds<ScaledPixels>, color: u32) {
    scene.insert_primitive(Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: rgb(color).into(),
        ..Default::default()
    });
}

pub(crate) fn material(bounds: Bounds<ScaledPixels>, scale: u32) -> Backdrop {
    Backdrop {
        order: 0,
        shape: 0,
        bounds,
        content_mask: ContentMask { bounds },
        corner_radius: ScaledPixels(0.0),
        blur_sigma: ScaledPixels(0.0),
        tint: [0.0; 4],
        saturation: 1.0,
        brightness: 0.0,
        refraction_amount: ScaledPixels(0.0),
        refraction_width: ScaledPixels(scale as f32),
        direction_mix: 0.0,
        dispersion: ScaledPixels(0.0),
        highlight: 0.0,
        edge_bleed: 0.0,
        scale_factor: scale as f32,
        opacity: 1.0,
    }
}

pub(crate) fn render(
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
        .expect("numeric reference checks require a working Metal device")
        .pixels()
        .map(|pixel| pixel.0)
        .collect()
}

pub(crate) fn linear(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn encoded(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let value = if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (value * 255.0).round() as u8
}
