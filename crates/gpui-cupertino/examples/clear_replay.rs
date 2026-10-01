//! Render an encoded BGRA control panel through the production Clear scene primitive.
#![cfg(target_os = "macos")]

use anyhow::{Context as _, Result, ensure};
use gpui::{
    AtlasKey, Bounds, ClearBackdrop, ContentMask, DevicePixels, ImageId, PlatformHeadlessRenderer,
    PolychromeSprite, RenderImageParams, ScaledPixels, Scene, WindowAppearance, point, size,
};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use std::{borrow::Cow, fs, io::Write as _};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    ensure!(
        args.len() == 13,
        "usage: clear_replay SOURCE_BGRA OUTPUT_RGBA WIDTH HEIGHT SCALE X Y WIDTH HEIGHT CORNER BLUR light|dark"
    );
    let width: i32 = args[3].parse()?;
    let height: i32 = args[4].parse()?;
    ensure!(
        width > 0 && height > 0 && width <= 8192 && height <= 16384,
        "invalid panel size"
    );
    let values: Vec<f32> = args[5..12]
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    let [scale, x, y, w, h, corner, blur] = values[..] else {
        unreachable!()
    };
    ensure!(
        values.iter().all(|v| v.is_finite())
            && scale > 0.0
            && w > 0.0
            && h > 0.0
            && corner >= 0.0
            && blur >= 0.0,
        "invalid glass geometry"
    );
    let appearance = match args[12].as_str() {
        "light" => WindowAppearance::Light,
        "dark" => WindowAppearance::Dark,
        _ => anyhow::bail!("appearance must be light or dark"),
    };
    let source = fs::read(&args[1]).context("reading BGRA control panel")?;
    ensure!(
        source.len() == width as usize * height as usize * 4,
        "control panel byte count differs"
    );
    let mut renderer = MetalHeadlessRenderer::new();
    let tile = renderer
        .sprite_atlas()
        .get_or_insert_with(
            AtlasKey::Image(RenderImageParams {
                image_id: ImageId(0),
                frame_index: 0,
            }),
            &mut || {
                Ok(Some((
                    size(DevicePixels(width), DevicePixels(height)),
                    Cow::Borrowed(&source),
                )))
            },
        )?
        .context("uploading control panel")?;
    let rect = |x, y, w, h| {
        Bounds::new(
            point(ScaledPixels(x), ScaledPixels(y)),
            size(ScaledPixels(w), ScaledPixels(h)),
        )
    };
    let clip = ContentMask {
        bounds: rect(0.0, 0.0, (width * 2) as f32, height as f32),
    };
    let mut scene = Scene::default();
    for x in [0, width] {
        scene.insert_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            bounds: rect(x as f32, 0.0, width as f32, height as f32),
            content_mask: clip,
            corner_radii: Default::default(),
            tile,
            grayscale: false.into(),
            opacity: 1.0,
        });
    }
    scene.insert_primitive(ClearBackdrop {
        order: 0,
        scale_factor: scale,
        bounds: rect(x * scale, y * scale, w * scale, h * scale),
        content_mask: clip,
        corner_radius: ScaledPixels(corner * scale),
        blur_radius: ScaledPixels(blur * scale),
        appearance,
        opacity: 1.0,
    });
    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, size(DevicePixels(width * 2), DevicePixels(height)))?;
    fs::File::create_new(&args[2])?.write_all(image.as_raw())?;
    Ok(())
}
