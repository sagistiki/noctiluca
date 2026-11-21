//! Draws what the projector shows: the pictures its surfaces need (Main,
//! the shared Complement, Main one beat late), each warped into its quad.

use std::path::Path;

use anyhow::Result;

use super::Renderer;
use super::compositor::{Compositor, Pattern, SurfaceDraw, surface_colour};
use super::uniforms::Uniforms;
use crate::director::DirectorOut;
use crate::mapping::{self, FULL_FRAME};
use crate::stage::{self, Planned, Source};

/// The renderer's view numbers.
pub const MAIN_VIEW: usize = 0;
pub const COMPLEMENT_VIEW: usize = 1;
/// Echo frames are kept no taller than this (memory: ~110 MB at 40 frames).
const ECHO_HEIGHT: u32 = 720;
const ECHO_SLOTS: usize = 40;

/// The pictures drawn this frame.
#[derive(Clone)]
pub struct Views {
    pub main: wgpu::TextureView,
    pub complement: Option<wgpu::TextureView>,
    /// Each picture's width over height.
    pub main_aspect: f32,
    pub complement_aspect: f32,
}

/// Which pictures: the ones for now (displays and projectors), or the ones
/// drawn ahead for network screens, which show them later. Each set keeps
/// its own trails and feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewSet {
    Now,
    Ahead,
}

/// The network screens' pictures.
pub const AHEAD_MAIN_VIEW: usize = 2;
pub const AHEAD_COMPLEMENT_VIEW: usize = 3;

impl ViewSet {
    pub fn main(self) -> usize {
        match self {
            ViewSet::Now => MAIN_VIEW,
            ViewSet::Ahead => AHEAD_MAIN_VIEW,
        }
    }
    pub fn complement(self) -> usize {
        match self {
            ViewSet::Now => COMPLEMENT_VIEW,
            ViewSet::Ahead => AHEAD_COMPLEMENT_VIEW,
        }
    }
}

fn aspect(size: (u32, u32)) -> f32 {
    size.0 as f32 / size.1.max(1) as f32
}

pub struct Projector {
    /// One compositor per target drawn in a frame: their buffers must not be shared.
    compositor: Compositor,
    echo_blit: Compositor,
    format: wgpu::TextureFormat,
    ring: Vec<(wgpu::Texture, wgpu::TextureView)>,
    ring_size: (u32, u32),
    head: usize,
}

impl Projector {
    pub fn new(
        device: &wgpu::Device,
        shader_dir: &Path,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        Ok(Self {
            compositor: Compositor::new(device, shader_dir, format)?,
            echo_blit: Compositor::new(device, shader_dir, format)?,
            format,
            ring: Vec::new(),
            ring_size: (0, 0),
            head: 0,
        })
    }

    /// Draws the pictures the surfaces of every output need, once for all of
    /// them: each big enough for every surface showing it, in its own shape
    /// (no taller than `max_height`). `planned`: each output's plan with its
    /// size in pixels. `backgrounds`: the sizes of the outputs that show Main
    /// behind their surfaces too.
#[allow(clippy::too_many_arguments)]
        pub fn render_views(
        r: &mut Renderer,
        encoder: &mut wgpu::CommandEncoder,
        set: ViewSet,
        planned: &[(&[Planned], (u32, u32))],
        backgrounds: &[(u32, u32)],
        max_height: u32,
        u: &Uniforms,
        show: &DirectorOut,
    ) -> Views {
        let extents = |src: &[Source]| {
            planned
                .iter()
                .flat_map(|(plan, px)| {
                    plan.iter()
                        .filter(|p| src.contains(&p.source))
                        .map(|p| mapping::extent(&p.corners, *px, p.aspect))
                })
                .collect::<Vec<_>>()
        };
        let fallback = planned
            .first()
            .map(|(_, px)| *px)
            .or(backgrounds.first().copied())
            .unwrap_or((1920, 1080));
        let mut main_extents = extents(&[Source::Main, Source::Echo]);
        main_extents.extend(
            backgrounds
                .iter()
                .map(|px| mapping::extent(&FULL_FRAME, *px, None)),
        );
        let main_size = stage::view_size(main_extents, fallback, max_height);
        let main = r.render_view(encoder, set.main(), main_size, u, show);
        let comp_extents = extents(&[Source::Complement]);
        let mut complement_aspect = 0.0;
        let complement = (!comp_extents.is_empty()).then(|| {
            let size = stage::view_size(comp_extents, fallback, max_height);
            complement_aspect = aspect(size);
            let show = DirectorOut {
                from: show.complement_from,
                to: show.complement_to,
                ..*show
            };
            r.render_view(encoder, set.complement(), size, u, &show)
        });
        Views {
            main,
            complement,
            main_aspect: aspect(main_size),
            complement_aspect,
        }
    }

    /// Keeps Main's frames for the Echo surfaces, and returns the one
    /// `frames_back` frames old.
    fn echo(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        main: &wgpu::TextureView,
        main_size: (u32, u32),
        frames_back: usize,
    ) -> wgpu::TextureView {
        let h = main_size.1.clamp(1, ECHO_HEIGHT);
        let size = (
            ((main_size.0 as f32) * h as f32 / main_size.1.max(1) as f32)
                .round()
                .max(1.0) as u32,
            h,
        );
        if self.ring_size != size || self.ring.is_empty() {
            self.ring = (0..ECHO_SLOTS)
                .map(|_| {
                    let t = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("echo"),
                        size: wgpu::Extent3d {
                            width: size.0,
                            height: size.1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    });
                    let v = t.create_view(&Default::default());
                    (t, v)
                })
                .collect();
            self.ring_size = size;
            self.head = 0;
        }
        self.head = (self.head + 1) % ECHO_SLOTS;
        let full = SurfaceDraw::full(main.clone(), 0.0);
        self.echo_blit.draw(
            device,
            queue,
            encoder,
            &self.ring[self.head].1,
            size,
            &[full],
            Pattern::Show,
            None,
        );
        let back = frames_back.clamp(1, ECHO_SLOTS - 1);
        self.ring[(self.head + ECHO_SLOTS - back) % ECHO_SLOTS]
            .1
            .clone()
    }

    /// Composites the surfaces into `target` (the projector's picture).
    #[allow(clippy::too_many_arguments)]
    pub fn compose(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        planned: &[Planned],
        views: Option<&Views>,
        main_size: (u32, u32),
        pattern: Pattern,
        black_outside: bool,
        echo_frames: usize,
        outlined: Option<usize>,
    ) {
        let echo = match views {
            Some(v)
                if pattern == Pattern::Show && planned.iter().any(|p| p.source == Source::Echo) =>
            {
                Some(self.echo(device, queue, encoder, &v.main, main_size, echo_frames))
            }
            _ => {
                // Without Echo surfaces the frames are not kept.
                self.ring.clear();
                self.ring_size = (0, 0);
                None
            }
        };
        let draws: Vec<SurfaceDraw> = planned
            .iter()
            .map(|p| {
                let (view, view_aspect) = match (views, p.source) {
                    (None, _) => (self.compositor_blank(), 0.0),
                    (Some(v), Source::Main) => (v.main.clone(), v.main_aspect),
                    (Some(v), Source::Complement) => match &v.complement {
                        Some(c) => (c.clone(), v.complement_aspect),
                        None => (v.main.clone(), v.main_aspect),
                    },
                    // The ring keeps Main's shape.
                    (Some(v), Source::Echo) => (
                        echo.clone().unwrap_or_else(|| v.main.clone()),
                        v.main_aspect,
                    ),
                };
                SurfaceDraw {
                    corners: p.corners,
                    view,
                    colour: surface_colour(p.index),
                    number: p.index as u32 + 1,
                    flip: p.flip,
                    feather_px: p.feather_px,
                    brightness: p.brightness,
                    chase: p.chase,
                    outline: outlined == Some(p.index),
                    view_aspect,
                    surface_aspect: p.aspect,
                }
            })
            .collect();
        let background = views
            .filter(|_| !black_outside)
            .map(|v| (&v.main, v.main_aspect));
        let pattern = if views.is_none() && pattern == Pattern::Show {
            Pattern::Black
        } else {
            pattern
        };
        self.compositor.draw(
            device, queue, encoder, target, size, &draws, pattern, background,
        );
    }

    fn compositor_blank(&self) -> wgpu::TextureView {
        self.compositor.blank()
    }
}
