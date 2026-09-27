use anyhow::{Result, anyhow};
use gpui::{Backdrop, DevicePixels, Size};
use metal::{MTLPixelFormat, MTLSize, MTLTextureType, MTLTextureUsage};
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
                include_str!("backdrop.metal"),
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
        let snapshot = texture(device, width, height, None, target.pixel_format());
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

        // Undecimated scales avoid phase shifts as content scrolls. The kernel
        // variance is 2*sum(weight[i]*i*i), and stride doubles at each scale.
        const KERNEL_VARIANCE: f64 = 0.995912;
        let desired = (1.0 + 3.0 * f64::from(backdrop.blur_sigma.0).powi(2) / KERNEL_VARIANCE)
            .log2()
            .mul_add(0.5, 0.0)
            .ceil() as u64;
        let available = (width.max(height) as f32).log2().floor() as u64;
        let levels = desired.min(available) + 1;
        // Sigma is uniform per effect, so only the final adjacent scales are
        // needed. Two full-resolution slices retain those scales by parity.
        // ponytail: full-resolution filtering costs bandwidth; optimize only while preserving phase invariance.
        let pyramid = texture(
            device,
            width,
            height,
            Some(levels.min(2)),
            MTLPixelFormat::RGBA16Float,
        );
        let mut previous = layer(&pyramid, 0);
        self.dispatch(command, &self.copy, &snapshot, &previous, 1);
        if levels > 1 {
            let horizontal = texture(device, width, height, None, MTLPixelFormat::RGBA16Float);
            for level in 1..levels {
                let stride = 1 << (level - 1);
                self.dispatch(command, &self.horizontal, &previous, &horizontal, stride);
                let next = layer(&pyramid, level % 2);
                self.dispatch(command, &self.vertical, &horizontal, &next, stride);
                previous = next;
            }
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
            viewport: [
                width as f32,
                height as f32,
                (levels - 1) as f32,
                backdrop.scale_factor,
            ],
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
        encoder.set_fragment_texture(1, Some(&pyramid));
        encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, 6);
        encoder.end_encoding();
    }

    fn dispatch(
        &self,
        command: &metal::CommandBufferRef,
        pipeline: &metal::ComputePipelineStateRef,
        input: &metal::TextureRef,
        output: &metal::TextureRef,
        stride: u32,
    ) {
        let encoder = command.new_compute_command_encoder();
        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_texture(0, Some(input));
        encoder.set_texture(1, Some(output));
        encoder.set_bytes(
            0,
            mem::size_of::<u32>() as u64,
            &stride as *const _ as *const c_void,
        );
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
    array_layers: Option<u64>,
    format: MTLPixelFormat,
) -> metal::Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_texture_type(if array_layers.is_some() {
        MTLTextureType::D2Array
    } else {
        MTLTextureType::D2
    });
    descriptor.set_width(width);
    descriptor.set_height(height);
    descriptor.set_pixel_format(format);
    descriptor.set_array_length(array_layers.unwrap_or(1));
    descriptor.set_storage_mode(metal::MTLStorageMode::Private);
    descriptor.set_usage(
        MTLTextureUsage::ShaderRead
            | MTLTextureUsage::ShaderWrite
            | MTLTextureUsage::PixelFormatView,
    );
    device.new_texture(&descriptor)
}

fn layer(texture: &metal::TextureRef, index: u64) -> metal::Texture {
    texture.new_texture_view_from_slice(
        texture.pixel_format(),
        MTLTextureType::D2,
        metal::NSRange {
            location: 0,
            length: 1,
        },
        metal::NSRange {
            location: index,
            length: 1,
        },
    )
}
