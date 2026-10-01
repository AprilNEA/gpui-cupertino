use anyhow::{Result, anyhow, ensure};
use gpui::{ClearBackdrop, WindowAppearance};
use metal::{MTLLanguageVersion, MTLPixelFormat, MTLSize, MTLTextureUsage};
use objc::{msg_send, sel, sel_impl};
use std::mem;

pub(super) struct ClearRenderer {
    capture: metal::RenderPipelineState,
    base: metal::RenderPipelineState,
    store: metal::RenderPipelineState,
    composite: metal::RenderPipelineState,
    unquantized: metal::RenderPipelineState,
    restore: metal::RenderPipelineState,
    copy: metal::ComputePipelineState,
    downsample: metal::ComputePipelineState,
}

#[repr(C)]
struct Uniforms {
    shape: [f32; 4],
    displacement: [f32; 4],
    face: [u16; 12],
    blur_radius: f32,
    opacity: f32,
    clip: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 4],
    sdf: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
struct CopyUniforms {
    base: [i16; 2],
    padding: [u16; 2],
    clamp: [i16; 4],
    base_size: [u16; 2],
    destination_size: [u16; 2],
    destination_level: u16,
    no_base_mip: u8,
    tail: [u8; 5],
}

#[repr(C)]
struct DownsampleUniforms {
    source_level: u16,
    destination_level: u16,
    width: u16,
    height: u16,
    step: [f32; 2],
}

struct Composite<'a> {
    target: &'a metal::TextureRef,
    source: &'a metal::TextureRef,
    background: Option<&'a metal::TextureRef>,
    backdrop: &'a ClearBackdrop,
    layout: &'a Layout,
}

struct Layout {
    origin: [f32; 2],
    size: [f32; 2],
    aligned_origin: [f32; 2],
    capture: [u64; 2],
    blur: [u64; 2],
    levels: u64,
    radius: f32,
    omitted_base: bool,
}

impl Layout {
    fn new(backdrop: &ClearBackdrop, height: u64) -> Result<Self> {
        let b = backdrop.bounds;
        let origin = [
            (b.origin.x.0 / 2.0).floor(),
            ((height as f32 - b.bottom().0) / 2.0).floor(),
        ];
        let end = [
            (b.right().0 / 2.0).ceil(),
            ((height as f32 - b.origin.y.0) / 2.0).ceil(),
        ];
        let size = [end[0] - origin[0], end[1] - origin[1]];
        let radius = backdrop.blur_radius.0 / 2.0;
        ensure!(
            origin.into_iter().chain(end).all(f32::is_finite)
                && size.into_iter().all(|v| v > 0.0 && v <= 16384.0)
                && radius.is_finite()
                && radius >= 0.0
                && backdrop.scale_factor.is_finite()
                && backdrop.scale_factor > 0.0,
            "invalid Clear geometry or device scale"
        );
        let rho = 1.6_f32 * radius;
        let requested = if rho == 0.0 {
            1
        } else {
            (rho.log2().ceil().max(0.0) as u32 + 1).max(2)
        };
        let count = (size[0].max(size[1]).log2().floor() as u32 + 1).min(requested);
        let omitted_base = rho >= 2.0 && count > 1;
        let factor = if omitted_base { 2.0 } else { 1.0 };
        let alignment = (1_u32 << count.min(7)) as f32;
        let aligned_origin = origin.map(|v| ((v - 2.8 * radius) / alignment).floor() * alignment);
        let aligned_end = end.map(|v| ((v + 2.8 * radius) / alignment).ceil() * alignment);
        let capture = size.map(|v| (v / 64.0).ceil() * 64.0);
        let blur = [0, 1]
            .map(|axis| ((aligned_end[axis] - aligned_origin[axis]) / factor / 64.0).ceil() * 64.0);
        ensure!(
            capture
                .into_iter()
                .chain(blur)
                .all(|v| v > 0.0 && v <= 16384.0),
            "Clear texture dimensions exceed the Apple GPU limit"
        );
        for axis in 0..2 {
            let base = aligned_origin[axis] - origin[axis];
            ensure!(
                base >= f32::from(i16::MIN) + 4.0
                    && base + blur[axis] * factor + 4.0 <= f32::from(i16::MAX),
                "Clear filter coordinates exceed signed 16-bit range"
            );
        }
        Ok(Self {
            origin,
            size,
            aligned_origin,
            capture: capture.map(|v| v as u64),
            blur: blur.map(|v| v as u64),
            levels: u64::from(count - u32::from(omitted_base)),
            radius: rho / factor,
            omitted_base,
        })
    }
}

impl ClearRenderer {
    pub(super) fn new(device: &metal::DeviceRef) -> Result<Self> {
        let options = metal::CompileOptions::new();
        options.set_language_version(MTLLanguageVersion::V3_1);
        options.set_fast_math_enabled(true);
        let library = device
            .new_library_with_source(
                concat!(
                    include_str!("../continuous_corners.metal"),
                    "\n",
                    include_str!("clear_blur.metal"),
                    "\n",
                    include_str!("clear.metal")
                ),
                &options,
            )
            .map_err(|e| anyhow!("compiling Clear shaders: {e}"))?;
        let function = |name| {
            library
                .get_function(name, None)
                .map_err(|e| anyhow!("Clear function {name}: {e}"))
        };
        let render = |vertex, fragment, blend| -> Result<_> {
            let descriptor = metal::RenderPipelineDescriptor::new();
            descriptor.set_label(fragment);
            let vertex_function = function(vertex)?;
            let fragment_function = function(fragment)?;
            descriptor.set_vertex_function(Some(&vertex_function));
            descriptor.set_fragment_function(Some(&fragment_function));
            let attachment = descriptor
                .color_attachments()
                .object_at(0)
                .expect("color attachment 0");
            attachment.set_pixel_format(if fragment == "clear_unquantized" {
                MTLPixelFormat::RGBA16Float
            } else {
                MTLPixelFormat::BGRA8Unorm
            });
            attachment.set_blending_enabled(blend);
            attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::One);
            attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::One);
            attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            attachment
                .set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            if vertex == "clear_vertex" {
                let layout = metal::VertexDescriptor::new();
                for (index, offset, format) in [
                    (0, 0, metal::MTLVertexFormat::Float4),
                    (1, 16, metal::MTLVertexFormat::Float2),
                    (2, 24, metal::MTLVertexFormat::Float2),
                ] {
                    let attribute = layout
                        .attributes()
                        .object_at(index)
                        .expect("vertex attribute");
                    attribute.set_format(format);
                    attribute.set_offset(offset);
                    attribute.set_buffer_index(1);
                }
                layout
                    .layouts()
                    .object_at(1)
                    .expect("vertex layout")
                    .set_stride(mem::size_of::<Vertex>() as u64);
                descriptor.set_vertex_descriptor(Some(&layout));
            }
            device
                .new_render_pipeline_state(&descriptor)
                .map_err(|e| anyhow!("Clear pipeline {fragment}: {e}"))
        };
        let compute = |name| {
            let function = function(name)?;
            device
                .new_compute_pipeline_state_with_function(&function)
                .map_err(|e| anyhow!("Clear pipeline {name}: {e}"))
        };
        Ok(Self {
            capture: render("fullscreen", "clear_capture", false)?,
            base: render("fullscreen", "clear_copy_base", false)?,
            store: render("fullscreen", "store_blur_half", false)?,
            composite: render("clear_vertex", "clear_fragment", true)?,
            unquantized: render("clear_vertex", "clear_unquantized", false)?,
            restore: render("clear_vertex", "clear_restore", true)?,
            copy: compute("copy_base_compute")?,
            downsample: compute("downsample_compute")?,
        })
    }

    pub(super) fn draw(
        &self,
        device: &metal::DeviceRef,
        command: &metal::CommandBufferRef,
        background: &metal::TextureRef,
        target: &metal::TextureRef,
        backdrop: &ClearBackdrop,
    ) -> Result<()> {
        let layout = Layout::new(backdrop, target.height())?;
        // Each effect captures the preceding framebuffer before the effect changes the target.
        let captured = texture(device, layout.capture, 1, MTLPixelFormat::BGRA8Unorm);
        let blurred = texture(
            device,
            layout.blur,
            layout.levels,
            MTLPixelFormat::BGRA8Unorm,
        );
        let staging = texture(
            device,
            layout.blur,
            layout.levels,
            MTLPixelFormat::RGBA16Float,
        );
        let encoder = render(command, &captured, 0, metal::MTLLoadAction::Clear);
        encoder.set_render_pipeline_state(&self.capture);
        encoder.set_scissor_rect(metal::MTLScissorRect {
            x: 0,
            y: 0,
            width: (layout.size[0] as u64 + 1).min(layout.capture[0]),
            height: (layout.size[1] as u64 + 1).min(layout.capture[1]),
        });
        let capture = [
            layout.origin[0] * 2.0,
            layout.origin[1] * 2.0,
            (layout.origin[0] + layout.size[0]) * 2.0 - 0.5,
            (layout.origin[1] + layout.size[1]) * 2.0 - 0.5,
            1.0 / target.width() as f32,
            1.0 / target.height() as f32,
            target.height() as f32,
            0.0,
        ];
        encoder.set_fragment_bytes(
            0,
            mem::size_of_val(&capture) as u64,
            capture.as_ptr().cast(),
        );
        encoder.set_fragment_texture(0, Some(background));
        encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, 3);
        encoder.end_encoding();

        let first_level = u64::from(!layout.omitted_base);
        let copy = CopyUniforms {
            base: [0, 1].map(|axis| (layout.aligned_origin[axis] - layout.origin[axis]) as i16),
            padding: [0; 2],
            clamp: [0, 0, layout.size[0] as i16 - 1, layout.size[1] as i16 - 1],
            base_size: layout
                .blur
                .map(|v| (v * if layout.omitted_base { 2 } else { 1 }) as u16),
            destination_size: layout.blur.map(|v| (v >> first_level) as u16),
            destination_level: first_level as u16,
            no_base_mip: u8::from(layout.omitted_base),
            tail: [0; 5],
        };
        if !layout.omitted_base {
            let encoder = render(command, &blurred, 0, metal::MTLLoadAction::DontCare);
            encoder.set_render_pipeline_state(&self.base);
            encoder.set_fragment_texture(0, Some(&captured));
            encoder.set_fragment_bytes(
                0,
                mem::size_of_val(&copy) as u64,
                (&copy as *const CopyUniforms).cast(),
            );
            encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, 3);
            encoder.end_encoding();
        }
        for level in first_level..layout.levels {
            let [width, height] = layout.blur.map(|v| v >> level);
            let first = level == first_level;
            let encoder = command.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(if first { &self.copy } else { &self.downsample });
            let block_width: u64 = if first { 32 } else { 16 };
            // SAFETY: MTLComputeCommandEncoder exposes this public selector with two NSUInteger arguments.
            unsafe {
                let _: () = msg_send![encoder, setImageblockWidth: block_width height: 32_u64];
            }
            encoder.set_texture(0, Some(if first { &captured } else { &blurred }));
            encoder.set_texture(1, Some(&staging));
            if first {
                encoder.set_bytes(
                    0,
                    mem::size_of_val(&copy) as u64,
                    (&copy as *const CopyUniforms).cast(),
                );
            } else {
                let uniform = DownsampleUniforms {
                    source_level: (level - 1) as u16,
                    destination_level: level as u16,
                    width: width as u16,
                    height: height as u16,
                    step: [1.0 / width as f32, 1.0 / height as f32],
                };
                encoder.set_bytes(
                    0,
                    mem::size_of_val(&uniform) as u64,
                    (&uniform as *const DownsampleUniforms).cast(),
                );
            }
            encoder.dispatch_thread_groups(
                MTLSize {
                    width: width.div_ceil(16),
                    height: height.div_ceil(if first { 16 } else { 32 }),
                    depth: 1,
                },
                MTLSize {
                    width: if first { 20 } else { 16 },
                    height: if first { 20 } else { 32 },
                    depth: 1,
                },
            );
            encoder.end_encoding();
            let encoder = render(command, &blurred, level, metal::MTLLoadAction::DontCare);
            encoder.set_render_pipeline_state(&self.store);
            encoder.set_fragment_texture(0, Some(&staging));
            let index = level as u32;
            encoder.set_fragment_bytes(
                0,
                mem::size_of_val(&index) as u64,
                (&index as *const u32).cast(),
            );
            encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, 3);
            encoder.end_encoding();
        }
        self.composite(
            command,
            Composite {
                target,
                source: &blurred,
                background: Some(background),
                backdrop,
                layout: &layout,
            },
            if target.pixel_format() == MTLPixelFormat::RGBA16Float {
                &self.unquantized
            } else {
                &self.composite
            },
        );
        Ok(())
    }

    pub(super) fn finish(
        &self,
        command: &metal::CommandBufferRef,
        target: &metal::TextureRef,
        source: &metal::TextureRef,
        backdrop: &ClearBackdrop,
    ) -> Result<()> {
        let layout = Layout::new(backdrop, target.height())?;
        self.composite(
            command,
            Composite {
                target,
                source,
                background: None,
                backdrop,
                layout: &layout,
            },
            &self.restore,
        );
        Ok(())
    }

    fn composite(
        &self,
        command: &metal::CommandBufferRef,
        input: Composite<'_>,
        pipeline: &metal::RenderPipelineStateRef,
    ) {
        let Composite {
            target,
            source,
            background,
            backdrop,
            layout,
        } = input;
        let encoder = render(
            command,
            target,
            0,
            if target.pixel_format() == MTLPixelFormat::RGBA16Float {
                metal::MTLLoadAction::Clear
            } else {
                metal::MTLLoadAction::Load
            },
        );
        encoder.set_render_pipeline_state(pipeline);
        encoder.set_fragment_texture(3, Some(source));
        if target.pixel_format() == MTLPixelFormat::RGBA16Float {
            encoder.set_fragment_texture(4, background);
        }
        let mvp = [
            2.0 / target.width() as f32,
            0.0,
            0.0,
            0.0,
            0.0,
            -2.0 / target.height() as f32,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            1.0,
            0.0,
            1.0,
        ];
        encoder.set_vertex_bytes(2, mem::size_of_val(&mvp) as u64, mvp.as_ptr().cast());
        let scale = backdrop.scale_factor;
        let b = backdrop.bounds;
        let size = [b.size.width.0 / scale, b.size.height.0 / scale];
        let radius = (backdrop.corner_radius.0 / scale).min(size[0].min(size[1]) / 2.0);
        let expanded = 1.528665_f32 * radius;
        let xs = [
            -1.0 / scale,
            expanded.min(size[0] / 2.0),
            size[0] - expanded.min(size[0] / 2.0),
            size[0] + 1.0 / scale,
        ];
        let ys = [
            -1.0 / scale,
            expanded.min(size[1] / 2.0),
            size[1] - expanded.min(size[1] / 2.0),
            size[1] + 1.0 / scale,
        ];
        let extent = layout
            .blur
            .map(|v| (v * if layout.omitted_base { 2 } else { 1 }) as f32);
        let face = if matches!(
            backdrop.appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ) {
            [
                0x3b32, 0xb170, 0xa45f, 0x2e3d, 0xaa76, 0x3a3e, 0xa465, 0x2e3d, 0xaa78, 0xb16f,
                0x3b76, 0x2e3d,
            ]
        } else {
            [
                0x3ad1, 0xb127, 0xa424, 0x347b, 0xaa1f, 0x39ea, 0xa42a, 0x347b, 0xaa21, 0xb126,
                0x3b12, 0x347b,
            ]
        };
        let clip = backdrop.content_mask.bounds;
        for corners in [true, false] {
            let uniform = Uniforms {
                shape: [
                    size[0] / 2.0,
                    size[1] / 2.0,
                    if corners { 4.0 } else { 0.0 },
                    radius,
                ],
                displacement: [scale / 2.0 / extent[0], 0.0, 0.0, -scale / 2.0 / extent[1]],
                face,
                blur_radius: layout.radius,
                opacity: backdrop.opacity,
                clip: [
                    clip.origin.x.0,
                    clip.origin.y.0,
                    clip.right().0,
                    clip.bottom().0,
                ],
            };
            let mut vertices = Vec::with_capacity(30);
            for y in 0..3 {
                for x in 0..3 {
                    if (x != 1 && y != 1) != corners {
                        continue;
                    }
                    for (ix, iy) in [
                        (x, y + 1),
                        (x + 1, y + 1),
                        (x + 1, y),
                        (x + 1, y),
                        (x, y),
                        (x, y + 1),
                    ] {
                        let local = [xs[ix], ys[iy]];
                        let point = [
                            b.origin.x.0 / 2.0 + local[0] * scale / 2.0,
                            (target.height() as f32 - b.bottom().0) / 2.0 + local[1] * scale / 2.0,
                        ];
                        vertices.push(Vertex {
                            position: [
                                point[0] * 2.0,
                                target.height() as f32 - point[1] * 2.0,
                                0.0,
                                1.0,
                            ],
                            sdf: [local[0] - size[0] / 2.0, size[1] / 2.0 - local[1]],
                            uv: [
                                (point[0] - layout.aligned_origin[0]) / extent[0],
                                (point[1] - layout.aligned_origin[1]) / extent[1],
                            ],
                        });
                    }
                }
            }
            encoder.set_fragment_bytes(
                1,
                mem::size_of_val(&uniform) as u64,
                (&uniform as *const Uniforms).cast(),
            );
            encoder.set_vertex_bytes(
                1,
                mem::size_of_val(vertices.as_slice()) as u64,
                vertices.as_ptr().cast(),
            );
            encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
        }
        encoder.end_encoding();
    }
}

fn texture(
    device: &metal::DeviceRef,
    size: [u64; 2],
    levels: u64,
    format: MTLPixelFormat,
) -> metal::Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_width(size[0]);
    descriptor.set_height(size[1]);
    descriptor.set_mipmap_level_count(levels);
    descriptor.set_pixel_format(format);
    descriptor.set_storage_mode(metal::MTLStorageMode::Private);
    if format == MTLPixelFormat::RGBA16Float {
        descriptor.set_usage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
    } else {
        // Lossy storage requires a render target. Shader writes use the half-float staging texture.
        descriptor.set_usage(MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget);
        descriptor.set_compression_type(metal::MTLTextureCompressionType::Lossy);
    }
    device.new_texture(&descriptor)
}

fn render<'a>(
    command: &'a metal::CommandBufferRef,
    target: &metal::TextureRef,
    level: u64,
    load: metal::MTLLoadAction,
) -> &'a metal::RenderCommandEncoderRef {
    let pass = metal::RenderPassDescriptor::new();
    let attachment = pass
        .color_attachments()
        .object_at(0)
        .expect("color attachment 0");
    attachment.set_texture(Some(target));
    attachment.set_level(level);
    attachment.set_load_action(load);
    attachment.set_store_action(metal::MTLStoreAction::Store);
    attachment.set_clear_color(metal::MTLClearColor::new(0.0, 0.0, 0.0, 0.0));
    let encoder = command.new_render_command_encoder(pass);
    encoder.set_viewport(metal::MTLViewport {
        originX: 0.0,
        originY: 0.0,
        width: (target.width() >> level) as f64,
        height: (target.height() >> level) as f64,
        znear: 0.0,
        zfar: 1.0,
    });
    encoder
}
