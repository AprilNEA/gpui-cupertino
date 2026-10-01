//! Capture an inactive GPUI Clear window through WindowServer.
#![cfg(target_os = "macos")]

use anyhow::{Context as _, Result, ensure};
use gpui::{
    App, AsyncApp, Bounds, Context, DevicePixels, Image, ImageFormat, IntoElement, Pixels, Render,
    RenderImage, Window, WindowAppearance, WindowBounds, WindowOptions, div, img, point,
    prelude::*, px, rgb, size,
};
use gpui_cupertino::{
    cupertino::materials::{ClearGlassMaterial, GlassShape},
    materials::Glass,
    platform::accessibility_preferences,
};
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSApplication, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

const TITLE: &str = "Cupertino Clear capture";

struct Options {
    source: Image,
    output: PathBuf,
    scale: f32,
    bounds: Bounds<Pixels>,
    material: ClearGlassMaterial,
    appearance: WindowAppearance,
    benchmark: bool,
}

impl Options {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        ensure!(
            args.len() == 11 || (args.len() == 12 && args[11] == "--benchmark"),
            "usage: clear_live SOURCE_PNG OUTPUT_DIRECTORY SCALE X Y WIDTH HEIGHT CORNER BLUR light|dark [--benchmark]"
        );
        let values: Vec<f32> = args[3..10]
            .iter()
            .map(|value| value.parse())
            .collect::<Result<_, _>>()?;
        let [scale, x, y, width, height, corner_radius, blur_radius] = values[..] else {
            unreachable!()
        };
        ensure!(
            values.iter().all(|value| value.is_finite())
                && scale > 0.0
                && width > 0.0
                && height > 0.0,
            "invalid capture geometry"
        );
        let material =
            ClearGlassMaterial::new(GlassShape::RoundedRectangle { corner_radius }, blur_radius)?;
        let appearance = match args[10].as_str() {
            "light" => WindowAppearance::Light,
            "dark" => WindowAppearance::Dark,
            _ => anyhow::bail!("appearance must be light or dark"),
        };
        let source = Image::from_bytes(ImageFormat::Png, fs::read(&args[1])?);
        let output = PathBuf::from(&args[2]);
        fs::create_dir(&output).context("creating a new capture directory")?;
        Ok(Self {
            source,
            output,
            scale,
            bounds: Bounds::new(point(px(x), px(y)), size(px(width), px(height))),
            material,
            appearance,
            benchmark: args.len() == 12,
        })
    }
}

struct Probe {
    image: Arc<RenderImage>,
    bounds: Bounds<Pixels>,
    material: ClearGlassMaterial,
    effects: usize,
}

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .bg(rgb(0x000000))
            .children([0.0, 384.0].map(|x| {
                img(self.image.clone())
                    .absolute()
                    .left(px(x))
                    .top(px(0.0))
                    .w(px(384.0))
                    .h(px(320.0))
            }))
            .children((0..self.effects).map(|index| {
                div()
                    .absolute()
                    .left(self.bounds.origin.x + px(index as f32 * 24.0))
                    .top(self.bounds.origin.y)
                    .child(Glass::clear(
                        self.material,
                        div().w(self.bounds.size.width).h(self.bounds.size.height),
                    ))
            }))
    }
}

fn validate_native(window: &NSWindow, app: &NSApplication, scale: f32) -> Result<()> {
    println!(
        "CAPTURE_STATE window={} visible={} active={} key={} scale={} frame={:?}",
        window.windowNumber(),
        window.isVisible(),
        app.isActive(),
        window.isKeyWindow(),
        window.backingScaleFactor(),
        window.frame(),
    );
    ensure!(
        window.isVisible() && !app.isActive() && !window.isKeyWindow(),
        "capture requires a visible, inactive, non-key window"
    );
    ensure!(
        window.backingScaleFactor() == f64::from(scale),
        "the actual display scale differs from the reference"
    );
    Ok(())
}

async fn capture(options: Options, cx: &mut AsyncApp) -> Result<()> {
    let window = cx.update(|cx| -> Result<_> {
        let preferences = accessibility_preferences(cx);
        ensure!(
            !preferences.reduce_transparency && !preferences.increase_contrast,
            "capture requires Reduce Transparency and Increase Contrast to be disabled"
        );
        let image = options.source.to_image_data(cx.svg_renderer())?;
        ensure!(
            image.size(0)
                == size(
                    DevicePixels((384.0 * options.scale) as i32),
                    DevicePixels((320.0 * options.scale) as i32),
                ),
            "source dimensions differ from the reference scale"
        );
        cx.set_window_appearance(Some(options.appearance));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(100.0), px(200.0)),
                    size(px(768.0), px(320.0)),
                ))),
                titlebar: None,
                focus: false,
                is_resizable: false,
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title(TITLE);
                cx.new(|_| Probe {
                    image,
                    bounds: options.bounds,
                    material: options.material,
                    effects: 1,
                })
            },
        )
    })?;
    let mtm = MainThreadMarker::new().context("capture must run on GPUI's main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let native = app
        .windows()
        .iter()
        .find(|window| window.title().to_string() == TITLE)
        .context("the owned GPUI capture window was not found")?;
    // SAFETY: This public setter receives the owned, live NSWindow on its main thread.
    unsafe {
        let _: () = msg_send![&*native, setStyleMask: NSWindowStyleMask::Borderless];
    }
    let frame = NSRect::new(NSPoint::new(100.0, 200.0), NSSize::new(768.0, 320.0));
    // SAFETY: The owned NSWindow receives a finite frame through its public main-thread setter.
    unsafe {
        let _: () = msg_send![&*native, setFrame: frame, display: true];
    }
    cx.background_executor()
        .timer(Duration::from_millis(800))
        .await;
    app.deactivate();
    cx.background_executor().timer(Duration::from_secs(5)).await;
    validate_native(&native, &app, options.scale)?;
    window.update(cx, |_, window, cx| -> Result<()> {
        let preferences = accessibility_preferences(cx);
        ensure!(
            !preferences.reduce_transparency && !preferences.increase_contrast,
            "accessibility preferences changed during capture"
        );
        let backdrops = window.painted_clear_backdrops();
        ensure!(backdrops.len() == 1, "expected one painted Clear material");
        ensure!(
            backdrops[0].appearance == options.appearance,
            "the painted appearance differs from the reference"
        );
        println!("CLEAR_PRIMITIVE {:?}", backdrops[0]);
        let image = window.render_to_image()?;
        ensure!(
            image.dimensions()
                == (
                    (768.0 * options.scale) as u32,
                    (320.0 * options.scale) as u32
                ),
            "the GPUI framebuffer size differs from the reference"
        );
        image.save(options.output.join("layer.png"))?;
        Ok(())
    })??;
    for repeat in 0..2 {
        validate_native(&native, &app, options.scale)?;
        let status = Command::new("/usr/sbin/screencapture")
            .args(["-x", "-o", &format!("-l{}", native.windowNumber())])
            .arg(options.output.join(format!("capture{repeat}.png")))
            .status()
            .context("capturing the owned GPUI window")?;
        ensure!(
            status.success(),
            "WindowServer capture failed; verify the graphical session and screen recording permission"
        );
        validate_native(&native, &app, options.scale)?;
        cx.background_executor()
            .timer(Duration::from_millis(300))
            .await;
    }
    if options.benchmark {
        for effects in 0..=2 {
            window.update(cx, |probe, _, cx| {
                probe.effects = effects;
                cx.notify();
            })?;
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            validate_native(&native, &app, options.scale)?;
            window.update(cx, |_, window, _| -> Result<()> {
                ensure!(
                    window.painted_clear_backdrops().len() == effects,
                    "benchmark scene has not been painted"
                );
                for frame in 0..23 {
                    let start = Instant::now();
                    // Each readback acquires a drawable; release the autoreleased drawable before the next frame.
                    drop(objc2::rc::autoreleasepool(|_| window.render_to_image())?);
                    let milliseconds = start.elapsed().as_secs_f64() * 1000.0;
                    if frame >= 3 {
                        println!(
                            "LIVE_BENCH effects={effects} frame={} ms={milliseconds:.6}",
                            frame - 3
                        );
                    }
                }
                Ok(())
            })??;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let options = Options::parse()?;
    gpui_platform::application().run(move |cx: &mut App| {
        cx.spawn(async move |cx| {
            // AppKit termination does not return to main, so report errors before quitting.
            if let Err(error) = capture(options, cx).await {
                eprintln!("Error: {error:#}");
                std::process::exit(1);
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    anyhow::bail!("the application event loop ended before capture completion")
}
