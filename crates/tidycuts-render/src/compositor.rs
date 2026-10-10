//! The GPU compositor: turns a [`Frame`] description into pixels.
//!
//! Every layer goes through the same chain of full-screen passes:
//! upload → source (YUV/RGBA to premultiplied RGBA) → enhance (denoise,
//! deblock, upscale, sharpen) → color grade → effects, and is then placed on
//! the canvas with its transform, crop, opacity and blend mode. Transitions
//! place both pictures on their own canvases and mix them. Intermediate
//! textures are 16-bit float and come from a pool, so a frame allocates
//! nothing once the pool is warm.

use crate::frame::*;
use crate::looks_gpu::{GradeLayouts, LookCache};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

const WORK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub(crate) const SHADER_SOURCE: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/place.wgsl"),
    include_str!("shaders/grade.wgsl"),
    include_str!("shaders/enhance.wgsl"),
    include_str!("shaders/effects.wgsl"),
    include_str!("shaders/transitions.wgsl"),
    include_str!("shaders/layout.wgsl"),
);

/// Uniforms shared by every pass; see `common.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
pub(crate) struct Params {
    pub dst_size: [f32; 2],
    pub src_size: [f32; 2],
    pub time: f32,
    pub progress: f32,
    pub kind: u32,
    pub flags: u32,
    pub p: [[f32; 4]; 4],
    pub m0: [f32; 4],
    pub m1: [f32; 4],
    pub c0: [f32; 4],
    pub c1: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Pass {
    SourceYuv,
    SourceRgba,
    Solid,
    Copy,
    Mix,
    Output,
    /// The final pass for video files, one plane at a time (see
    /// [`Nv12Target`]).
    OutputY,
    OutputUv,
    Place,
    Grade,
    Effect,
    Transition,
    Denoise,
    Deblock,
    Sharpen,
    Upscale,
    Over,
    Shadow,
    Matte,
    Cell,
    Visualizer,
}

impl Pass {
    fn entry(self) -> &'static str {
        match self {
            Pass::SourceYuv => "fs_source_yuv",
            Pass::SourceRgba => "fs_source_rgba",
            Pass::Solid => "fs_solid",
            Pass::Copy => "fs_copy",
            Pass::Mix => "fs_mix",
            Pass::Output => "fs_output",
            Pass::OutputY => "fs_output_y",
            Pass::OutputUv => "fs_output_uv",
            Pass::Place => "fs_place",
            Pass::Grade => "fs_grade",
            Pass::Effect => "fs_effect",
            Pass::Transition => "fs_transition",
            Pass::Denoise => "fs_denoise",
            Pass::Deblock => "fs_deblock",
            Pass::Sharpen => "fs_sharpen",
            Pass::Upscale => "fs_upscale",
            Pass::Over => "fs_over",
            Pass::Shadow => "fs_shadow",
            Pass::Matte => "fs_matte",
            Pass::Cell => "fs_cell",
            Pass::Visualizer => "fs_visualizer",
        }
    }
}

/// GPU objects that every compositor on a device can share.
pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    module: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: Mutex<HashMap<(Pass, wgpu::TextureFormat), wgpu::RenderPipeline>>,
    sampler: wgpu::Sampler,
    dummy: wgpu::TextureView,
    /// The grade pass's second bind group (HSL, curves and LUT).
    grade_layouts: GradeLayouts,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Arc<Renderer> {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tidycuts shaders"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tidycuts pass"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex_entry(2),
                tex_entry(3),
                tex_entry(4),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tidycuts pass"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tidycuts linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let dummy_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("tidycuts dummy"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[0, 0, 0, 0],
        );
        let dummy = dummy_tex.create_view(&Default::default());
        let grade_layouts = GradeLayouts::new(&device, &queue, &layout);
        Arc::new(Renderer {
            device,
            queue,
            module,
            layout,
            pipeline_layout,
            pipelines: Mutex::new(HashMap::new()),
            sampler,
            dummy,
            grade_layouts,
        })
    }

    fn pipeline(&self, pass: Pass, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let mut map = self.pipelines.lock().unwrap();
        map.entry((pass, format))
            .or_insert_with(|| {
                self.device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(pass.entry()),
                        layout: Some(if pass == Pass::Grade {
                            &self.grade_layouts.pipeline_layout
                        } else {
                            &self.pipeline_layout
                        }),
                        vertex: wgpu::VertexState {
                            module: &self.module,
                            entry_point: Some("vs_full"),
                            compilation_options: Default::default(),
                            buffers: &[],
                        },
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &self.module,
                            entry_point: Some(pass.entry()),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    })
            })
            .clone()
    }

    /// Builds every pipeline up front so the first frame doesn't stall.
    pub fn warm_up(&self, output: wgpu::TextureFormat) {
        use Pass::*;
        for p in [
            SourceYuv, SourceRgba, Solid, Copy, Mix, Place, Grade, Effect, Transition, Denoise,
            Deblock, Sharpen, Upscale, Cell, Visualizer,
        ] {
            self.pipeline(p, WORK_FORMAT);
        }
        self.pipeline(Output, output);
        self.pipeline(OutputY, wgpu::TextureFormat::R8Unorm);
        self.pipeline(OutputUv, wgpu::TextureFormat::Rg8Unorm);
    }
}

/// A texture the compositor can render into: the preview display or an
/// export buffer.
pub struct Target {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// The same picture seen without the sRGB decoding, as egui wants it:
    /// egui expects the colours as they are stored. Showing the sRGB view
    /// would darken the picture a second time.
    pub display_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

/// The picture of a video file in the form encoders want: a brightness plane
/// and a half-size colour plane, drawn by the graphics card (see
/// [`Compositor::submit_nv12`]).
pub struct Nv12Target {
    y: wgpu::Texture,
    uv: wgpu::Texture,
    y_view: wgpu::TextureView,
    uv_view: wgpu::TextureView,
    y_buf: wgpu::Buffer,
    uv_buf: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
    /// Bytes per row in the read-back buffers.
    pitch: u32,
}

struct PoolTex {
    view: wgpu::TextureView,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    busy: bool,
    idle_frames: u32,
    _tex: wgpu::Texture,
}

#[derive(Default)]
struct Pool {
    items: Vec<PoolTex>,
}

impl Pool {
    fn get(
        &mut self,
        device: &wgpu::Device,
        w: u32,
        h: u32,
        format: wgpu::TextureFormat,
        upload: bool,
    ) -> usize {
        if let Some(i) = self
            .items
            .iter()
            .position(|t| !t.busy && t.w == w && t.h == h && t.format == format)
        {
            self.items[i].busy = true;
            self.items[i].idle_frames = 0;
            return i;
        }
        let usage = if upload {
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
        } else {
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tidycuts pool"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.items.push(PoolTex {
            view,
            w,
            h,
            format,
            busy: true,
            idle_frames: 0,
            _tex: tex,
        });
        self.items.len() - 1
    }

    fn texture(&self, i: usize) -> &wgpu::Texture {
        &self.items[i]._tex
    }

    fn release(&mut self, i: usize) {
        self.items[i].busy = false;
    }

    fn end_frame(&mut self) {
        for t in &mut self.items {
            if !t.busy {
                t.idle_frames += 1;
            }
            t.busy = false;
        }
        // Drop textures nobody has used for a while (e.g. after a resize).
        let mut i = 0;
        while i < self.items.len() {
            if self.items[i].idle_frames > 90 {
                self.items.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
}

struct Static {
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    w: u32,
    h: u32,
    last_used: u64,
}

/// A rendered layer waiting to be placed: a work texture plus how to sample it.
struct Processed {
    tex: usize,
    w: u32,
    h: u32,
    crop: [f32; 4],
    bicubic: bool,
}

/// Renders frames on the GPU. One per thread of work (the preview and each
/// export get their own); they can share a [`Renderer`].
pub struct Compositor {
    r: Arc<Renderer>,
    work: Pool,
    uploads: Pool,
    statics: HashMap<u64, Static>,
    looks: LookCache,
    frame_no: u64,
}

fn tex_view(pool: &Pool, i: usize) -> &wgpu::TextureView {
    &pool.items[i].view
}

impl Compositor {
    pub fn new(r: Arc<Renderer>) -> Self {
        Compositor {
            r,
            work: Pool::default(),
            uploads: Pool::default(),
            statics: HashMap::new(),
            looks: LookCache::default(),
            frame_no: 0,
        }
    }

    pub fn renderer(&self) -> &Arc<Renderer> {
        &self.r
    }

    /// Creates a render target. `readback` targets can be copied to the CPU;
    /// display targets can be shown by egui.
    pub fn make_target(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        readback: bool,
    ) -> Target {
        let mut usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        // An sRGB target can also be looked at as plain bytes.
        let raw = format.remove_srgb_suffix();
        let raw_formats = if raw != format { vec![raw] } else { Vec::new() };
        if readback {
            usage |= wgpu::TextureUsages::COPY_SRC;
        }
        let texture = self.r.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tidycuts target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &raw_formats,
        });
        let view = texture.create_view(&Default::default());
        let display_view = if raw_formats.is_empty() {
            texture.create_view(&Default::default())
        } else {
            texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(raw),
                ..Default::default()
            })
        };
        Target {
            texture,
            view,
            display_view,
            width: width.max(1),
            height: height.max(1),
            format,
        }
    }

    /// Whether a static picture (image, title, sticker) is already on the GPU.
    pub fn has_static(&mut self, key: u64) -> bool {
        match self.statics.get_mut(&key) {
            Some(s) => {
                s.last_used = self.frame_no;
                true
            }
            None => false,
        }
    }

    /// Uploads a static picture (premultiplied RGBA) under `key`.
    pub fn store_static(&mut self, key: u64, image: &RgbaImage) {
        let (w, h) = (image.width.max(1), image.height.max(1));
        let tex = self.r.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tidycuts static"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if !image.pixels.is_empty() {
            self.r.queue.write_texture(
                tex.as_image_copy(),
                &image.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = tex.create_view(&Default::default());
        self.statics.insert(
            key,
            Static {
                tex,
                view,
                w,
                h,
                last_used: self.frame_no,
            },
        );
    }

    /// Renders a frame into `target`.
    pub fn render(&mut self, frame: &Frame, target: &Target) {
        let mut enc = self
            .r
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tidycuts frame"),
            });
        self.encode(&mut enc, frame, target);
        self.r.queue.submit([enc.finish()]);
        self.finish_frame();
    }

    /// Renders a frame and copies it to the CPU as tightly packed RGBA8
    /// (gamma-encoded, opaque). `target` must be a readback `Rgba8Unorm` target.
    pub fn render_to_cpu(&mut self, frame: &Frame, target: &Target) -> Vec<u8> {
        let (w, h) = (target.width, target.height);
        let padded = (w * 4).div_ceil(256) * 256;
        let buffer = self.r.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tidycuts readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .r
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tidycuts frame"),
            });
        self.encode(&mut enc, frame, target);
        enc.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let index = self.r.queue.submit([enc.finish()]);
        self.finish_frame();
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.r.device.poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        });
        let Ok(data) = slice.get_mapped_range() else {
            return vec![0; (w * h * 4) as usize];
        };
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h as usize {
            let start = row * padded as usize;
            out.extend_from_slice(&data[start..start + w as usize * 4]);
        }
        drop(data);
        buffer.unmap();
        out
    }

    /// Copies a finished target to the CPU as tightly packed RGBA8, exactly
    /// as it is stored (for an sRGB target that is the gamma-encoded picture
    /// the screen shows). The target must have been made with `readback`.
    pub fn read_target(&self, target: &Target) -> Vec<u8> {
        let (w, h) = (target.width, target.height);
        let padded = (w * 4).div_ceil(256) * 256;
        let buffer = self.r.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tidycuts target readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .r
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tidycuts target copy"),
            });
        enc.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let index = self.r.queue.submit([enc.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.r.device.poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        });
        let Ok(data) = slice.get_mapped_range() else {
            return vec![0; (w * h * 4) as usize];
        };
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h as usize {
            let start = row * padded as usize;
            out.extend_from_slice(&data[start..start + w as usize * 4]);
        }
        drop(data);
        buffer.unmap();
        out
    }

    /// Planes and read-back buffers for [`Compositor::submit_nv12`].
    pub fn make_nv12_target(&self, width: u32, height: u32) -> Nv12Target {
        let (w, h) = (width.max(2) & !1, height.max(2) & !1);
        let make = |tw: u32, th: u32, format| {
            self.r.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tidycuts nv12 plane"),
                size: wgpu::Extent3d {
                    width: tw,
                    height: th,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let y = make(w, h, wgpu::TextureFormat::R8Unorm);
        let uv = make(w / 2, h / 2, wgpu::TextureFormat::Rg8Unorm);
        // Both planes have `w` bytes per row (the colour plane is half as wide
        // with two bytes per texel); rows are padded for the copy.
        let pitch = w.div_ceil(256) * 256;
        let buffer = |rows: u32| {
            self.r.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tidycuts nv12 readback"),
                size: (pitch * rows) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };
        Nv12Target {
            y_view: y.create_view(&Default::default()),
            uv_view: uv.create_view(&Default::default()),
            y,
            uv,
            y_buf: buffer(h),
            uv_buf: buffer(h / 2),
            width: w,
            height: h,
            pitch,
        }
    }

    /// Starts drawing `frame` as video-file colours (8-bit, limited range,
    /// BT.709, the colour half the size, like NV12) and copying it to
    /// `target`'s buffers. Doesn't wait: call [`Compositor::read_nv12`] with
    /// the returned index once the next frame has been got ready. Compared
    /// with reading back RGBA, this moves less than half the data and leaves
    /// ffmpeg no colour conversion to do.
    pub fn submit_nv12(&mut self, frame: &Frame, t: &Nv12Target) -> wgpu::SubmissionIndex {
        let (w, h) = (t.width, t.height);
        let mut enc = self
            .r
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tidycuts nv12 frame"),
            });
        let canvas = self.build_canvas(&mut enc, frame, w, h);
        let bg = frame.background;
        for (pass, format, view, size) in [
            (
                Pass::OutputY,
                wgpu::TextureFormat::R8Unorm,
                &t.y_view,
                (w, h),
            ),
            (
                Pass::OutputUv,
                wgpu::TextureFormat::Rg8Unorm,
                &t.uv_view,
                (w / 2, h / 2),
            ),
        ] {
            let params = Params {
                dst_size: [size.0 as f32, size.1 as f32],
                src_size: [w as f32, h as f32],
                c0: [bg[0], bg[1], bg[2], 1.0],
                ..Default::default()
            };
            let pipeline = self.r.pipeline(pass, format);
            let bind = self.bind(&params, [Some(tex_view(&self.work, canvas)), None, None]);
            run_pass(&mut enc, &pipeline, &bind, view, None);
        }
        self.work.release(canvas);
        for (tex, buf, tw, th) in [(&t.y, &t.y_buf, w, h), (&t.uv, &t.uv_buf, w / 2, h / 2)] {
            enc.copy_texture_to_buffer(
                tex.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(t.pitch),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: tw,
                    height: th,
                    depth_or_array_layers: 1,
                },
            );
        }
        let index = self.r.queue.submit([enc.finish()]);
        self.finish_frame();
        t.y_buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        t.uv_buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        index
    }

    /// Waits for a frame started with [`Compositor::submit_nv12`] and returns
    /// it as tightly packed NV12: all the brightness rows, then the
    /// interleaved colour rows (`width * height * 3 / 2` bytes).
    pub fn read_nv12(&self, t: &Nv12Target, index: wgpu::SubmissionIndex) -> Vec<u8> {
        let _ = self.r.device.poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        });
        let (w, h) = (t.width as usize, t.height as usize);
        let pitch = t.pitch as usize;
        let mut out = Vec::with_capacity(w * h * 3 / 2);
        for (buf, rows) in [(&t.y_buf, h), (&t.uv_buf, h / 2)] {
            match buf.slice(..).get_mapped_range() {
                Ok(data) => {
                    for row in 0..rows {
                        out.extend_from_slice(&data[row * pitch..row * pitch + w]);
                    }
                }
                Err(_) => out.resize(out.len() + w * rows, 0),
            }
            buf.unmap();
        }
        out
    }

    fn finish_frame(&mut self) {
        self.work.end_frame();
        self.uploads.end_frame();
        self.looks.end_frame(self.frame_no);
        self.frame_no += 1;
        let now = self.frame_no;
        // Forget pictures not shown for a while (titles that were edited etc.).
        self.statics.retain(|_, s| now - s.last_used < 600);
    }

    fn encode(&mut self, enc: &mut wgpu::CommandEncoder, frame: &Frame, target: &Target) {
        let (w, h) = (target.width, target.height);
        let canvas = self.build_canvas(enc, frame, w, h);
        let bg = frame.background;
        let params = Params {
            dst_size: [w as f32, h as f32],
            src_size: [w as f32, h as f32],
            flags: if target.format.is_srgb() { 1 } else { 0 },
            c0: [bg[0], bg[1], bg[2], 1.0],
            ..Default::default()
        };
        let pipeline = self.r.pipeline(Pass::Output, target.format);
        let bind = self.bind(&params, [Some(tex_view(&self.work, canvas)), None, None]);
        run_pass(enc, &pipeline, &bind, &target.view, None);
        self.work.release(canvas);
    }

    /// Draws every layer of `frame` into a work texture and returns its
    /// index; the caller runs the final pass and releases it.
    fn build_canvas(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        frame: &Frame,
        w: u32,
        h: u32,
    ) -> usize {
        let scale = w as f32 / frame.canvas[0].max(1.0);
        let bg = frame.background;
        let mut canvas = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        self.clear(
            enc,
            canvas,
            [bg[0] * bg[3], bg[1] * bg[3], bg[2] * bg[3], bg[3]],
        );

        for item in &frame.items {
            match item {
                Item::Layer(layer) => {
                    if let Some(p) = self.process_layer(enc, layer, scale, (w, h)) {
                        if let Some(sh) = &layer.shadow {
                            canvas = self.drop_shadow(enc, &p, layer, sh, scale, canvas, (w, h));
                        }
                        let next = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                        self.place(enc, &p, layer, scale, canvas, next, (w, h));
                        self.work.release(p.tex);
                        self.work.release(canvas);
                        canvas = next;
                    }
                }
                Item::Matte(m) => {
                    let a = self.placed_alone(enc, &m.layer, scale, (w, h));
                    let b = self.placed_alone(enc, &m.matte, scale, (w, h));
                    let next = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                    let params = Params {
                        dst_size: [w as f32, h as f32],
                        src_size: [w as f32, h as f32],
                        kind: m.mode,
                        ..Default::default()
                    };
                    self.pass_tex(
                        enc,
                        Pass::Matte,
                        &params,
                        [Some(a), Some(b), Some(canvas)],
                        next,
                    );
                    self.work.release(a);
                    self.work.release(b);
                    self.work.release(canvas);
                    canvas = next;
                }
                Item::Transition(tr) => {
                    let mut side = |this: &mut Self, l: &Option<Layer>| -> usize {
                        match l {
                            Some(layer) => this.placed_alone(enc, layer, scale, (w, h)),
                            None => {
                                let t = this.work.get(&this.r.device, w, h, WORK_FORMAT, false);
                                this.clear(enc, t, [0.0; 4]);
                                t
                            }
                        }
                    };
                    let a = side(self, &tr.from);
                    let b = side(self, &tr.to);
                    let next = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                    let mut params = Params {
                        dst_size: [w as f32, h as f32],
                        src_size: [w as f32, h as f32],
                        time: tr.time,
                        progress: tr.progress,
                        kind: tr.kind,
                        ..Default::default()
                    };
                    params.p = tr.params;
                    params.c0 = tr.color;
                    self.pass_tex(
                        enc,
                        Pass::Transition,
                        &params,
                        [Some(a), Some(b), Some(canvas)],
                        next,
                    );
                    self.work.release(a);
                    self.work.release(b);
                    self.work.release(canvas);
                    canvas = next;
                }
                Item::Adjust(adj) => {
                    let before = canvas;
                    let mut cur = canvas;
                    let mut keep_before = false;
                    if let Some(g) = &adj.grade {
                        let next = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                        self.grade(enc, g, adj.time, cur, next, (w, h));
                        keep_before = true;
                        cur = next;
                    }
                    for fx in &adj.effects {
                        for pass in 0..fx.passes.max(1) {
                            let next = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                            self.effect(enc, fx, pass, cur, next, (w, h));
                            if cur != before {
                                self.work.release(cur);
                            }
                            keep_before = true;
                            cur = next;
                        }
                    }
                    if keep_before && adj.opacity < 0.999 {
                        let next = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                        let params = Params {
                            dst_size: [w as f32, h as f32],
                            src_size: [w as f32, h as f32],
                            p: [[adj.opacity, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]],
                            ..Default::default()
                        };
                        self.pass_tex(
                            enc,
                            Pass::Mix,
                            &params,
                            [Some(cur), Some(before), None],
                            next,
                        );
                        self.work.release(cur);
                        cur = next;
                    }
                    if cur != before {
                        self.work.release(before);
                    }
                    canvas = cur;
                }
            }
        }

        canvas
    }

    fn clear(&mut self, enc: &mut wgpu::CommandEncoder, tex: usize, color: [f32; 4]) {
        let view = tex_view(&self.work, tex);
        let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tidycuts clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: color[0] as f64,
                        g: color[1] as f64,
                        b: color[2] as f64,
                        a: color[3] as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }

    fn bind(&self, params: &Params, texes: [Option<&wgpu::TextureView>; 3]) -> wgpu::BindGroup {
        let buf = self
            .r
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tidycuts params"),
                contents: bytemuck::bytes_of(params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let t = |i: usize| texes[i].unwrap_or(&self.r.dummy);
        self.r.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tidycuts pass"),
            layout: &self.r.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.r.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(t(0)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(t(1)),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(t(2)),
                },
            ],
        })
    }

    /// Runs a pass reading work textures and writing work texture `out`.
    fn pass_tex(
        &self,
        enc: &mut wgpu::CommandEncoder,
        pass: Pass,
        params: &Params,
        inputs: [Option<usize>; 3],
        out: usize,
    ) {
        let pipeline = self.r.pipeline(pass, WORK_FORMAT);
        let views = inputs.map(|i| i.map(|i| tex_view(&self.work, i)));
        let bind = self.bind(params, views);
        run_pass(enc, &pipeline, &bind, tex_view(&self.work, out), None);
    }

    /// Uploads a layer's pixels and converts them to a premultiplied work texture.
    fn source(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        layer: &Layer,
        scale: f32,
        out_size: (u32, u32),
    ) -> Option<(usize, u32, u32)> {
        let device = self.r.device.clone();
        match &layer.source {
            Source::Pixels(Pixels::Rgba(img)) => {
                let (w, h) = (img.width.max(1), img.height.max(1));
                if img.pixels.len() < (w * h * 4) as usize {
                    return None;
                }
                let up = self
                    .uploads
                    .get(&device, w, h, wgpu::TextureFormat::Rgba8Unorm, true);
                self.write_plane(up, &img.pixels, w, h, 4);
                let out = self.work.get(&device, w, h, WORK_FORMAT, false);
                let params = Params {
                    dst_size: [w as f32, h as f32],
                    src_size: [w as f32, h as f32],
                    flags: if img.premultiplied { 0 } else { 1 },
                    ..Default::default()
                };
                let pipeline = self.r.pipeline(Pass::SourceRgba, WORK_FORMAT);
                let bind = self.bind(&params, [Some(tex_view(&self.uploads, up)), None, None]);
                run_pass(enc, &pipeline, &bind, tex_view(&self.work, out), None);
                Some((out, w, h))
            }
            Source::Pixels(Pixels::Yuv420(f)) => {
                let (w, h) = (f.width.max(2), f.height.max(2));
                let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
                let ylen = (w * h) as usize;
                let clen = (cw * ch) as usize;
                if f.data.len() < ylen + 2 * clen {
                    return None;
                }
                let yt = self
                    .uploads
                    .get(&device, w, h, wgpu::TextureFormat::R8Unorm, true);
                let ut = self
                    .uploads
                    .get(&device, cw, ch, wgpu::TextureFormat::R8Unorm, true);
                let vt = self
                    .uploads
                    .get(&device, cw, ch, wgpu::TextureFormat::R8Unorm, true);
                self.write_plane(yt, &f.data[..ylen], w, h, 1);
                self.write_plane(ut, &f.data[ylen..ylen + clen], cw, ch, 1);
                self.write_plane(vt, &f.data[ylen + clen..ylen + 2 * clen], cw, ch, 1);
                let out = self.work.get(&device, w, h, WORK_FORMAT, false);
                let (yo, ys, cs) = if f.full_range {
                    (0.0, 1.0, 1.0)
                } else {
                    (16.0 / 255.0, 255.0 / 219.0, 255.0 / 224.0)
                };
                let coeffs = match f.matrix {
                    YuvMatrix::Bt601 => [1.402, 0.344136, 0.714136, 1.772],
                    YuvMatrix::Bt709 => [1.5748, 0.1873, 0.4681, 1.8556],
                    YuvMatrix::Bt2020 => [1.4746, 0.16455, 0.57135, 1.8814],
                };
                let params = Params {
                    dst_size: [w as f32, h as f32],
                    src_size: [w as f32, h as f32],
                    p: [[yo, ys, cs, 1.0], coeffs, [0.0; 4], [0.0; 4]],
                    ..Default::default()
                };
                let pipeline = self.r.pipeline(Pass::SourceYuv, WORK_FORMAT);
                let bind = self.bind(
                    &params,
                    [
                        Some(tex_view(&self.uploads, yt)),
                        Some(tex_view(&self.uploads, ut)),
                        Some(tex_view(&self.uploads, vt)),
                    ],
                );
                run_pass(enc, &pipeline, &bind, tex_view(&self.work, out), None);
                Some((out, w, h))
            }
            Source::Cached(key) => {
                let s = self.statics.get_mut(key)?;
                s.last_used = self.frame_no;
                let (w, h) = (s.w, s.h);
                let view = s.view.clone();
                let _keep = &s.tex;
                let out = self.work.get(&device, w, h, WORK_FORMAT, false);
                let params = Params {
                    dst_size: [w as f32, h as f32],
                    src_size: [w as f32, h as f32],
                    ..Default::default()
                };
                let pipeline = self.r.pipeline(Pass::SourceRgba, WORK_FORMAT);
                let bind = self.bind(&params, [Some(&view), None, None]);
                run_pass(enc, &pipeline, &bind, tex_view(&self.work, out), None);
                Some((out, w, h))
            }
            Source::Visualizer(v) => {
                // Levels as a one-row texture, then the bars drawn at the
                // size they'll appear.
                let n = v.levels.len().clamp(1, 64) as u32;
                let bytes: Vec<u8> = (0..n as usize)
                    .map(|i| {
                        let l = v.levels.get(i).copied().unwrap_or(0.0);
                        (l.clamp(0.0, 1.0) * 255.0).round() as u8
                    })
                    .collect();
                let up = self
                    .uploads
                    .get(&device, n, 1, wgpu::TextureFormat::R8Unorm, true);
                self.write_plane(up, &bytes, n, 1, 1);
                let fw = (layer.base_size[0] * scale * layer.scale[0].abs().max(0.05)).round();
                let fh = (layer.base_size[1] * scale * layer.scale[1].abs().max(0.05)).round();
                let w = (fw as u32).clamp(2, out_size.0.max(2) * 2);
                let h = (fh as u32).clamp(2, out_size.1.max(2) * 2);
                let out = self.work.get(&device, w, h, WORK_FORMAT, false);
                let params = Params {
                    dst_size: [w as f32, h as f32],
                    src_size: [n as f32, 1.0],
                    p: [
                        [
                            v.style as f32,
                            n as f32,
                            if v.mirror { 1.0 } else { 0.0 },
                            v.thickness,
                        ],
                        [0.0; 4],
                        [0.0; 4],
                        [0.0; 4],
                    ],
                    c0: v.colors[0],
                    c1: v.colors[1],
                    ..Default::default()
                };
                let pipeline = self.r.pipeline(Pass::Visualizer, WORK_FORMAT);
                let bind = self.bind(&params, [Some(tex_view(&self.uploads, up)), None, None]);
                run_pass(enc, &pipeline, &bind, tex_view(&self.work, out), None);
                Some((out, w, h))
            }
            Source::Solid(color) | Source::Gradient(color, _, _) => {
                // Generated pictures are drawn at the size they'll appear.
                let fw = (layer.base_size[0] * scale * layer.scale[0].abs().max(0.05)).round();
                let fh = (layer.base_size[1] * scale * layer.scale[1].abs().max(0.05)).round();
                let w = (fw as u32).clamp(2, out_size.0.max(2) * 2);
                let h = (fh as u32).clamp(2, out_size.1.max(2) * 2);
                let out = self.work.get(&device, w, h, WORK_FORMAT, false);
                let mut params = Params {
                    dst_size: [w as f32, h as f32],
                    src_size: [w as f32, h as f32],
                    c0: *color,
                    ..Default::default()
                };
                if let Source::Gradient(_, b, angle) = &layer.source {
                    params.flags = 1;
                    params.c1 = *b;
                    params.p[0][0] = angle.to_radians();
                }
                let pipeline = self.r.pipeline(Pass::Solid, WORK_FORMAT);
                let bind = self.bind(&params, [None, None, None]);
                run_pass(enc, &pipeline, &bind, tex_view(&self.work, out), None);
                Some((out, w, h))
            }
        }
    }

    fn write_plane(&self, up: usize, data: &[u8], w: u32, h: u32, bpp: u32) {
        self.r.queue.write_texture(
            self.uploads.texture(up).as_image_copy(),
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * bpp),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    fn simple(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        pass: Pass,
        input: usize,
        size: (u32, u32),
        p0: [f32; 4],
    ) -> usize {
        let out = self
            .work
            .get(&self.r.device, size.0, size.1, WORK_FORMAT, false);
        let params = Params {
            dst_size: [size.0 as f32, size.1 as f32],
            src_size: [size.0 as f32, size.1 as f32],
            p: [p0, [0.0; 4], [0.0; 4], [0.0; 4]],
            ..Default::default()
        };
        self.pass_tex(enc, pass, &params, [Some(input), None, None], out);
        self.work.release(input);
        out
    }

    fn grade(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        g: &Grade,
        time: f32,
        input: usize,
        out: usize,
        size: (u32, u32),
    ) {
        let (looks, flags, lut_strength) = self.looks.prepare(
            &self.r.device,
            &self.r.queue,
            &self.r.grade_layouts,
            g.looks.as_deref(),
            self.frame_no,
        );
        let params = Params {
            dst_size: [size.0 as f32, size.1 as f32],
            src_size: [size.0 as f32, size.1 as f32],
            time,
            progress: lut_strength,
            flags,
            p: [
                [g.exposure, g.brightness, g.contrast, g.saturation],
                [g.highlights, g.shadows, g.whites, g.blacks],
                [g.temperature, g.tint, g.vibrance, g.hue.to_radians()],
                [
                    g.vignette,
                    g.fade,
                    g.strength,
                    if g.mono { 1.0 } else { 0.0 },
                ],
            ],
            m0: [g.lift[0], g.lift[1], g.lift[2], g.grain],
            m1: [g.gain[0], g.gain[1], g.gain[2], g.gamma],
            c0: g.shadow_tint,
            c1: g.highlight_tint,
            ..Default::default()
        };
        let pipeline = self.r.pipeline(Pass::Grade, WORK_FORMAT);
        let bind = self.bind(&params, [Some(tex_view(&self.work, input)), None, None]);
        run_pass_groups(
            enc,
            &pipeline,
            &[&bind, &looks],
            tex_view(&self.work, out),
            None,
        );
    }

    fn effect(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        fx: &Fx,
        pass: u32,
        input: usize,
        out: usize,
        size: (u32, u32),
    ) {
        let params = Params {
            dst_size: [size.0 as f32, size.1 as f32],
            src_size: [size.0 as f32, size.1 as f32],
            time: fx.time,
            kind: fx.shader,
            flags: pass,
            p: fx.params,
            c0: fx.colors[0],
            c1: fx.colors[1],
            ..Default::default()
        };
        self.pass_tex(enc, Pass::Effect, &params, [Some(input), None, None], out);
    }

    /// Source, enhance, grade and effects for one layer.
    fn process_layer(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        layer: &Layer,
        scale: f32,
        out_size: (u32, u32),
    ) -> Option<Processed> {
        if layer.opacity <= 0.0 {
            return None;
        }
        let (mut cur, mut w, mut h) = self.source(enc, layer, scale, out_size)?;
        let mut crop = layer.crop;
        let mut bicubic = false;
        let e = &layer.enhance;
        if e.denoise > 0.0 {
            cur = self.simple(enc, Pass::Denoise, cur, (w, h), [e.denoise, 0.0, 0.0, 0.0]);
        }
        if e.deblock > 0.0 {
            cur = self.simple(
                enc,
                Pass::Deblock,
                cur,
                (w, h),
                [e.deblock, e.block_size.max(2.0), 0.0, 0.0],
            );
        }
        // How many output pixels each texel of the visible part covers.
        let foot_w = layer.base_size[0] * scale * layer.scale[0].abs();
        let foot_h = layer.base_size[1] * scale * layer.scale[1].abs();
        let vis_w = w as f32 * (crop[2] - crop[0]).max(0.01);
        let vis_h = h as f32 * (crop[3] - crop[1]).max(0.01);
        let enlarge = (foot_w / vis_w).max(foot_h / vis_h);
        let mut sharpen = e.sharpen;
        if enlarge > 1.15 {
            match e.upscale {
                1 => bicubic = true,
                2 => {
                    let tw = (foot_w.round() as u32).clamp(2, out_size.0 * 2);
                    let th = (foot_h.round() as u32).clamp(2, out_size.1 * 2);
                    let out = self.work.get(&self.r.device, tw, th, WORK_FORMAT, false);
                    let params = Params {
                        dst_size: [tw as f32, th as f32],
                        src_size: [w as f32, h as f32],
                        p: [crop, [0.0; 4], [0.0; 4], [0.0; 4]],
                        ..Default::default()
                    };
                    self.pass_tex(enc, Pass::Upscale, &params, [Some(cur), None, None], out);
                    self.work.release(cur);
                    cur = out;
                    (w, h) = (tw, th);
                    crop = [0.0, 0.0, 1.0, 1.0];
                    sharpen = sharpen.max(0.45);
                }
                _ => {}
            }
        }
        if sharpen > 0.0 {
            cur = self.simple(enc, Pass::Sharpen, cur, (w, h), [sharpen, 0.0, 0.0, 0.0]);
        }
        for g in layer.grade.iter().chain(layer.filters.iter()) {
            let out = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
            self.grade(enc, g, layer.time, cur, out, (w, h));
            self.work.release(cur);
            cur = out;
        }
        for fx in &layer.effects {
            for pass in 0..fx.passes.max(1) {
                let out = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                self.effect(enc, fx, pass, cur, out, (w, h));
                self.work.release(cur);
                cur = out;
            }
        }
        Some(Processed {
            tex: cur,
            w,
            h,
            crop,
            bicubic,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        p: &Processed,
        layer: &Layer,
        scale: f32,
        below: usize,
        out: usize,
        size: (u32, u32),
    ) {
        // Layer-local uv from destination pixel: undo position, rotation and size.
        let ext_x = (layer.base_size[0] * layer.scale[0].abs() * scale).max(0.001);
        let ext_y = (layer.base_size[1] * layer.scale[1].abs() * scale).max(0.001);
        let cx = layer.center[0] * scale;
        let cy = layer.center[1] * scale;
        let th = (-layer.rotation).to_radians();
        let (s, c) = th.sin_cos();
        // local = R(-θ) (p - c) / ext + 0.5
        let m0 = [c / ext_x, -s / ext_x, (-c * cx + s * cy) / ext_x + 0.5, 0.0];
        let m1 = [s / ext_y, c / ext_y, (-s * cx - c * cy) / ext_y + 0.5, 0.0];
        let mut flags = 0;
        if layer.flip[0] != (layer.scale[0] < 0.0) {
            flags |= 1;
        }
        if layer.flip[1] != (layer.scale[1] < 0.0) {
            flags |= 2;
        }
        let params = Params {
            dst_size: [size.0 as f32, size.1 as f32],
            src_size: [p.w as f32, p.h as f32],
            flags,
            p: [
                p.crop,
                [
                    layer.opacity.clamp(0.0, 1.0),
                    layer.blend as f32,
                    if p.bicubic { 1.0 } else { 0.0 },
                    layer.corner_radius * scale,
                ],
                [ext_x, ext_y, 0.0, 0.0],
                [0.0; 4],
            ],
            m0,
            m1,
            ..Default::default()
        };
        let mut params = params;
        if let Some(m) = &layer.mask {
            params.p[3] = [
                m.shape as f32 + 1.0,
                m.center[0],
                m.center[1],
                m.rotation.to_radians(),
            ];
            params.c0 = [
                m.size[0],
                m.size[1],
                m.roundness,
                m.feather * ext_x.min(ext_y),
            ];
            params.c1 = [if m.invert { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0];
        }
        if let Some(cell) = &layer.cell {
            self.place_in_cell(enc, p, params, cell, layer, scale, below, out);
            return;
        }
        self.pass_tex(
            enc,
            Pass::Place,
            &params,
            [Some(p.tex), None, Some(below)],
            out,
        );
    }

    /// Places a layer on its own, then shows it over `below` only inside
    /// its split-screen cell, with the cell's border around it.
    #[allow(clippy::too_many_arguments)]
    fn place_in_cell(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        p: &Processed,
        mut params: Params,
        cell: &CellParams,
        layer: &Layer,
        scale: f32,
        below: usize,
        out: usize,
    ) {
        let (w, h) = (params.dst_size[0] as u32, params.dst_size[1] as u32);
        let clear = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        self.clear(enc, clear, [0.0; 4]);
        let alone = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        // Blend modes apply when the cell meets the canvas.
        params.p[1][1] = 0.0;
        self.pass_tex(
            enc,
            Pass::Place,
            &params,
            [Some(p.tex), None, Some(clear)],
            alone,
        );
        self.work.release(clear);
        let px = |q: [f32; 2]| [q[0] * scale, q[1] * scale];
        let pair = |a: [f32; 2], b: [f32; 2]| {
            let (a, b) = (px(a), px(b));
            [a[0], a[1], b[0], b[1]]
        };
        let cell_params = Params {
            dst_size: [w as f32, h as f32],
            src_size: [w as f32, h as f32],
            p: [
                pair(cell.outer[0], cell.outer[1]),
                pair(cell.outer[2], cell.outer[3]),
                [
                    cell.radius * scale,
                    layer.blend as f32,
                    layer.opacity.clamp(0.0, 1.0),
                    0.0,
                ],
                [0.0; 4],
            ],
            m0: pair(cell.inner[0], cell.inner[1]),
            m1: pair(cell.inner[2], cell.inner[3]),
            c0: cell.border,
            ..Default::default()
        };
        self.pass_tex(
            enc,
            Pass::Cell,
            &cell_params,
            [Some(alone), None, Some(below)],
            out,
        );
        self.work.release(alone);
    }

    /// Processes and places a layer on its own clear canvas.
    fn placed_alone(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        layer: &Layer,
        scale: f32,
        size: (u32, u32),
    ) -> usize {
        let (w, h) = size;
        let t = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        self.clear(enc, t, [0.0; 4]);
        let Some(p) = self.process_layer(enc, layer, scale, size) else {
            return t;
        };
        let placed = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        self.place(enc, &p, layer, scale, t, placed, size);
        self.work.release(p.tex);
        self.work.release(t);
        placed
    }

    /// Draws a layer's blurred shadow onto `canvas` and returns the new canvas.
    #[allow(clippy::too_many_arguments)]
    fn drop_shadow(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        p: &Processed,
        layer: &Layer,
        sh: &ShadowParams,
        scale: f32,
        canvas: usize,
        size: (u32, u32),
    ) -> usize {
        let (w, h) = size;
        let clear = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        self.clear(enc, clear, [0.0; 4]);
        let mut solo = layer.clone();
        solo.blend = 0;
        solo.mask = layer.mask;
        let alone = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        self.place(enc, p, &solo, scale, clear, alone, size);
        self.work.release(clear);
        let mut cur = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        let params = Params {
            dst_size: [w as f32, h as f32],
            src_size: [w as f32, h as f32],
            p: [
                [
                    sh.offset[0] * scale / w as f32,
                    sh.offset[1] * scale / h as f32,
                    0.0,
                    0.0,
                ],
                [0.0; 4],
                [0.0; 4],
                [0.0; 4],
            ],
            c0: [
                sh.color[0],
                sh.color[1],
                sh.color[2],
                sh.color[3] * layer.opacity.clamp(0.0, 1.0),
            ],
            ..Default::default()
        };
        self.pass_tex(enc, Pass::Shadow, &params, [Some(alone), None, None], cur);
        self.work.release(alone);
        let radius_px = sh.blur * scale;
        if radius_px >= 0.5 {
            // The gaussian effect takes its radius as a fraction of 6% of the height.
            let fx = Fx {
                shader: 1,
                passes: 2,
                params: [
                    [radius_px / (0.06 * h as f32), 0.0, 0.0, 0.0],
                    [0.0; 4],
                    [0.0; 4],
                    [0.0; 4],
                ],
                colors: [[0.0; 4]; 2],
                time: 0.0,
            };
            for pass in 0..2 {
                let out = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
                self.effect(enc, &fx, pass, cur, out, size);
                self.work.release(cur);
                cur = out;
            }
        }
        let next = self.work.get(&self.r.device, w, h, WORK_FORMAT, false);
        let params = Params {
            dst_size: [w as f32, h as f32],
            src_size: [w as f32, h as f32],
            ..Default::default()
        };
        self.pass_tex(
            enc,
            Pass::Over,
            &params,
            [Some(cur), Some(canvas), None],
            next,
        );
        self.work.release(cur);
        self.work.release(canvas);
        next
    }
}

fn run_pass(
    enc: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bind: &wgpu::BindGroup,
    target: &wgpu::TextureView,
    clear: Option<wgpu::Color>,
) {
    run_pass_groups(enc, pipeline, &[bind], target, clear);
}

/// `run_pass` for pipelines with more than one bind group.
fn run_pass_groups(
    enc: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    binds: &[&wgpu::BindGroup],
    target: &wgpu::TextureView,
    clear: Option<wgpu::Color>,
) {
    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: match clear {
                    Some(c) => wgpu::LoadOp::Clear(c),
                    None => wgpu::LoadOp::Load,
                },
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    for (i, bind) in binds.iter().enumerate() {
        pass.set_bind_group(i as u32, *bind, &[]);
    }
    pass.draw(0..3, 0..1);
}
