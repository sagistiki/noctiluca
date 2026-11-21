//! Puts the views on a projector: each surface's picture warped into its
//! quad (see src/mapping.rs and shaders/composite.wgsl), or the calibration
//! patterns.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Vec2};

use super::shaders;
use crate::mapping::{self, Corners};

/// At most this many surfaces on one output (8), plus the full-frame background.
const SLOTS: usize = 9;

/// What the projector shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    #[default]
    Show,
    Grid,
    White,
    Black,
}

/// One surface to draw.
#[derive(Clone, Debug)]
pub struct SurfaceDraw {
    pub corners: Corners,
    /// Its picture (ignored for the plain patterns).
    pub view: wgpu::TextureView,
    pub colour: [f32; 3],
    /// 1, 2 or 3: shown by the calibration pattern.
    pub number: u32,
    pub flip: bool,
    pub feather_px: f32,
    pub brightness: f32,
    /// Extra brightness from the chase, 0..0.2.
    pub chase: f32,
    /// Draw its outline and corners over it (the surface being mapped).
    pub outline: bool,
    /// The picture's width over height. The surface shows the middle of it
    /// in its own shape (like "cover"), so a picture is never stretched,
    /// whatever the surface's size, shape or angle. 0: stretch to fit.
    pub view_aspect: f32,
    /// The surface's real shape where it lands, if given.
    pub surface_aspect: Option<f32>,
}

impl SurfaceDraw {
    /// `view` over the whole target, nothing else.
    pub fn full(view: wgpu::TextureView, view_aspect: f32) -> Self {
        SurfaceDraw {
            corners: mapping::FULL_FRAME,
            view,
            colour: [1.0; 3],
            number: 0,
            flip: false,
            feather_px: 0.0,
            brightness: 1.0,
            chase: 0.0,
            outline: false,
            view_aspect,
            surface_aspect: None,
        }
    }
}

/// Must match `struct Surface` in shaders/composite.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SurfaceUniforms {
    inverse: [[f32; 4]; 3],
    colour: [f32; 4],
    params: [f32; 4],
    output: [f32; 4],
    extent: [f32; 4],
    crop: [f32; 4],
}

pub struct Compositor {
    pipeline: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// One per slot, so one frame's draws do not overwrite each other.
    buffers: Vec<wgpu::Buffer>,
    binds: Vec<wgpu::BindGroup>,
    /// Stands in for a view when a pattern needs none.
    blank: wgpu::TextureView,
}

impl Compositor {
    pub fn new(
        device: &wgpu::Device,
        shader_dir: &Path,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let shader = shaders::assemble_alone(shader_dir, "composite")?;
        shader
            .validate()
            .map_err(|e| anyhow!(e))
            .context("composite.wgsl")?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite"),
            source: wgpu::ShaderSource::Wgsl(shader.source.into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<SurfaceUniforms>() as u64),
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite view"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let buffers: Vec<wgpu::Buffer> = (0..SLOTS)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("composite surface"),
                    size: size_of::<SurfaceUniforms>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let binds = buffers
            .iter()
            .map(|b| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("composite surface"),
                    layout: &uniform_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: b.as_entire_binding(),
                    }],
                })
            })
            .collect();
        let blank = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("blank"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Ok(Self {
            pipeline,
            uniform_layout,
            texture_layout,
            sampler,
            buffers,
            binds,
            blank,
        })
    }

    /// A 1 x 1 stand-in texture, for surfaces with no picture.
    pub fn blank(&self) -> wgpu::TextureView {
        self.blank.clone()
    }

    /// Clears `target` to black and draws `background` (full frame, if any)
    /// and then each surface. At most three surfaces are drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        surfaces: &[SurfaceDraw],
        pattern: Pattern,
        background: Option<(&wgpu::TextureView, f32)>,
    ) {
        let _ = &self.uniform_layout;
        let mut draws: Vec<(SurfaceUniforms, wgpu::TextureView)> = Vec::new();
        if let (Some((bg, aspect)), Pattern::Show) = (background, pattern) {
            let full = SurfaceDraw::full(bg.clone(), aspect);
            if let Some(u) = uniforms(&full, size, Pattern::Show) {
                draws.push((u, bg.clone()));
            }
        }
        for s in surfaces.iter().take(SLOTS - 1) {
            if let Some(u) = uniforms(s, size, pattern) {
                let view = if pattern == Pattern::Show {
                    s.view.clone()
                } else {
                    self.blank.clone()
                };
                draws.push((u, view));
            }
        }
        for (slot, (u, _)) in draws.iter().enumerate() {
            queue.write_buffer(&self.buffers[slot], 0, bytemuck::bytes_of(u));
        }
        let texture_binds: Vec<wgpu::BindGroup> = draws
            .iter()
            .map(|(_, view)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("composite view"),
                    layout: &self.texture_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            })
            .collect();
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&self.pipeline);
        for (slot, bind) in texture_binds.iter().enumerate() {
            rp.set_bind_group(0, &self.binds[slot], &[]);
            rp.set_bind_group(1, bind, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

/// A surface's uniforms; `None` if its corners cannot be mapped.
fn uniforms(s: &SurfaceDraw, size: (u32, u32), pattern: Pattern) -> Option<SurfaceUniforms> {
    let h = mapping::homography(&s.corners)?;
    let inv: Mat3 = h.inverse();
    let px = Vec2::new(size.0 as f32, size.1 as f32);
    // The surface's size in output pixels (in its real shape, if given): the
    // grid's squares and the circle come out square and round on the wall.
    let e = mapping::extent(&s.corners, size, s.surface_aspect);
    let extent = [e.x, e.y];
    let col = |c: glam::Vec3| [c.x, c.y, c.z, 0.0];
    Some(SurfaceUniforms {
        inverse: [col(inv.x_axis), col(inv.y_axis), col(inv.z_axis)],
        colour: [s.colour[0], s.colour[1], s.colour[2], s.number as f32],
        params: [
            s.flip as u8 as f32,
            s.feather_px.max(0.0),
            s.brightness.max(0.0),
            s.chase.clamp(0.0, 0.2),
        ],
        output: [px.x, px.y, pattern as u8 as f32, 0.0],
        extent: [
            extent[0].max(1.0),
            extent[1].max(1.0),
            if s.outline { 4.0 } else { 0.0 },
            0.0,
        ],
        crop: crop(extent[0] / extent[1].max(1.0), s.view_aspect),
    })
}

/// The part of the picture a surface shows, as offset and scale in its
/// texture coordinates: the middle, in the surface's own shape.
pub fn crop(surface_aspect: f32, view_aspect: f32) -> [f32; 4] {
    if !(surface_aspect > 0.0 && view_aspect > 0.0) || !surface_aspect.is_finite() {
        return [0.0, 0.0, 1.0, 1.0];
    }
    if surface_aspect < view_aspect {
        let w = surface_aspect / view_aspect;
        [(1.0 - w) / 2.0, 0.0, w, 1.0]
    } else {
        let h = view_aspect / surface_aspect;
        [0.0, (1.0 - h) / 2.0, 1.0, h]
    }
}

/// The settings' surface colours (1 cyan, 2 magenta, 3 amber), linear.
pub fn surface_colour(index: usize) -> [f32; 3] {
    // The same eight as the editor's (src/ui/style.rs).
    let srgb = [
        [0x22, 0xe4, 0xff],
        [0xff, 0x2b, 0xd6],
        [0xff, 0xb0, 0x20],
        [0x3d, 0xff, 0x8f],
        [0x9b, 0x6d, 0xff],
        [0xff, 0x7a, 0x2f],
        [0x4d, 0x9d, 0xff],
        [0xc6, 0xff, 0x3d],
    ][index % 8];
    srgb.map(|c| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        tests_device()
    }

    pub fn tests_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
    }

    fn texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        w: u32,
        h: u32,
        pixels: &[[u8; 4]],
    ) -> wgpu::TextureView {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            t.as_image_copy(),
            bytemuck::cast_slice(pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        t.create_view(&Default::default())
    }

    /// Composites into a w x h picture and reads it back.
    fn render(
        surfaces: &[SurfaceDraw],
        pattern: Pattern,
        w: u32,
        h: u32,
        dq: &(wgpu::Device, wgpu::Queue),
    ) -> Vec<[u8; 4]> {
        let (device, queue) = dq;
        let c = Compositor::new(device, &shaders::default_dir(), FORMAT).unwrap();
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row = (4 * w).next_multiple_of(256);
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        c.draw(
            device,
            queue,
            &mut enc,
            &target.create_view(&Default::default()),
            (w, h),
            surfaces,
            pattern,
            None,
        );
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);
        buf.map_async(wgpu::MapMode::Read, .., |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = buf.get_mapped_range(..).unwrap();
        data.chunks(row as usize)
            .flat_map(|line| line[..(4 * w) as usize].as_chunks::<4>().0.to_vec())
            .collect()
    }

    fn surface(view: wgpu::TextureView, corners: Corners) -> SurfaceDraw {
        SurfaceDraw {
            corners,
            view,
            colour: [0.0, 1.0, 1.0],
            number: 1,
            flip: false,
            feather_px: 0.0,
            brightness: 1.0,
            chase: 0.0,
            outline: false,
            view_aspect: 0.0,
            surface_aspect: None,
        }
    }

    #[test]
    fn a_surface_shows_the_middle_of_the_picture_in_its_own_shape() {
        // A square surface on a 16:9 picture: the middle square, no stretch.
        let c = crop(1.0, 16.0 / 9.0);
        assert!((c[2] - 0.5625).abs() < 1e-6 && c[3] == 1.0 && (c[0] - 0.21875).abs() < 1e-6);
        // A tall surface (a pillar, a door) on a wide picture.
        let c = crop(9.0 / 16.0, 16.0 / 9.0);
        assert!((c[2] * 16.0 / 9.0 - 9.0 / 16.0).abs() < 1e-5);
        // A very wide one: the middle band.
        let c = crop(4.0, 16.0 / 9.0);
        assert!(c[2] == 1.0 && (c[3] - (16.0 / 9.0) / 4.0).abs() < 1e-6 && c[1] > 0.0);
        // Same shape, or unknown: all of it.
        assert_eq!(crop(16.0 / 9.0, 16.0 / 9.0), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(crop(1.0, 0.0), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(crop(f32::NAN, 1.0), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn identity_surface_reproduces_the_view() {
        let Some(dq) = device() else { return };
        let (w, h) = (16, 8);
        let pixels: Vec<[u8; 4]> = (0..w * h)
            .map(|i| {
                [
                    (i * 13 % 256) as u8,
                    (i * 7 % 256) as u8,
                    (i * 29 % 256) as u8,
                    255,
                ]
            })
            .collect();
        let view = texture(&dq.0, &dq.1, w, h, &pixels);
        let out = render(
            &[surface(view, mapping::FULL_FRAME)],
            Pattern::Show,
            w,
            h,
            &dq,
        );
        let diff: u32 = out
            .iter()
            .zip(&pixels)
            .map(|(a, b)| {
                (0..3)
                    .map(|k| (a[k] as i32 - b[k] as i32).unsigned_abs())
                    .sum::<u32>()
            })
            .sum();
        assert!(
            diff as f32 / (w * h * 3) as f32 <= 1.0,
            "mean difference {}",
            diff as f32 / (w * h * 3) as f32
        );
    }

    #[test]
    fn keystoned_surface_samples_the_right_texel() {
        let Some(dq) = device() else { return };
        // A 2 x 2 view: red, green / blue, white.
        let view = texture(
            &dq.0,
            &dq.1,
            2,
            2,
            &[
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 255, 255],
            ],
        );
        let corners: Corners = [[0.2, 0.1], [0.9, 0.2], [0.8, 0.9], [0.1, 0.8]];
        let (w, h) = (200, 200);
        let out = render(&[surface(view, corners)], Pattern::Show, w, h, &dq);
        let hmat = mapping::homography(&corners).unwrap();
        let at = |u: f32, v: f32| {
            let p = mapping::to_output(&hmat, Vec2::new(u, v));
            out[(p.y * h as f32) as usize * w as usize + (p.x * w as f32) as usize]
        };
        let near = |a: [u8; 4], b: [u8; 3]| (0..3).all(|k| (a[k] as i32 - b[k] as i32).abs() < 40);
        assert!(near(at(0.2, 0.2), [255, 0, 0]), "{:?}", at(0.2, 0.2));
        assert!(near(at(0.8, 0.2), [0, 255, 0]), "{:?}", at(0.8, 0.2));
        assert!(near(at(0.2, 0.8), [0, 0, 255]), "{:?}", at(0.2, 0.8));
        assert!(near(at(0.8, 0.8), [255, 255, 255]), "{:?}", at(0.8, 0.8));
        // Outside the quad: black.
        assert_eq!(out[2 * w as usize + 2][..3], [0, 0, 0]);
    }

    #[test]
    fn grid_pattern_never_shows_the_show() {
        let Some(dq) = device() else { return };
        // A view of pure red: under the grid pattern no pixel may be red.
        let red = texture(&dq.0, &dq.1, 4, 4, &[[255, 0, 0, 255]; 16]);
        let out = render(
            &[surface(red, mapping::FULL_FRAME)],
            Pattern::Grid,
            160,
            90,
            &dq,
        );
        assert!(
            out.iter().all(|p| !(p[0] > 200 && p[1] < 60 && p[2] < 60)),
            "the show is under the grid"
        );
        assert!(
            out.iter().any(|p| p[1] > 200 && p[2] > 200),
            "no grid lines in the surface's colour"
        );
        assert!(
            out.iter().any(|p| p[0] < 10 && p[1] < 10 && p[2] < 10),
            "no black between the lines"
        );
    }

    #[test]
    fn flip_and_feather_and_brightness() {
        let Some(dq) = device() else { return };
        let view = texture(&dq.0, &dq.1, 2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
        let mut s = surface(view, mapping::FULL_FRAME);
        s.flip = true;
        s.feather_px = 10.0;
        s.brightness = 0.5;
        let (w, h) = (100, 50);
        let out = render(&[s], Pattern::Show, w, h, &dq);
        let px = |x: usize, y: usize| out[y * w as usize + x];
        // Flipped: blue on the left.
        assert!(px(25, 25)[2] > px(25, 25)[0]);
        // Half brightness: no channel near full.
        assert!(
            px(25, 25)[2] < 160 && px(25, 25)[2] > 60,
            "{:?}",
            px(25, 25)
        );
        // The edge fades to black.
        assert!(px(0, 25)[2] < 15, "{:?}", px(0, 25));
    }
}
