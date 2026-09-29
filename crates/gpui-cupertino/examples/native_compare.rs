//! Capture inactive AppKit glass beside a deterministic Metal readback on the same background.
//! Run on macOS 26+: `cargo run -p gpui-cupertino --example native_compare -- work/comparison`.
//! Isolate one case with `OUT probe --background step:v --shape roundrect --appearance light --style regular`.
//! Optional `--size WIDTHxHEIGHT` centers the shape; circles require equal dimensions.
//! `step:v[:phase]` / `step:h[:phase]` shift the edge right/down in half-logical-pixel increments.
//! Step boundaries must map exactly to device pixels at the current display scale.

#[cfg(target_os = "macos")]
#[path = "native_compare/probe.rs"]
mod probe;

#[cfg(target_os = "macos")]
mod comparison {
    use anyhow::{Context, Result, bail, ensure};
    use gpui::{
        Backdrop, Bounds, ContentMask, DevicePixels, PlatformHeadlessRenderer, Quad, ScaledPixels,
        Scene, point, rgb, size,
    };
    use gpui_apple::metal_renderer::MetalHeadlessRenderer;
    use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, rc::Retained};
    use objc2_app_kit::{
        NSAppearance, NSApplication, NSApplicationActivationPolicy, NSBackingStoreType,
        NSEventMask, NSGlassEffectView, NSGlassEffectViewStyle, NSImage, NSImageScaling,
        NSImageView, NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
    };
    use objc2_foundation::{NSDate, NSPoint, NSRect, NSSize, NSString};
    use std::{
        path::Path,
        process::Command,
        time::{Duration, Instant},
    };

    pub(super) const WIDTH: f32 = 384.0;
    pub(super) const HEIGHT: f32 = 320.0;
    const SHAPES: [[f32; 5]; 3] = [
        [24.0, 24.0, 336.0, 72.0, 20.0],
        [24.0, 136.0, 200.0, 64.0, 32.0],
        [256.0, 120.0, 96.0, 96.0, 48.0],
    ];

    fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    fn bounds(x: f32, y: f32, width: f32, height: f32, scale: f32) -> Bounds<ScaledPixels> {
        Bounds::new(
            point(ScaledPixels(x * scale), ScaledPixels(y * scale)),
            size(ScaledPixels(width * scale), ScaledPixels(height * scale)),
        )
    }

    fn scene(scale: f32, dark: bool, phase: u32, glass: bool, clear: bool) -> Scene {
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

    fn image_view(path: &Path, x: f64, mtm: MainThreadMarker) -> Result<Retained<NSImageView>> {
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

    fn settle(app: &NSApplication, duration: Duration) {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            let deadline = NSDate::dateWithTimeIntervalSinceNow(0.01);
            if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&deadline),
                &NSString::from_str("kCFRunLoopDefaultMode"),
                true,
            ) {
                app.sendEvent(&event);
            }
            app.updateWindows();
        }
    }

    fn appearance(app: &NSApplication, dark: bool) -> Result<()> {
        let appearance = NSAppearance::appearanceNamed(&NSString::from_str(if dark {
            "NSAppearanceNameDarkAqua"
        } else {
            "NSAppearanceNameAqua"
        }))
        .context("loading AppKit appearance")?;
        app.setAppearance(Some(&appearance));
        Ok(())
    }

    fn add_glass(
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

    fn capture(app: &NSApplication, window: &NSWindow, prefix: &Path, scale: f64) -> Result<()> {
        settle(app, Duration::from_millis(800));
        // Launch activation can arrive during the first event-loop drain.
        app.deactivate();
        settle(app, Duration::from_millis(800));
        for repeat in 0..2 {
            let path = prefix.with_file_name(format!(
                "{}-capture{repeat}.png",
                prefix
                    .file_name()
                    .context("capture prefix needs a name")?
                    .to_str()
                    .context("capture name must be UTF-8")?
            ));
            validate_capture_state(&format!("{} before", path.display()), app, window, scale)?;
            let status = Command::new("/usr/sbin/screencapture")
                .args(["-x", "-o", &format!("-l{}", window.windowNumber())])
                .arg(&path)
                .status()
                .context("capturing the owned reference window")?;
            ensure!(
                status.success(),
                "capture of owned window failed; verify the graphical session and screen recording permission"
            );
            settle(app, Duration::from_millis(300));
            validate_capture_state(&format!("{} after", path.display()), app, window, scale)?;
        }
        Ok(())
    }

    fn validate_capture_state(
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
            visible && !active && !key,
            "{label}: native comparison requires a visible, inactive, non-key window"
        );
        ensure!(
            scale == expected_scale,
            "{label}: display scale changed from {expected_scale} to {scale}"
        );
        ensure!(
            !reduce_transparency && !increase_contrast,
            "{label}: native comparison requires Reduce Transparency and Increase Contrast to be disabled"
        );
        Ok(())
    }

    pub fn run() -> Result<()> {
        ensure!(
            objc2::available!(macos = 26.0),
            "native glass comparison requires macOS 26 or later"
        );
        let mut args = std::env::args().skip(1);
        let directory = args.next().context("supply an output directory")?;
        let probe = match args.next().as_deref() {
            None => None,
            Some("probe") => Some(super::probe::Probe::parse(args)?),
            Some(mode) => bail!(
                "unknown mode {mode}; use OUT or OUT probe --background ... --shape ... --appearance ... --style ..."
            ),
        };
        let directory = Path::new(&directory);
        std::fs::create_dir_all(directory)?;
        let mtm = MainThreadMarker::new().context("reference app must run on the main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        ensure!(
            app.setActivationPolicy(NSApplicationActivationPolicy::Regular),
            "activating reference app policy"
        );
        app.finishLaunching();
        // SAFETY: initialized on the main thread with a valid content rectangle and buffered backing.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(100.0, 200.0, (WIDTH * 2.0).into(), HEIGHT.into()),
                NSWindowStyleMask::Titled,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: Rust retains the window; AppKit must not release it again when closed.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(
            "Native AppKit (left) | Cupertino demo parameters (right)",
        ));
        window.orderFront(None);
        app.deactivate();
        let scale = window.backingScaleFactor();
        let render_scale = scale as f32;
        let mut renderer = MetalHeadlessRenderer::new();
        let output_size = size(
            DevicePixels((WIDTH * render_scale) as i32),
            DevicePixels((HEIGHT * render_scale) as i32),
        );
        if let Some(probe) = probe {
            appearance(&app, probe.dark)?;
            window.setTitle(&NSString::from_str(
                "Native AppKit probe (left) | Same input, no glass (right)",
            ));
            let name = probe.name();
            let background = directory.join(format!("{name}-background.png"));
            probe.save_background(&mut renderer, render_scale, &background)?;
            let content = NSView::initWithFrame(
                NSView::alloc(mtm),
                rect(0.0, 0.0, (WIDTH * 2.0).into(), HEIGHT.into()),
            );
            let native = image_view(&background, 0.0, mtm)?;
            content.addSubview(&native);
            let control = image_view(&background, WIDTH.into(), mtm)?;
            content.addSubview(&control);
            add_glass(&native, probe.geometry(), probe.clear, mtm);
            window.setContentView(Some(&content));
            probe.save_metadata(
                directory,
                scale,
                window.frame().size.height - f64::from(HEIGHT),
            )?;
            capture(&app, &window, &directory.join(name), scale)?;
            window.orderOut(None);
            return Ok(());
        }
        for dark in [false, true] {
            appearance(&app, dark)?;
            for clear in [false, true] {
                for phase in [0, 7] {
                    let name = format!(
                        "{}-{}-phase{phase}",
                        if dark { "dark" } else { "light" },
                        if clear { "clear" } else { "regular" }
                    );
                    let background = directory.join(format!("{name}-background.png"));
                    let independent = directory.join(format!("{name}-metal.png"));
                    renderer
                        .render_scene_to_image(
                            &scene(render_scale, dark, phase, false, clear),
                            output_size,
                        )?
                        .save(&background)?;
                    renderer
                        .render_scene_to_image(
                            &scene(render_scale, dark, phase, true, clear),
                            output_size,
                        )?
                        .save(&independent)?;
                    let content = NSView::initWithFrame(
                        NSView::alloc(mtm),
                        rect(0.0, 0.0, (WIDTH * 2.0).into(), HEIGHT.into()),
                    );
                    let native = image_view(&background, 0.0, mtm)?;
                    content.addSubview(&native);
                    let metal = image_view(&independent, WIDTH.into(), mtm)?;
                    content.addSubview(&metal);
                    for shape in SHAPES {
                        add_glass(&native, shape, clear, mtm);
                    }
                    window.setContentView(Some(&content));
                    capture(&app, &window, &directory.join(name), scale)?;
                }
            }
        }
        window.orderOut(None);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    comparison::run()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("native comparison requires macOS 26 or later");
}
