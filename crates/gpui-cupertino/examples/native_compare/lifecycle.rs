//! Advance captures between turns of the real AppKit application event loop.

use super::{comparison::*, probe::Probe};
use anyhow::{Context, Result, bail, ensure};
use gpui::{DevicePixels, PlatformHeadlessRenderer, size};
use gpui_apple::metal_renderer::MetalHeadlessRenderer;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectNSDelayedPerforming, NSObjectProtocol, NSString,
};
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, process::Command, time::Instant};

enum Case {
    Probe(Probe),
    Demo { dark: bool, clear: bool, phase: u32 },
}

#[derive(Clone, Copy)]
enum Stage {
    Prepare,
    Deactivate,
    AwaitInactive { deadline: Instant },
    AfterCapture { repeat: u32 },
}

struct Session {
    directory: PathBuf,
    cases: VecDeque<Case>,
    window: Retained<NSWindow>,
    renderer: MetalHeadlessRenderer,
    scale: f64,
    name: String,
    stage: Stage,
    capture_count: u32,
    prepared_at: Instant,
}

impl Session {
    fn new(directory: PathBuf, probe: Option<Probe>, mtm: MainThreadMarker) -> Self {
        let capture_count = probe.as_ref().map_or(2, |probe| probe.capture_count);
        // Calibration inputs must not include the window decoration's alpha mask.
        let style = if probe.is_some() {
            NSWindowStyleMask::Borderless
        } else {
            NSWindowStyleMask::Titled
        };
        let cases = if let Some(probe) = probe {
            VecDeque::from([Case::Probe(probe)])
        } else {
            [false, true]
                .into_iter()
                .flat_map(|dark| {
                    [false, true].into_iter().flat_map(move |clear| {
                        [0, 7]
                            .into_iter()
                            .map(move |phase| Case::Demo { dark, clear, phase })
                    })
                })
                .collect()
        };
        // SAFETY: main-thread initialization with a valid content rectangle and buffered backing.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(100.0, 200.0, (WIDTH * 2.0).into(), HEIGHT.into()),
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: Rust retains the window; AppKit must not release it again when closed.
        unsafe { window.setReleasedWhenClosed(false) };
        let scale = window.backingScaleFactor();
        Self {
            directory,
            cases,
            window,
            renderer: MetalHeadlessRenderer::new(),
            scale,
            name: String::new(),
            stage: Stage::Prepare,
            capture_count,
            prepared_at: Instant::now(),
        }
    }

    fn prepare(&mut self, case: Case, app: &NSApplication, mtm: MainThreadMarker) -> Result<()> {
        self.prepared_at = Instant::now();
        let scale = self.scale as f32;
        let (dark, clear) = match &case {
            Case::Probe(probe) => (probe.dark, probe.clear),
            Case::Demo { dark, clear, .. } => (*dark, *clear),
        };
        appearance(app, dark)?;
        self.name = match &case {
            Case::Probe(probe) => probe.name(),
            Case::Demo { phase, .. } => format!(
                "{}-{}-phase{phase}",
                if dark { "dark" } else { "light" },
                if clear { "clear" } else { "regular" }
            ),
        };
        let background = self.directory.join(format!("{}-background.png", self.name));
        let (reference, shapes, title) = match &case {
            Case::Probe(probe) => {
                probe.save_background(&mut self.renderer, scale, &background)?;
                (
                    background.clone(),
                    vec![probe.geometry()],
                    "Native AppKit probe (left) | Same input, no glass (right)",
                )
            }
            Case::Demo { phase, .. } => {
                let output_size = size(
                    DevicePixels((WIDTH * scale) as i32),
                    DevicePixels((HEIGHT * scale) as i32),
                );
                let reference = self.directory.join(format!("{}-metal.png", self.name));
                self.renderer
                    .render_scene_to_image(&scene(scale, dark, *phase, false, clear), output_size)?
                    .save(&background)?;
                self.renderer
                    .render_scene_to_image(&scene(scale, dark, *phase, true, clear), output_size)?
                    .save(&reference)?;
                (
                    reference,
                    SHAPES.to_vec(),
                    "Native AppKit (left) | Cupertino demo parameters (right)",
                )
            }
        };
        self.window.setTitle(&NSString::from_str(title));
        let content = NSView::initWithFrame(
            NSView::alloc(mtm),
            rect(0.0, 0.0, (WIDTH * 2.0).into(), HEIGHT.into()),
        );
        let native = image_view(&background, 0.0, mtm)?;
        content.addSubview(&native);
        let reference_view = image_view(&reference, WIDTH.into(), mtm)?;
        content.addSubview(&reference_view);
        for shape in shapes {
            add_glass(&native, shape, clear, mtm);
        }
        self.window.setContentView(Some(&content));
        if let Case::Probe(probe) = case {
            probe.save_metadata(
                &self.directory,
                self.scale,
                self.window.frame().size.height - f64::from(HEIGHT),
            )?;
        }
        Ok(())
    }

    fn capture_path(&self, repeat: u32) -> PathBuf {
        self.directory
            .join(format!("{}-capture{repeat}.png", self.name))
    }

    fn record_timing(&self, stage: &str, repeat: Option<u32>) {
        let elapsed = self.prepared_at.elapsed().as_secs_f64() * 1000.0;
        let repeat = repeat.map_or_else(String::new, |repeat| format!(",\"repeat\":{repeat}"));
        println!("TIMING {{\"stage\":\"{stage}\",\"elapsed_since_prepare_ms\":{elapsed}{repeat}}}");
    }

    fn capture(&mut self, app: &NSApplication, repeat: u32) -> Result<Option<f64>> {
        let path = self.capture_path(repeat);
        validate_capture_state(
            &format!("{} before", path.display()),
            app,
            &self.window,
            self.scale,
        )?;
        self.record_timing("before_capture", Some(repeat));
        let status = Command::new("/usr/sbin/screencapture")
            .args(["-x", "-o", &format!("-l{}", self.window.windowNumber())])
            .arg(&path)
            .status()
            .context("capturing the owned reference window")?;
        ensure!(
            status.success(),
            "capture of owned window failed; verify the graphical session and screen recording permission"
        );
        self.record_timing("after_capture", Some(repeat));
        self.stage = Stage::AfterCapture { repeat };
        Ok(Some(0.3))
    }

    fn advance(&mut self, app: &NSApplication, mtm: MainThreadMarker) -> Result<Option<f64>> {
        match self.stage {
            Stage::Prepare => {
                let Some(case) = self.cases.pop_front() else {
                    self.window.orderOut(None);
                    return Ok(None);
                };
                self.prepare(case, app, mtm)?;
                self.stage = Stage::Deactivate;
                Ok(Some(0.8))
            }
            Stage::Deactivate => {
                app.deactivate();
                self.record_timing("deactivate", None);
                self.stage = Stage::AwaitInactive {
                    deadline: Instant::now() + std::time::Duration::from_secs(10),
                };
                Ok(Some(5.0))
            }
            Stage::AwaitInactive { deadline } => {
                if !app.isActive() && !self.window.isKeyWindow() {
                    return self.capture(app, 0);
                }
                ensure!(
                    Instant::now() < deadline,
                    "native comparison did not reach inactive/non-key state within ten seconds: running={}, active={}, key={}",
                    app.isRunning(),
                    app.isActive(),
                    self.window.isKeyWindow()
                );
                Ok(Some(0.1))
            }
            Stage::AfterCapture { repeat } => {
                validate_capture_state(
                    &format!("{} after", self.capture_path(repeat).display()),
                    app,
                    &self.window,
                    self.scale,
                )?;
                if repeat + 1 < self.capture_count {
                    self.capture(app, repeat + 1)
                } else {
                    self.stage = Stage::Prepare;
                    Ok(Some(0.0))
                }
            }
        }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; ivars are confined to the main thread.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<Session>]
    struct CaptureDelegate;

    unsafe impl NSObjectProtocol for CaptureDelegate {}
    unsafe impl NSApplicationDelegate for CaptureDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            self.ivars().borrow().window.orderFront(None);
            self.schedule(0.0);
        }
    }
    impl CaptureDelegate {
        #[unsafe(method(advanceCapture:))]
        fn advance_capture(&self, _sender: Option<&AnyObject>) {
            let app = NSApplication::sharedApplication(self.mtm());
            let next = self.ivars().borrow_mut().advance(&app, self.mtm());
            match next {
                Ok(Some(delay)) => self.schedule(delay),
                Ok(None) => app.terminate(None),
                Err(error) => {
                    eprintln!("Error: {error:#}");
                    std::process::exit(1);
                }
            }
        }
    }
);

impl CaptureDelegate {
    fn new(session: Session, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RefCell::new(session));
        // SAFETY: the superclass is NSObject and all Rust ivars are initialized.
        unsafe { msg_send![super(this), init] }
    }

    fn schedule(&self, delay: f64) {
        // SAFETY: advanceCapture: accepts one nullable object and runs on the main-thread receiver.
        unsafe { self.performSelector_withObject_afterDelay(sel!(advanceCapture:), None, delay) };
    }
}

pub(super) fn run() -> Result<()> {
    ensure!(
        objc2::available!(macos = 26.0),
        "native glass comparison requires macOS 26 or later"
    );
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(args.next().context("supply an output directory")?);
    let probe = match args.next().as_deref() {
        None => None,
        Some("probe") => Some(Probe::parse(args)?),
        Some(mode) => bail!(
            "unknown mode {mode}; use OUT or OUT probe --background ... --shape ... --appearance ... --style ..."
        ),
    };
    std::fs::create_dir_all(&directory)?;
    let mtm = MainThreadMarker::new().context("reference app must run on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    ensure!(
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular),
        "activating reference app policy"
    );
    let delegate = CaptureDelegate::new(Session::new(directory, probe, mtm), mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
    bail!("native comparison event loop ended before capture completion")
}
