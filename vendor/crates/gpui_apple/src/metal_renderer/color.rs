// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    data::{CFData, CFDataRef},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::CFString,
};
use metal::{MTLPixelFormat, MTLResourceOptions, MTLSize, MTLStorageMode, MTLTextureUsage};
use objc::{msg_send, runtime::Object, sel, sel_impl};
use std::{mem, ptr};

#[cfg(any(test, feature = "test-support"))]
pub(super) mod tests;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCopyICCData(space: CFTypeRef) -> CFDataRef;
    fn CGColorSpaceGetNumberOfComponents(space: CFTypeRef) -> usize;
}

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    fn ColorSyncProfileCreate(data: CFDataRef, error: *mut CFTypeRef) -> CFTypeRef;
    fn ColorSyncTransformCreate(sequence: CFArrayRef, options: CFDictionaryRef) -> CFTypeRef;
    fn ColorSyncTransformCopyProperty(
        transform: CFTypeRef,
        key: CFTypeRef,
        options: CFDictionaryRef,
    ) -> CFTypeRef;
    static kColorSyncProfile: CFTypeRef;
    static kColorSyncTransformTag: CFTypeRef;
    static kColorSyncTransformDeviceToPCS: CFTypeRef;
    static kColorSyncTransformPCSToDevice: CFTypeRef;
    static kColorSyncRenderingIntent: CFTypeRef;
    static kColorSyncRenderingIntentRelative: CFTypeRef;
    static kColorSyncBlackPointCompensation: CFTypeRef;
    static kColorSyncConvertQuality: CFTypeRef;
    static kColorSyncBestQuality: CFTypeRef;
    static kColorSyncTransformParametricConversionData: CFTypeRef;
}

#[derive(Clone, PartialEq)]
pub(super) struct ColorSpaces {
    source: CFType,
    display: CFType,
}

impl ColorSpaces {
    /// Read the attached layer and window display spaces on the window's thread.
    ///
    /// # Safety
    /// `window` must refer to the live NSWindow that owns `layer`.
    pub(super) unsafe fn for_window(
        window: *mut Object,
        layer: &metal::MetalLayerRef,
    ) -> Result<Option<Self>> {
        // SAFETY: CAMetalLayer's public colorspace property returns a borrowed CGColorSpaceRef.
        let source: CFTypeRef = unsafe { msg_send![layer, colorspace] };
        if source.is_null() {
            // A nil layer colorspace disables color matching; its values are already display RGB.
            return Ok(None);
        }
        // SAFETY: The caller supplies a live NSWindow on its owning thread.
        let screen: *mut Object = unsafe { msg_send![window, screen] };
        ensure!(
            !screen.is_null(),
            "Clear requires a window attached to a display"
        );
        // SAFETY: NSScreen's public colorSpace property returns NSColorSpace or nil.
        let color: *mut Object = unsafe { msg_send![screen, colorSpace] };
        ensure!(!color.is_null(), "the display has no color space");
        // SAFETY: NSColorSpace's public CGColorSpace property returns a borrowed CGColorSpaceRef.
        let display: CFTypeRef = unsafe { msg_send![color, CGColorSpace] };
        ensure!(!display.is_null(), "the display has no CGColorSpace");
        // SAFETY: The non-null layer property remains valid while the owning layer is borrowed.
        let source = unsafe { CFType::wrap_under_get_rule(source) };
        // SAFETY: The non-null display property remains valid while the screen is alive.
        let display = unsafe { CFType::wrap_under_get_rule(display) };
        Ok((source != display).then_some(Self { source, display }))
    }
}

#[derive(Clone, Copy)]
pub(super) enum Destination {
    Display,
    Layer,
}

/// Execute the public ColorSync parametric conversion on the renderer's GPU.
pub(super) struct ColorContext {
    pub(super) spaces: ColorSpaces,
    forward: Transform,
    reverse: Transform,
    to_display: metal::ComputePipelineState,
    to_layer: metal::ComputePipelineState,
}

impl ColorContext {
    pub(super) fn new(device: &metal::DeviceRef, spaces: &ColorSpaces) -> Result<Self> {
        let options = metal::CompileOptions::new();
        options.set_fast_math_enabled(false);
        let library = device
            .new_library_with_source(include_str!("color.metal"), &options)
            .map_err(|error| anyhow!("compiling color conversion: {error}"))?;
        let pipeline = |name| -> Result<_> {
            let function = library
                .get_function(name, None)
                .map_err(|error| anyhow!("color function {name}: {error}"))?;
            device
                .new_compute_pipeline_state_with_function(&function)
                .map_err(|error| anyhow!("color pipeline {name}: {error}"))
        };
        Ok(Self {
            spaces: spaces.clone(),
            forward: Transform::new(device, &spaces.source, &spaces.display)?,
            reverse: Transform::new(device, &spaces.display, &spaces.source)?,
            to_display: pipeline("color_to_display")?,
            to_layer: pipeline("color_to_layer")?,
        })
    }

    pub(super) fn convert(
        &self,
        device: &metal::DeviceRef,
        command: &metal::CommandBufferRef,
        input: &metal::TextureRef,
        destination: Destination,
    ) -> metal::Texture {
        let (transform, pipeline) = match destination {
            Destination::Display => (&self.forward, &self.to_display),
            Destination::Layer => (&self.reverse, &self.to_layer),
        };
        let output = texture(device, input, MTLPixelFormat::BGRA8Unorm);
        let encoder = command.new_compute_command_encoder();
        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_texture(0, Some(input));
        encoder.set_texture(1, Some(&output));
        encoder.set_buffer(0, Some(&transform.stages), 0);
        encoder.set_bytes(
            1,
            mem::size_of::<u32>() as u64,
            (&transform.count as *const u32).cast(),
        );
        let width = pipeline.thread_execution_width();
        encoder.dispatch_threads(
            MTLSize {
                width: output.width(),
                height: output.height(),
                depth: 1,
            },
            MTLSize {
                width,
                height: pipeline.max_total_threads_per_threadgroup() / width,
                depth: 1,
            },
        );
        encoder.end_encoding();
        output
    }
}

#[repr(C)]
struct Stage {
    values: [[f32; 4]; 3],
    operation: u32,
    channel: u32,
    padding: [u32; 2],
}

struct Transform {
    stages: metal::Buffer,
    count: u32,
    #[cfg(any(test, feature = "test-support"))]
    reference: CFType,
}

impl Transform {
    fn new(device: &metal::DeviceRef, source: &CFType, destination: &CFType) -> Result<Self> {
        // SAFETY: These public ColorSync constants have process lifetime.
        let tags = unsafe {
            [
                kColorSyncTransformDeviceToPCS,
                kColorSyncTransformPCSToDevice,
            ]
        };
        let sequence = [source, destination]
            .into_iter()
            .zip(tags)
            .map(|(space, tag)| -> Result<_> {
                let profile = profile(space)?;
                // SAFETY: All constants are public process-lifetime CFStrings; profile is retained by the dictionary.
                Ok(unsafe {
                    CFDictionary::from_CFType_pairs(&[
                        (CFType::wrap_under_get_rule(kColorSyncProfile), profile),
                        (
                            CFType::wrap_under_get_rule(kColorSyncTransformTag),
                            CFType::wrap_under_get_rule(tag),
                        ),
                        (
                            CFType::wrap_under_get_rule(kColorSyncRenderingIntent),
                            CFType::wrap_under_get_rule(kColorSyncRenderingIntentRelative),
                        ),
                        (
                            CFType::wrap_under_get_rule(kColorSyncBlackPointCompensation),
                            CFBoolean::false_value().as_CFType(),
                        ),
                    ])
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let sequence = CFArray::from_CFTypes(&sequence);
        // SAFETY: These public ColorSync constants have process lifetime.
        let options = unsafe {
            CFDictionary::from_CFType_pairs(&[(
                CFType::wrap_under_get_rule(kColorSyncConvertQuality),
                CFType::wrap_under_get_rule(kColorSyncBestQuality),
            )])
        };
        // SAFETY: Both containers retain the live profiles and public transform options.
        let transform = unsafe {
            ColorSyncTransformCreate(
                sequence.as_concrete_TypeRef(),
                options.as_concrete_TypeRef(),
            )
        };
        ensure!(
            !transform.is_null(),
            "cannot create the display color transform"
        );
        // SAFETY: The public Create function returned a non-null owned Core Foundation object.
        let transform = unsafe { CFType::wrap_under_create_rule(transform) };
        // SAFETY: The live transform accepts this public property key; no additional options are requested.
        let components = unsafe {
            ColorSyncTransformCopyProperty(
                transform.as_CFTypeRef(),
                kColorSyncTransformParametricConversionData,
                ptr::null(),
            )
        };
        ensure!(
            !components.is_null(),
            "Clear requires parametric RGB color profiles"
        );
        // SAFETY: The public Copy function returned a non-null owned Core Foundation object.
        let components = unsafe { CFType::wrap_under_create_rule(components) };
        let stages = array(&components)?
            .iter()
            .map(Stage::parse)
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            !stages.is_empty(),
            "the display color transform has no components"
        );
        let count = stages
            .len()
            .try_into()
            .context("too many color transform components")?;
        Ok(Self {
            stages: device.new_buffer_with_data(
                stages.as_ptr().cast(),
                mem::size_of_val(stages.as_slice()) as u64,
                MTLResourceOptions::StorageModeShared,
            ),
            count,
            #[cfg(any(test, feature = "test-support"))]
            reference: transform,
        })
    }
}

impl Stage {
    fn parse(value: &CFType) -> Result<Self> {
        let component = value
            .downcast::<CFDictionary>()
            .context("invalid ColorSync component")?;
        let mut stage = Self {
            values: [[0.0; 4]; 3],
            operation: 0,
            channel: 0,
            padding: [0; 2],
        };
        if let Some(matrix) = field(&component, "com.apple.cmm.Matrix") {
            let rows = array(&matrix)?;
            ensure!(rows.len() == 3, "ColorSync matrix must have three rows");
            for (row, target) in rows.iter().zip(&mut stage.values) {
                let values = array(row)?;
                ensure!(
                    values.len() == 4,
                    "ColorSync matrix row must have four values"
                );
                for (value, output) in values.iter().zip(target) {
                    *output = number(value)?;
                }
            }
            return Ok(stage);
        }
        for kind in 0..5 {
            if let Some(curve) = field(&component, &format!("com.apple.cmm.ParamCurve{kind}")) {
                let channel = field(&component, "com.apple.cmm.ChannelID")
                    .and_then(|value| value.downcast::<CFNumber>())
                    .and_then(|number| number.to_i64())
                    .context("ColorSync curve has no channel")?;
                ensure!(
                    (0..3).contains(&channel),
                    "Clear requires RGB color transforms"
                );
                let values = array(&curve)?;
                ensure!(
                    values.len() == 7,
                    "ColorSync curve must have seven parameters"
                );
                for (value, output) in values.iter().zip(stage.values.iter_mut().flatten()) {
                    *output = number(value)?;
                }
                stage.operation = kind + 1;
                stage.channel = channel as u32;
                return Ok(stage);
            }
        }
        bail!("unsupported public ColorSync parametric component")
    }
}

fn array(value: &CFType) -> Result<Vec<CFType>> {
    let values = value
        .downcast::<CFArray>()
        .context("expected a ColorSync parameter array")?;
    Ok(values
        .iter()
        .map(|value| {
            // SAFETY: Public ColorSync component arrays contain retained Core Foundation objects.
            unsafe { CFType::wrap_under_get_rule(*value) }
        })
        .collect())
}

fn field(dictionary: &CFDictionary, name: &str) -> Option<CFType> {
    dictionary
        .find(CFString::new(name).as_CFTypeRef())
        .map(|value| {
            // SAFETY: Public ColorSync component dictionaries contain retained Core Foundation objects.
            unsafe { CFType::wrap_under_get_rule(*value) }
        })
}

fn number(value: &CFType) -> Result<f32> {
    value
        .downcast::<CFNumber>()
        .and_then(|number| number.to_f32())
        .context("expected a ColorSync Float32 parameter")
}

fn profile(space: &CFType) -> Result<CFType> {
    // SAFETY: ColorSpaces retains live CGColorSpace objects from the owning layer and display.
    ensure!(
        unsafe { CGColorSpaceGetNumberOfComponents(space.as_CFTypeRef()) } == 3,
        "Clear requires RGB layer and display color spaces"
    );
    // SAFETY: The live CGColorSpace accepts the public ICC export call.
    let data = unsafe { CGColorSpaceCopyICCData(space.as_CFTypeRef()) };
    ensure!(!data.is_null(), "cannot export the display ICC profile");
    // SAFETY: The public Copy function returned non-null owned CFData.
    let data = unsafe { CFData::wrap_under_create_rule(data) };
    // SAFETY: The retained ICC data is live; a null error pointer is permitted by this public API.
    let profile = unsafe { ColorSyncProfileCreate(data.as_concrete_TypeRef(), ptr::null_mut()) };
    ensure!(
        !profile.is_null(),
        "cannot create the display ColorSync profile"
    );
    // SAFETY: The public Create function returned a non-null owned ColorSyncProfile.
    Ok(unsafe { CFType::wrap_under_create_rule(profile) })
}

pub(super) fn texture(
    device: &metal::DeviceRef,
    source: &metal::TextureRef,
    format: MTLPixelFormat,
) -> metal::Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_width(source.width());
    descriptor.set_height(source.height());
    descriptor.set_pixel_format(format);
    descriptor.set_storage_mode(MTLStorageMode::Private);
    descriptor.set_usage(
        MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite | MTLTextureUsage::RenderTarget,
    );
    device.new_texture(&descriptor)
}
