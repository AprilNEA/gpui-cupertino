// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

use super::*;
use metal::{MTLCommandBufferStatus, MTLOrigin, MTLRegion};
use std::ffi::c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateWithName(name: CFTypeRef) -> CFTypeRef;
    static kCGColorSpaceSRGB: CFTypeRef;
    static kCGColorSpaceDisplayP3: CFTypeRef;
    static kCGColorSpaceAdobeRGB1998: CFTypeRef;
}

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    fn ColorSyncTransformConvert(
        transform: CFTypeRef,
        width: usize,
        height: usize,
        destination: *mut c_void,
        destination_depth: i32,
        destination_layout: u32,
        destination_stride: usize,
        source: *const c_void,
        source_depth: i32,
        source_layout: u32,
        source_stride: usize,
        options: CFDictionaryRef,
    ) -> bool;
}

fn half(value: f32) -> f32 {
    let step = 2.0_f32.powi(
        (value.abs().log2().floor() as i32)
            .saturating_sub(10)
            .max(-24),
    );
    (value / step).round_ties_even() * step
}

pub(crate) fn check_color_conversion() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let device = metal::Device::system_default().context("Metal device required")?;
        let queue = device.new_command_queue();
        let size = MTLSize {
            width: 256,
            height: 256,
            depth: 1,
        };
        let origin = MTLOrigin { x: 0, y: 0, z: 0 };
        let region = MTLRegion { origin, size };
        let pixels: Vec<u8> = (0..65536_u32)
            .flat_map(|index| {
                let red = (index >> 8) as u8;
                let green = index as u8;
                let blue = red.wrapping_mul(37).wrapping_add(green.wrapping_mul(13));
                [blue, green, red, 255]
            })
            .collect();
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_width(size.width);
        descriptor.set_height(size.height);
        descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        descriptor.set_storage_mode(MTLStorageMode::Shared);
        descriptor.set_usage(MTLTextureUsage::ShaderRead);
        let source = device.new_texture(&descriptor);
        source.replace_region(region, 0, pixels.as_ptr().cast(), size.width * 4);
        // SAFETY: These public named color-space constants have process lifetime.
        let pairs = unsafe {
            [
                (kCGColorSpaceSRGB, kCGColorSpaceDisplayP3),
                (kCGColorSpaceAdobeRGB1998, kCGColorSpaceDisplayP3),
            ]
        };
        for (source_name, display_name) in pairs {
            let named = |name| -> Result<CFType> {
                // SAFETY: The pair contains public, process-lifetime CGColorSpace name constants.
                let space = unsafe { CGColorSpaceCreateWithName(name) };
                ensure!(
                    !space.is_null(),
                    "cannot create the standard test color space"
                );
                // SAFETY: The public Create function returned a non-null owned CGColorSpace.
                Ok(unsafe { CFType::wrap_under_create_rule(space) })
            };
            let context = ColorContext::new(
                &device,
                &ColorSpaces {
                    source: named(source_name)?,
                    display: named(display_name)?,
                },
            )?;
            for destination in [Destination::Display, Destination::Layer] {
                let (reference, half_boundary) = match destination {
                    Destination::Display => (&context.forward.reference, true),
                    Destination::Layer => (&context.reverse.reference, false),
                };
                let input: Vec<f32> = pixels
                    .chunks_exact(4)
                    .flat_map(|pixel| {
                        [pixel[2], pixel[1], pixel[0]].map(|channel| {
                            let value = f32::from(channel) / 255.0;
                            if half_boundary { half(value) } else { value }
                        })
                    })
                    .collect();
                let mut expected = vec![0.0_f32; input.len()];
                let stride = mem::size_of_val(input.as_slice());
                // SAFETY: The live transform receives complete packed RGB Float32 buffers with matching strides; depth 7 and layout 0 are public constants.
                let converted = unsafe {
                    ColorSyncTransformConvert(
                        reference.as_CFTypeRef(),
                        input.len() / 3,
                        1,
                        expected.as_mut_ptr().cast(),
                        7,
                        0,
                        stride,
                        input.as_ptr().cast(),
                        7,
                        0,
                        stride,
                        ptr::null(),
                    )
                };
                ensure!(converted, "public ColorSync reference conversion failed");
                let command = queue.new_command_buffer();
                let output = context.convert(&device, command, &source, destination);
                let readback = device.new_texture(&descriptor);
                let blit = command.new_blit_command_encoder();
                blit.copy_from_texture(&output, 0, 0, origin, size, &readback, 0, 0, origin);
                blit.end_encoding();
                command.commit();
                command.wait_until_completed();
                assert_eq!(command.status(), MTLCommandBufferStatus::Completed);
                let mut actual = vec![0_u8; pixels.len()];
                readback.get_bytes(actual.as_mut_ptr().cast(), size.width * 4, region, 0);
                let mut changed_channels = 0;
                for (index, (pixel, reference)) in actual
                    .chunks_exact(4)
                    .zip(expected.chunks_exact(3))
                    .enumerate()
                {
                    for channel in 0..3 {
                        let value = if half_boundary {
                            half(reference[channel])
                        } else {
                            reference[channel]
                        };
                        let code = (value.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8;
                        changed_channels += usize::from(pixel[2 - channel] != code);
                        assert!(
                            pixel[2 - channel].abs_diff(code) <= 1,
                            "pixel {index}, channel {channel}, half={half_boundary}: GPU {} vs ColorSync {code}",
                            pixel[2 - channel]
                        );
                    }
                }
                // Allow rare power-evaluation ties, but reject a missing half-precision boundary.
                assert!(
                    changed_channels <= expected.len() / 1000,
                    "{changed_channels} channels differ from ColorSync"
                );
            }
        }
        Ok(())
    })
}
