// Modified by AprilNEA for GPUI Alloy. Patch records: https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md

use anyhow::{Result, anyhow};
use gpui::{Backdrop, DevicePixels, Size};
use metal::{MTLPixelFormat, MTLResourceOptions, MTLSize, MTLTextureType, MTLTextureUsage};
use std::{ffi::c_void, mem};

/// Owns pipelines, while each ordered backdrop owns a fresh snapshot.
// ponytail: full-window textures per effect; crop and share only after profiling read dependencies.
pub(super) struct BackdropRenderer {
    copy: metal::ComputePipelineState,
    horizontal: metal::ComputePipelineState,
    vertical: metal::ComputePipelineState,
    composite: metal::RenderPipelineState,
}

#[repr(C)]
struct Uniforms {
    bounds: [f32; 4],
    clip: [f32; 4],
    tint: [f32; 4],
    shape: [f32; 4],
    optics: [f32; 4],
    color: [f32; 4],
    viewport: [f32; 4],
}

impl BackdropRenderer {
    pub(super) fn new(device: &metal::DeviceRef) -> Result<Self> {
        let library = device
            .new_library_with_source(
                concat!(
                    include_str!("../continuous_corners.metal"),
                    "\n",
                    include_str!("backdrop.metal")
                ),
                &metal::CompileOptions::new(),
            )
            .map_err(|error| anyhow!("compiling backdrop shaders: {error}"))?;
        let compute = |name| {
            let function = library
                .get_function(name, None)
                .map_err(|error| anyhow!("{error}"))?;
            device
                .new_compute_pipeline_state_with_function(&function)
                .map_err(|error| anyhow!("{name}: {error}"))
        };
        let descriptor = metal::RenderPipelineDescriptor::new();
        descriptor.set_label("Cupertino backdrop");
        let vertex = library
            .get_function("backdrop_vertex", None)
            .map_err(|error| anyhow!("{error}"))?;
        let fragment = library
            .get_function("backdrop_fragment", None)
            .map_err(|error| anyhow!("{error}"))?;
        descriptor.set_vertex_function(Some(&vertex));
        descriptor.set_fragment_function(Some(&fragment));
        let attachment = descriptor
            .color_attachments()
            .object_at(0)
            .expect("pipeline has color attachment 0");
        attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        // The fragment blends against its immutable snapshot, exactly once.
        attachment.set_blending_enabled(false);
        Ok(Self {
            copy: compute("backdrop_copy")?,
            horizontal: compute("backdrop_horizontal")?,
            vertical: compute("backdrop_vertical")?,
            composite: device
                .new_render_pipeline_state(&descriptor)
                .map_err(|error| anyhow!("backdrop pipeline: {error}"))?,
        })
    }

    pub(super) fn draw(
        &self,
        device: &metal::DeviceRef,
        command: &metal::CommandBufferRef,
        target: &metal::TextureRef,
        size: Size<DevicePixels>,
        backdrop: &Backdrop,
    ) {
        let (width, height) = (target.width(), target.height());
        let snapshot = texture(device, width, height, target.pixel_format());
        let blit = command.new_blit_command_encoder();
        blit.copy_from_texture(
            target,
            0,
            0,
            metal::MTLOrigin { x: 0, y: 0, z: 0 },
            MTLSize {
                width,
                height,
                depth: 1,
            },
            &snapshot,
            0,
            0,
            metal::MTLOrigin { x: 0, y: 0, z: 0 },
        );
        blit.end_encoding();

        let filtered = texture(device, width, height, MTLPixelFormat::RGBA16Float);
        self.dispatch(command, &self.copy, &snapshot, &filtered, None);
        if backdrop.blur_sigma.0 > 0.0 {
            let horizontal = texture(device, width, height, MTLPixelFormat::RGBA16Float);
            let horizontal_kernel = gaussian_kernel(device, backdrop.blur_sigma.0, width);
            let vertical_kernel = gaussian_kernel(device, backdrop.blur_sigma.0, height);
            self.dispatch(
                command,
                &self.horizontal,
                &filtered,
                &horizontal,
                Some(&horizontal_kernel),
            );
            self.dispatch(
                command,
                &self.vertical,
                &horizontal,
                &filtered,
                Some(&vertical_kernel),
            );
        }
        let b = backdrop.bounds;
        let c = backdrop.content_mask.bounds;
        let uniforms = Uniforms {
            bounds: [b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0],
            clip: [c.origin.x.0, c.origin.y.0, c.size.width.0, c.size.height.0],
            tint: backdrop.tint,
            shape: [
                backdrop.corner_radius.0,
                backdrop.shape as f32,
                backdrop.blur_sigma.0,
                backdrop.opacity,
            ],
            optics: [
                backdrop.refraction_amount.0,
                backdrop.refraction_width.0,
                backdrop.direction_mix,
                backdrop.dispersion.0,
            ],
            color: [
                backdrop.saturation,
                backdrop.brightness,
                backdrop.highlight,
                backdrop.edge_bleed,
            ],
            viewport: [width as f32, height as f32, 0.0, backdrop.scale_factor],
        };
        let encoder = super::new_command_encoder_for_texture(command, target, size, None);
        encoder.set_render_pipeline_state(&self.composite);
        encoder.set_vertex_bytes(
            0,
            mem::size_of::<Uniforms>() as u64,
            &uniforms as *const _ as *const c_void,
        );
        encoder.set_fragment_bytes(
            0,
            mem::size_of::<Uniforms>() as u64,
            &uniforms as *const _ as *const c_void,
        );
        encoder.set_fragment_texture(0, Some(&snapshot));
        encoder.set_fragment_texture(1, Some(&filtered));
        encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, 6);
        encoder.end_encoding();
    }

    fn dispatch(
        &self,
        command: &metal::CommandBufferRef,
        pipeline: &metal::ComputePipelineStateRef,
        input: &metal::TextureRef,
        output: &metal::TextureRef,
        kernel: Option<&metal::BufferRef>,
    ) {
        let encoder = command.new_compute_command_encoder();
        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_texture(0, Some(input));
        encoder.set_texture(1, Some(output));
        let tap_count = kernel.map_or(0, |buffer| {
            buffer.length() / mem::size_of::<[f32; 2]>() as u64
        }) as u32;
        encoder.set_bytes(
            0,
            mem::size_of::<u32>() as u64,
            &tap_count as *const _ as *const c_void,
        );
        encoder.set_buffer(1, kernel, 0);
        encoder.dispatch_thread_groups(
            MTLSize {
                width: output.width().div_ceil(8),
                height: output.height().div_ceil(8),
                depth: 1,
            },
            MTLSize {
                width: 8,
                height: 8,
                depth: 1,
            },
        );
        encoder.end_encoding();
    }
}

fn texture(
    device: &metal::DeviceRef,
    width: u64,
    height: u64,
    format: MTLPixelFormat,
) -> metal::Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_texture_type(MTLTextureType::D2);
    descriptor.set_width(width);
    descriptor.set_height(height);
    descriptor.set_pixel_format(format);
    descriptor.set_storage_mode(metal::MTLStorageMode::Private);
    descriptor.set_usage(
        MTLTextureUsage::ShaderRead
            | MTLTextureUsage::ShaderWrite
            | MTLTextureUsage::PixelFormatView,
    );
    device.new_texture(&descriptor)
}

fn gaussian_kernel(device: &metal::DeviceRef, sigma: f32, extent: u64) -> metal::Buffer {
    let sigma = f64::from(sigma);
    let radius = (4.0 * sigma).ceil().min((extent - 1) as f64) as usize;
    let inverse_width = 1.0 / (sigma * std::f64::consts::SQRT_2);
    let mut weights: Vec<f64> = (0..=radius)
        .map(|index| {
            let offset = index as f64;
            0.5 * (libm::erf((offset + 0.5) * inverse_width)
                - libm::erf((offset - 0.5) * inverse_width))
        })
        .collect();
    if radius == extent as usize - 1 {
        // Beyond the axis extent every tap clamps to the same edge pixel.
        // Fold that entire Gaussian tail into the last tap instead of dropping it.
        weights[radius] +=
            libm::erfc((radius as f64 + 0.5) * inverse_width) * if radius == 0 { 1.0 } else { 0.5 };
    }
    let total = weights[0] + 2.0 * weights[1..].iter().sum::<f64>();
    let mut taps = vec![[0.0, (weights[0] / total) as f32]];
    for index in (1..=radius).step_by(2) {
        let near = weights[index];
        let far = weights.get(index + 1).copied().unwrap_or(0.0);
        let weight = near + far;
        if weight > 0.0 {
            taps.push([
                (index as f64 + far / weight) as f32,
                (weight / total) as f32,
            ]);
        }
    }
    // Linear sampling combines each adjacent pair exactly. A Metal buffer also
    // handles large kernels beyond set_bytes' 4 KiB limit without unbounded allocation.
    device.new_buffer_with_data(
        taps.as_ptr().cast::<c_void>(),
        mem::size_of_val(taps.as_slice()) as u64,
        MTLResourceOptions::StorageModeShared,
    )
}
