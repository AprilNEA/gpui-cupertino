//! Read the existing GPUI window's attached layer without changing its color space.

use anyhow::{Context, Result, ensure};
use gpui::App;
use objc2::{Encoding, MainThreadMarker, RefEncode, msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::NSApplication;
use objc2_foundation::NSString;
use std::{io::Write, time::Duration};

#[repr(C)]
struct CGColorSpace {
    _opaque: [u8; 0],
}

// SAFETY: CoreGraphics declares CGColorSpaceRef as a pointer to opaque struct CGColorSpace.
// The diagnostic borrows this pointer and only accesses it through CoreGraphics.
unsafe impl RefEncode for CGColorSpace {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGColorSpace", &[]));
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    // CFStringRef is toll-free bridged to NSString; Copy transfers one retained reference.
    fn CGColorSpaceCopyName(space: *const CGColorSpace) -> *mut NSString;
}

pub(super) fn inspect_color_space_and_quit(cx: &App) {
    cx.spawn(async |cx| {
        cx.background_executor().timer(Duration::from_secs(1)).await;
        cx.update(|cx| {
            let result = inspect();
            cx.quit();
            result.expect("attached GPUI Metal layer inspection failed");
        });
    })
    .detach();
}

fn inspect() -> Result<()> {
    let mtm = MainThreadMarker::new().context("inspection requires GPUI's main thread")?;
    let windows = NSApplication::sharedApplication(mtm).windows();
    let window = windows
        .iter()
        .find(|window| window.title().to_string() == "Cupertino Glass")
        .context("the owned glass example window was not found")?;
    ensure!(
        window.isVisible(),
        "the glass example window is not visible"
    );
    let content = window
        .contentView()
        .context("the GPUI window has no content view")?;
    // GPUI installs its renderer-owning GPUIView directly inside AppKit's content view.
    let view = content
        .subviews()
        .iter()
        .find(|view| view.class().name() == c"GPUIView")
        .context("the owned content view has no direct GPUIView child")?;
    // SAFETY: NSView.layer is a public nullable object getter, called on the main thread.
    let layer: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, layer] };
    let layer = layer.context("the GPUIView has no attached layer")?;
    let is_metal = std::iter::successors(Some(layer.class()), |class| class.superclass())
        .any(|class| class.name() == c"CAMetalLayer");
    ensure!(
        is_metal,
        "attached layer is {}, not CAMetalLayer",
        layer.class().name().to_string_lossy()
    );
    // SAFETY: the class check above establishes CAMetalLayer's public CGColorSpaceRef getter.
    let color_space: *const CGColorSpace = unsafe { msg_send![&*layer, colorspace] };
    let name = if color_space.is_null() {
        "nil".to_owned()
    } else {
        // SAFETY: CAMetalLayer retains the non-null color space for this synchronous call.
        let name = unsafe { CGColorSpaceCopyName(color_space) };
        // SAFETY: CopyName returns either null or a +1 CFString, toll-free bridged to NSString.
        let name = unsafe { Retained::from_raw(name) };
        name.map_or_else(|| "unnamed".to_owned(), |name| name.to_string())
    };
    println!(
        "GPUI_ATTACHED_LAYER window={} visible={} view_class={} class={} colorspace_is_nil={} colorspace_name={name} colorspace_pointer={color_space:p}",
        window.windowNumber(),
        window.isVisible(),
        view.class().name().to_string_lossy(),
        layer.class().name().to_string_lossy(),
        color_space.is_null()
    );
    std::io::stdout()
        .flush()
        .context("flushing layer inspection")
}
