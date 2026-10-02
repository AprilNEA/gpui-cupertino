//! Capture AppKit glass beside a deterministic Metal readback on the same background.
//! Run on macOS 26+: `cargo run -p gpui-cupertino --example native_compare -- work/comparison`.
//! Isolate one case with `OUT probe --background step:v --shape roundrect --appearance light --style regular`.
//! Optional `--size WIDTHxHEIGHT` centers the shape; circles require equal dimensions.
//! Optional `--offset DX,DY` shifts only the glass by integer logical pixels within the panel.
//! Optional `--capture-count N` captures 2 through 32 frames with a 0.3-second delay after each capture completes.
//! `step:v[:phase]` / `step:h[:phase]` shift the edge right/down in half-logical-pixel increments.
//! Step boundaries must map exactly to device pixels at the current display scale.
//! `checker:N:phase:RRGGBB:RRGGBB` uses explicit colors; `checker:N[:phase]` keeps appearance palettes.
//! Probes default to inactive. `--window-state active` checks application and window focus at each recorded capture checkpoint.

#[cfg(target_os = "macos")]
#[path = "native_compare/background.rs"]
mod background;

#[cfg(target_os = "macos")]
#[path = "native_compare/probe.rs"]
mod probe;

#[cfg(target_os = "macos")]
#[path = "native_compare/lifecycle.rs"]
mod lifecycle;

#[cfg(target_os = "macos")]
mod comparison {
    use super::probe::WindowState;
    use anyhow::{Context, Result, ensure};
    use gpui::{Backdrop, Bounds, ContentMask, Quad, ScaledPixels, Scene, point, rgb, size};
    use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, rc::Retained};
    use objc2_app_kit::{
        NSAppearance, NSApplication, NSGlassEffectView, NSGlassEffectViewStyle, NSImage,
        NSImageScaling, NSImageView, NSView, NSWindow, NSWorkspace,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    use std::path::Path;

    pub(super) const WIDTH: f32 = 384.0;
    pub(super) const HEIGHT: f32 = 320.0;
    pub(super) const SHAPES: [[f32; 5]; 3] = [
        [24.0, 24.0, 336.0, 72.0, 20.0],
        [24.0, 136.0, 200.0, 64.0, 32.0],
        [256.0, 120.0, 96.0, 96.0, 48.0],
    ];

    pub(super) fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    fn bounds(x: f32, y: f32, width: f32, height: f32, scale: f32) -> Bounds<ScaledPixels> {
        Bounds::new(
            point(ScaledPixels(x * scale), ScaledPixels(y * scale)),
            size(ScaledPixels(width * scale), ScaledPixels(height * scale)),
        )
    }

    pub(super) fn scene(scale: f32, dark: bool, phase: u32, glass: bool, clear: bool) -> Scene {
        let mut scene = Scene::default();
        let frame = bounds(0.0, 0.0, WIDTH, HEIGHT, scale);
        // Integer-cell backgrounds provide identical input to both renderers.
        for row in 0..=HEIGHT as u32 / 32 {
            for col in 0..=WIDTH as u32 / 32 {
                let odd = (col + row) % 2 == 0;
                let x = (col * 32) as f32 - phase as f32;
                let y = (row * 32) as f32 - phase as f32;
                let color = match (dark, odd) {
                    (false, true) => 0x76bced,
                    (false, false) => 0xf1a684,
                    (true, true) => 0x162e46,
                    (true, false) => 0x74423a,
                };
                scene.insert_primitive(Quad {
                    bounds: bounds(x, y, 32.0, 32.0, scale),
                    content_mask: ContentMask { bounds: frame },
                    background: rgb(color).into(),
                    ..Default::default()
                });
            }
        }
        if glass {
            for (index, [x, y, w, h, radius]) in SHAPES.into_iter().enumerate() {
                let bounds = bounds(x, y, w, h, scale);
                scene.insert_primitive(Backdrop {
                    order: 0,
                    shape: index as u32,
                    bounds,
                    content_mask: ContentMask { bounds: frame },
                    corner_radius: ScaledPixels(radius * scale),
                    blur_sigma: ScaledPixels(8.0 * scale),
                    tint: if dark {
                        [0.012, 0.018, 0.035, if clear { 0.12 } else { 0.55 }]
                    } else {
                        [1.0, 1.0, 1.0, if clear { 0.12 } else { 0.5 }]
                    },
                    saturation: 1.25,
                    brightness: 0.0,
                    refraction_amount: ScaledPixels(7.0 * scale),
                    refraction_width: ScaledPixels(18.0 * scale),
                    direction_mix: 0.25,
                    dispersion: ScaledPixels(0.8 * scale),
                    highlight: 0.35,
                    edge_bleed: 0.0,
                    scale_factor: scale,
                    opacity: 1.0,
                });
            }
        }
        scene.finish();
        scene
    }

    pub(super) fn image_view(
        path: &Path,
        x: f64,
        mtm: MainThreadMarker,
    ) -> Result<Retained<NSImageView>> {
        let image = NSImage::initWithContentsOfFile(
            NSImage::alloc(),
            &NSString::from_str(path.to_str().context("image path must be UTF-8")?),
        )
        .context("loading reference PNG")?;
        image.setSize(NSSize::new(WIDTH.into(), HEIGHT.into()));
        let view = NSImageView::initWithFrame(
            NSImageView::alloc(mtm),
            rect(x, 0.0, WIDTH.into(), HEIGHT.into()),
        );
        view.setImage(Some(&image));
        view.setImageScaling(NSImageScaling::ScaleNone);
        Ok(view)
    }

    pub(super) fn appearance(app: &NSApplication, dark: bool) -> Result<()> {
        let appearance = NSAppearance::appearanceNamed(&NSString::from_str(if dark {
            "NSAppearanceNameDarkAqua"
        } else {
            "NSAppearanceNameAqua"
        }))
        .context("loading AppKit appearance")?;
        app.setAppearance(Some(&appearance));
        Ok(())
    }

    pub(super) fn add_glass(
        native: &NSImageView,
        [x, y, w, h, radius]: [f32; 5],
        clear: bool,
        mtm: MainThreadMarker,
    ) {
        let glass = NSGlassEffectView::initWithFrame(
            NSGlassEffectView::alloc(mtm),
            rect(x.into(), (HEIGHT - y - h).into(), w.into(), h.into()),
        );
        glass.setCornerRadius(radius.into());
        glass.setStyle(if clear {
            NSGlassEffectViewStyle::Clear
        } else {
            NSGlassEffectViewStyle::Regular
        });
        glass.setContentView(Some(&NSView::initWithFrame(
            NSView::alloc(mtm),
            rect(0.0, 0.0, w.into(), h.into()),
        )));
        native.addSubview(&glass);
    }

    impl WindowState {
        pub(super) fn validate_capture_state(
            self,
            label: &str,
            app: &NSApplication,
            window: &NSWindow,
            expected_scale: f64,
        ) -> Result<()> {
            let visible = window.isVisible();
            let active = app.isActive();
            let key = window.isKeyWindow();
            let scale = window.backingScaleFactor();
            let workspace = NSWorkspace::sharedWorkspace();
            let reduce_transparency = workspace.accessibilityDisplayShouldReduceTransparency();
            let increase_contrast = workspace.accessibilityDisplayShouldIncreaseContrast();
            println!(
                "{label}: window={}, visible={visible}, active={active}, key={key}, scale={scale}, reduce_transparency={reduce_transparency}, increase_contrast={increase_contrast}",
                window.windowNumber()
            );
            ensure!(
                visible && self.matches(active, key),
                "{label}: native comparison requires a visible window in {self:?} state"
            );
            ensure!(
                scale == expected_scale,
                "{label}: display scale changed from {expected_scale} to {scale}"
            );
            ensure!(
                !reduce_transparency && !increase_contrast,
                "{label}: native comparison requires Reduce Transparency and Increase Contrast to be disabled"
            );
            if self.is_active() {
                let frame = window.frame();
                let screen = window
                    .screen()
                    .context("active probe must have an attached screen")?;
                println!(
                    "ACTIVE_CONTEXT {{\"frame\":[{},{},{},{}],\"headroom\":{},\"potential_headroom\":{},\"reference_headroom\":{}}}",
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                    screen.maximumExtendedDynamicRangeColorComponentValue(),
                    screen.maximumPotentialExtendedDynamicRangeColorComponentValue(),
                    screen.maximumReferenceExtendedDynamicRangeColorComponentValue(),
                );
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    lifecycle::run()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("native comparison requires macOS 26 or later");
}
