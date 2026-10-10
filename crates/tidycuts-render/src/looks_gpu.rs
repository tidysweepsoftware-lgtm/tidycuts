//! The GPU side of the Color tab's looks: the grade pass's second bind
//! group (the tone table and the 3D LUT) and a cache so each table and LUT
//! is uploaded once.

use crate::looks::{Looks, TONE_SIZE};
use crate::lut::Lut3d;
use wgpu::util::DeviceExt;

/// `Params::flags` bits the grade pass reads.
const FLAG_CURVES: u32 = 1;
const FLAG_HSL: u32 = 2;
const FLAG_LUT: u32 = 4;

/// Frames a texture may go unused before it's dropped.
const KEEP_TONE: u64 = 120;
const KEEP_LUT: u64 = 600;

/// The grade pass's extra bind group layout and its stand-in textures.
pub(crate) struct GradeLayouts {
    layout: wgpu::BindGroupLayout,
    /// Group 0 (shared by every pass) plus the looks group.
    pub pipeline_layout: wgpu::PipelineLayout,
    /// Bound when a grade has no looks.
    none: wgpu::BindGroup,
    dummy_tone: wgpu::TextureView,
    dummy_lut: wgpu::TextureView,
}

impl GradeLayouts {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        base: &wgpu::BindGroupLayout,
    ) -> GradeLayouts {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tidycuts looks"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tidycuts grade"),
            bind_group_layouts: &[Some(base), Some(&layout)],
            immediate_size: 0,
        });
        let dummy_tone =
            tone_texture(device, queue, &[0.0; TONE_SIZE * 2 * 4]).create_view(&Default::default());
        let dummy_lut =
            lut_texture(device, queue, &Lut3d::from_fn(2, |c| c)).create_view(&Default::default());
        let none = bind(device, &layout, &dummy_tone, &dummy_lut);
        GradeLayouts {
            layout,
            pipeline_layout,
            none,
            dummy_tone,
            dummy_lut,
        }
    }
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    tone: &wgpu::TextureView,
    lut: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tidycuts looks"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(tone),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(lut),
            },
        ],
    })
}

fn tone_desc() -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("tidycuts tone table"),
        size: wgpu::Extent3d {
            width: TONE_SIZE as u32,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    }
}

fn tone_texture(device: &wgpu::Device, queue: &wgpu::Queue, data: &[f32]) -> wgpu::Texture {
    device.create_texture_with_data(
        queue,
        &tone_desc(),
        wgpu::util::TextureDataOrder::LayerMajor,
        bytemuck::cast_slice(data),
    )
}

fn lut_texture(device: &wgpu::Device, queue: &wgpu::Queue, lut: &Lut3d) -> wgpu::Texture {
    let n = lut.size as u32;
    device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("tidycuts lut"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        bytemuck::cast_slice(&lut.to_rgba_f16()),
    )
}

struct Cached {
    key: u64,
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    last_used: u64,
}

/// Tone tables and LUTs on the GPU, one cache per compositor.
#[derive(Default)]
pub(crate) struct LookCache {
    tones: Vec<Cached>,
    luts: Vec<Cached>,
}

impl LookCache {
    /// The looks bind group for a grade pass, the flags telling the shader
    /// which looks to apply, and the LUT's strength.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layouts: &GradeLayouts,
        looks: Option<&Looks>,
        frame: u64,
    ) -> (wgpu::BindGroup, u32, f32) {
        let Some(looks) = looks else {
            return (layouts.none.clone(), 0, 0.0);
        };
        let mut flags = 0;
        if looks.curves {
            flags |= FLAG_CURVES;
        }
        if looks.hsl {
            flags |= FLAG_HSL;
        }
        let tone = match &looks.tone {
            Some(t) => self.tone(device, queue, t.key, &t.data, frame),
            None => layouts.dummy_tone.clone(),
        };
        let lut = match &looks.lut {
            Some(l) => {
                flags |= FLAG_LUT;
                self.lut(device, queue, l, frame)
            }
            None => layouts.dummy_lut.clone(),
        };
        (
            bind(device, &layouts.layout, &tone, &lut),
            flags,
            looks.lut_strength,
        )
    }

    fn tone(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: u64,
        data: &[f32],
        frame: u64,
    ) -> wgpu::TextureView {
        if let Some(c) = self.tones.iter_mut().find(|c| c.key == key) {
            c.last_used = frame;
            return c.view.clone();
        }
        // Animated HSL or curves make a new table every frame: rewrite one
        // that wasn't used in this frame or the last instead of allocating.
        if let Some(c) = self
            .tones
            .iter_mut()
            .filter(|c| c.last_used + 1 < frame)
            .min_by_key(|c| c.last_used)
        {
            queue.write_texture(
                c.tex.as_image_copy(),
                bytemuck::cast_slice(data),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(TONE_SIZE as u32 * 16),
                    rows_per_image: None,
                },
                tone_desc().size,
            );
            c.key = key;
            c.last_used = frame;
            return c.view.clone();
        }
        let tex = tone_texture(device, queue, data);
        let view = tex.create_view(&Default::default());
        self.tones.push(Cached {
            key,
            tex,
            view: view.clone(),
            last_used: frame,
        });
        view
    }

    fn lut(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lut: &Lut3d,
        frame: u64,
    ) -> wgpu::TextureView {
        if let Some(c) = self.luts.iter_mut().find(|c| c.key == lut.key) {
            c.last_used = frame;
            return c.view.clone();
        }
        let tex = lut_texture(device, queue, lut);
        let view = tex.create_view(&Default::default());
        self.luts.push(Cached {
            key: lut.key,
            tex,
            view: view.clone(),
            last_used: frame,
        });
        view
    }

    /// Drops textures that haven't been used for a while.
    pub fn end_frame(&mut self, frame: u64) {
        self.tones
            .retain(|c| frame.saturating_sub(c.last_used) < KEEP_TONE);
        self.luts
            .retain(|c| frame.saturating_sub(c.last_used) < KEEP_LUT);
    }
}
