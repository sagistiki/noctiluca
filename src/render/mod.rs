//! Everything that talks to the GPU.
//!
//! A frame is drawn in passes. Each pass is one full-screen triangle with a
//! fragment shader that decides the colour of every pixel:
//!
//!   scene   → `scene`     one background + one pattern, in HDR
//!   objects → `scene`     the 3D object, drawn over it (reading a copy, `behind`)
//!   post    → `feedback`  scene mixed with the previous frame: trails, zoom, kaleidoscope
//!   glow    → `glow`      bright parts only, blurred, at quarter size
//!   present → the screen  feedback + glow, strobe, tone mapping, HUD
//!
//! `feedback` is two textures used in turn: this frame is written to one
//! while the previous frame is read from the other (a GPU cannot read and
//! write the same texture in one pass).

pub mod compositor;
pub mod model;
pub mod objects;
pub mod projector;
pub mod readback;
pub mod resources;
pub mod shaders;
pub mod snapshot;
pub mod uniforms;
pub mod window_target;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use winit::window::Window;

use wgpu::util::DeviceExt;

use crate::director::{ATMOSPHERES, BACKGROUNDS, DirectorOut, Look, OBJECTS, PATTERNS};
use objects::ObjectUniforms;
use shaders::ShaderWatcher;
use uniforms::Uniforms;

/// Half-float colour: lets the picture be brighter than white, which is what
/// makes glow and trails look right.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const GLOW_DOWNSCALE: u32 = 4;
/// Atmospheres are soft (clouds, smoke, fire), so half resolution is plenty.
const ATMOS_DOWNSCALE: u32 = 2;
/// new * constant + old * (1 - constant): a cross-fade whose amount is set per draw.
const CROSS_FADE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Constant,
        dst_factor: wgpu::BlendFactor::OneMinusConstant,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Constant,
        dst_factor: wgpu::BlendFactor::OneMinusConstant,
        operation: wgpu::BlendOperation::Add,
    },
};
/// The scene: the outgoing look drawn solid, the incoming one over it by the
/// alpha its shader gives (the fade, or an ink blot or opening circle for
/// other transitions). The picture stays opaque.
const SCENE_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
};
/// The scene pass is the expensive one (ray marching, layered noise), and
/// what it draws is soft: it is blurred, echoed and glowed afterwards. So it
/// is drawn at no more than 1080 lines and scaled up by the effects pass,
/// which keeps a 3024x1964 laptop screen above 60 fps.
const MAX_SCENE_HEIGHT: u32 = 1080;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Objects are drawn with four samples per pixel and averaged down, so
/// their edges are smooth instead of stair-stepped.
const OBJECT_SAMPLES: u32 = 4;
/// Objects are drawn as compositions of many copies (a wall of eyes, a rain
/// of prisms); two objects can be
/// on screen while one fades into the next.
const MAX_OBJECT_DRAWS: usize = 64;

/// An off-screen picture: `view` to draw into it, `bind` to read it in a later pass.
struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

/// A model on the GPU.
struct GpuModel {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

/// Every off-screen picture, plus the bind groups for the passes that read two at once.
struct Targets {
    scene: Target,
    /// A copy of the scene before any object is drawn: what glass sees through.
    behind: Target,
    /// Which object surface is nearest, per pixel, so objects hide their own back.
    depth: wgpu::TextureView,
    /// Where objects are drawn at four samples per pixel before being
    /// averaged into `scene`.
    multisampled: wgpu::TextureView,
    scene_size: (u32, u32),
    /// The atmosphere, at half the scene's size.
    atmos: Target,
    feedback: [Target; 2],
    glow_a: Target,
    glow_b: Target,
    /// `post_bind[i]` reads the scene, feedback[1 - i] and the atmosphere; used when writing feedback[i].
    post_bind: [wgpu::BindGroup; 2],
    /// `present_bind[i]` reads feedback[i] and the finished glow.
    present_bind: [wgpu::BindGroup; 2],
}

struct Pipelines {
    /// Every background and pattern in one program; a draw picks the pair
    /// by its instance number (see `scene_instance`).
    scene: wgpu::RenderPipeline,
    post: wgpu::RenderPipeline,
    glow_across: wgpu::RenderPipeline,
    glow_down: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    object: wgpu::RenderPipeline,
    /// Copies `behind` into the multisampled picture before objects go on it.
    object_backdrop: wgpu::RenderPipeline,
    /// One per entry in ATMOSPHERES; `None` for "none".
    atmos: Vec<Option<wgpu::RenderPipeline>>,
}

/// What a pass reads besides the uniforms.
#[derive(Clone, Copy)]
enum Reads {
    Nothing,
    OneTexture,
    TwoTextures,
    ThreeTextures,
}

/// The parts needed to build pipelines and targets, kept together so the
/// builder functions take one argument instead of six.
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    uniform_layout: wgpu::BindGroupLayout,
    one_texture_layout: wgpu::BindGroupLayout,
    two_texture_layout: wgpu::BindGroupLayout,
    three_texture_layout: wgpu::BindGroupLayout,
    object_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

pub struct Renderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipelines: Pipelines,
    /// One per entry in OBJECTS; `None` for "none" and for files that failed to load.
    models: Vec<Option<GpuModel>>,
    /// The pictures of the show: view 0 is the main one (the window, the
    /// menu's preview, snapshots); more for other surfaces of a projector.
    views: Vec<View>,
    shader_dir: PathBuf,
    watcher: ShaderWatcher,
    /// Kept to make surfaces for more windows (a projector).
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
}

/// One complete picture of the show, with everything that is its own: its
/// targets and echoes, and the uniform buffers it is drawn with. Each view
/// has its own buffers because all of a frame's buffer writes reach the GPU
/// before any of its drawing does.
struct View {
    /// Size of the off-screen targets.
    size: (u32, u32),
    targets: Targets,
    /// Which of the two feedback textures this frame writes to.
    flip: usize,
    uniform_buf: wgpu::Buffer,
    uniform_bind: wgpu::BindGroup,
    /// One uniform buffer per object drawn in a frame.
    object_bufs: Vec<wgpu::Buffer>,
    object_binds: Vec<wgpu::BindGroup>,
    /// The uniforms of the frame being drawn; objects move with them.
    current: Uniforms,
    /// The finished picture, when the view is drawn off-screen.
    output: Option<(wgpu::Texture, wgpu::TextureView)>,
}

impl View {
    fn new(gpu: &Gpu, size: (u32, u32)) -> Self {
        let uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms"),
            layout: &gpu.uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });
        let object_bufs: Vec<wgpu::Buffer> = (0..MAX_OBJECT_DRAWS)
            .map(|_| {
                gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("object"),
                    size: size_of::<ObjectUniforms>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let object_binds = object_bufs
            .iter()
            .map(|buf| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("object"),
                    layout: &gpu.object_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    }],
                })
            })
            .collect();
        Self {
            size,
            targets: gpu.targets(size.0, size.1),
            flip: 0,
            uniform_buf,
            uniform_bind,
            object_bufs,
            object_binds,
            current: bytemuck::Zeroable::zeroed(),
            output: None,
        }
    }

    fn resize(&mut self, gpu: &Gpu, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 || size == self.size {
            return;
        }
        self.size = size;
        self.targets = gpu.targets(size.0, size.1);
        self.output = None;
    }

    /// The texture the view is drawn into off-screen, made at its size.
    fn output(&mut self, gpu: &Gpu, format: wgpu::TextureFormat) -> wgpu::TextureView {
        let size = self.size;
        self.output
            .get_or_insert_with(|| {
                let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("view"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                (texture, view)
            })
            .1
            .clone()
    }
}

impl Renderer {
    pub fn new(window: Arc<Window>, shader_dir: PathBuf, model_dir: &Path) -> Result<Self> {
        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window)?;
        // Setting up the GPU is asynchronous in wgpu; `block_on` simply waits.
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .context("no compatible GPU found")?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| anyhow!("the window surface is not supported by this GPU"))?;
        let formats = surface.get_capabilities(&adapter).formats;
        config.format = formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(formats[0]);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let gpu = Gpu::new(device, queue);
        let models = OBJECTS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                if i == 0 {
                    return None;
                }
                match model::load(&model_dir.join(format!("{name}.glb"))) {
                    Ok(m) => Some(gpu.upload(name, &m)),
                    Err(e) => {
                        eprintln!("no {name}: {e:#}");
                        None
                    }
                }
            })
            .collect();
        let views = vec![View::new(&gpu, (width, height))];
        // At start-up a broken shader is fatal: there is nothing older to fall back on.
        let pipelines = gpu
            .pipelines(&shader_dir, config.format)
            .map_err(|e| anyhow!(e))?;
        let watcher = ShaderWatcher::new(&shader_dir);

        Ok(Self {
            gpu,
            surface,
            config,
            pipelines,
            models,
            views,
            shader_dir,
            watcher,
            instance,
            adapter,
        })
    }

    /// Call when the window changes size. A minimised window reports zero,
    /// which the GPU would reject, so that is ignored.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.gpu.device, &self.config);
        self.set_render_size(width, height);
    }

    /// Changes the size of the main view's off-screen targets only.
    pub fn set_render_size(&mut self, width: u32, height: u32) {
        let gpu = &self.gpu;
        self.views[0].resize(gpu, (width, height));
    }

    /// The main view's size.
    pub fn size(&self) -> (u32, u32) {
        self.views[0].size
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.gpu.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.gpu.queue
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Waits for the window's screen refresh before showing a frame (on), or
    /// shows it at once (off: used while a projector does the waiting).
    pub fn set_vsync(&mut self, on: bool) {
        let mode = if on {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.gpu.device, &self.config);
        }
    }

    /// The window's size in pixels.
    pub fn surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// The window's next picture to draw into; `None` when there is none
    /// this frame (hidden, or the window just changed and was reconfigured).
    pub fn acquire(&mut self) -> Option<wgpu::SurfaceTexture> {
        use wgpu::CurrentSurfaceTexture as Acquired;
        match self.surface.get_current_texture() {
            Acquired::Success(frame) | Acquired::Suboptimal(frame) => Some(frame),
            Acquired::Outdated | Acquired::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                None
            }
            _ => None,
        }
    }

    pub fn submit(&self, encoder: wgpu::CommandEncoder, frame: wgpu::SurfaceTexture) {
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.queue.present(frame);
    }

    /// Draws the show into the main view's own texture at `size` and
    /// returns it: the menu's preview card.
    pub fn render_preview(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        u: &Uniforms,
        show: &DirectorOut,
        size: (u32, u32),
    ) -> wgpu::TextureView {
        self.render_view(encoder, 0, size, u, show)
    }

    /// Draws view `id` (made if new) at `size` into its own texture, and
    /// returns that texture. Views keep their echoes from frame to frame.
    pub fn render_view(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        id: usize,
        size: (u32, u32),
        u: &Uniforms,
        show: &DirectorOut,
    ) -> wgpu::TextureView {
        while self.views.len() <= id {
            let view = View::new(&self.gpu, size);
            self.views.push(view);
        }
        let format = self.config.format;
        let gpu = &self.gpu;
        self.views[id].resize(gpu, size);
        let target = self.views[id].output(gpu, format);
        // The shaders take the picture's shape from here.
        let mut u = *u;
        u.res_time[0] = size.0 as f32;
        u.res_time[1] = size.1 as f32;
        self.prepare(id, &u);
        self.draw(id, encoder, show, Some(&target));
        target
    }

    /// A target for drawing into another window (the projector's).
    pub fn window_target(&self, window: Arc<Window>) -> Result<window_target::WindowTarget> {
        window_target::WindowTarget::new(
            &self.instance,
            &self.adapter,
            &self.gpu.device,
            window,
            self.config.format,
        )
    }

    /// Where the shaders are read from.
    pub fn shader_dir(&self) -> &Path {
        &self.shader_dir
    }

    /// The size view `id` was last drawn at.
    pub fn view_size(&self, id: usize) -> Option<(u32, u32)> {
        self.views.get(id).map(|v| v.size)
    }

    /// The finished texture of view `id`, if it has been drawn off-screen.
    pub fn view_texture(&self, id: usize) -> Option<wgpu::TextureView> {
        self.views.get(id)?.output.as_ref().map(|(_, v)| v.clone())
    }

    /// Draws one frame to the window.
    pub fn frame(&mut self, u: &Uniforms, show: &DirectorOut) -> Result<()> {
        // Back from the menu's small preview to the window's own size.
        self.set_render_size(self.config.width, self.config.height);
        self.prepare(0, u);
        use wgpu::CurrentSurfaceTexture as Acquired;
        let frame = match self.surface.get_current_texture() {
            Acquired::Success(frame) | Acquired::Suboptimal(frame) => frame,
            Acquired::Outdated | Acquired::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(());
            }
            // Timed out or hidden behind another window: skip this frame.
            _ => return Ok(()),
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.draw(0, &mut encoder, show, Some(&view));
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.queue.present(frame);
        Ok(())
    }

    /// Advances the picture by one frame without showing it. Trails and
    /// echoes depend on earlier frames, so snapshot mode has to draw them all.
    pub fn frame_offscreen(&mut self, u: &Uniforms, show: &DirectorOut) {
        self.prepare(0, u);
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.draw(0, &mut encoder, show, None);
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// Blocks until the GPU has finished everything submitted so far. Only
    /// used for measuring how long a frame really takes.
    pub fn wait_idle(&self) {
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Draws one frame off-screen and saves it as a PNG.
    pub fn snapshot(&mut self, u: &Uniforms, show: &DirectorOut, path: &Path) -> Result<()> {
        self.prepare(0, u);
        let size = self.views[0].size;
        self.capture(size, path, |r, encoder, view| {
            r.draw(0, encoder, show, Some(view))
        })
    }

    /// Lets `draw` paint a picture of `size` (in the window's format) and
    /// saves it as a PNG. For pictures of the show and of the menu's pages.
    pub fn capture(
        &mut self,
        size: (u32, u32),
        path: &Path,
        draw: impl FnOnce(&mut Self, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) -> Result<()> {
        let (width, height) = size;
        let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());

        // The GPU wants each row of a copied image padded to a multiple of 256 bytes.
        let row = width * 4;
        let padded_row = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: (padded_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        draw(self, &mut encoder, &view);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit([encoder.finish()]);

        buffer.map_async(wgpu::MapMode::Read, .., |_| {});
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        let data = buffer.get_mapped_range(..)?;
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut rgba = Vec::with_capacity((row * height) as usize);
        for line in data.chunks(padded_row as usize) {
            for px in line[..row as usize].as_chunks::<4>().0 {
                if bgra {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        snapshot::write_png(path, width, height, &rgba)
    }

    fn prepare(&mut self, id: usize, u: &Uniforms) {
        if self.watcher.take_dirty() {
            self.reload_shaders();
        }
        let view = &mut self.views[id];
        self.gpu
            .queue
            .write_buffer(&view.uniform_buf, 0, bytemuck::bytes_of(u));
        view.current = *u;
    }

    /// The objects to draw this frame: (index into OBJECTS, material, how far
    /// grown in). During a cross-fade the old one shrinks as the new one grows.
    fn objects_on_screen(show: &DirectorOut) -> Vec<(usize, usize, f32)> {
        let (from, to) = (show.from.wrapped(), show.to.wrapped());
        let mut out = Vec::new();
        if from.object == to.object {
            let material = if show.mix < 0.5 {
                from.material
            } else {
                to.material
            };
            out.push((to.object, material, 1.0));
        } else {
            out.push((from.object, from.material, 1.0 - show.mix));
            out.push((to.object, to.material, show.mix));
        }
        out.retain(|(object, _, appear)| *object != 0 && *appear > 0.01);
        out
    }

    /// Rebuilds the pipelines from disk. If anything is wrong with the new
    /// shaders the old pipelines stay in place.
    fn reload_shaders(&mut self) {
        match self.gpu.pipelines(&self.shader_dir, self.config.format) {
            Ok(pipelines) => {
                self.pipelines = pipelines;
                println!("shaders reloaded");
            }
            Err(e) => eprintln!("shader error, keeping the previous version:\n{e}"),
        }
    }

    fn draw(
        &mut self,
        id: usize,
        encoder: &mut wgpu::CommandEncoder,
        show: &DirectorOut,
        screen: Option<&wgpu::TextureView>,
    ) {
        let (gpu, p, models) = (&self.gpu, &self.pipelines, &self.models);
        let v = &mut self.views[id];
        let (t, uniforms) = (&v.targets, &v.uniform_bind);
        let now = v.flip;
        // The draw's instance number: which look, plus a flag for the
        // outgoing look while another comes in (LEAVING) or the incoming one
        // (ARRIVING); see scene.wgsl.
        let scene = |look: Look, flag: u32| {
            let look = look.wrapped();
            let k = (look.bg * shaders::SCENE_SLOTS + look.pattern) as u32 + flag;
            k..k + 1
        };

        // The scene: the outgoing look, with the incoming one faded in over it.
        // The scene pipelines blend by a constant we set per draw, so the same
        // pipeline can draw solid (1) or partly transparent (`mix`).
        {
            let mut rp = begin_pass(encoder, &t.scene.view);
            rp.set_bind_group(0, uniforms, &[]);
            rp.set_pipeline(&p.scene);
            let same_scene = (show.from.bg, show.from.pattern) == (show.to.bg, show.to.pattern);
            let arriving = !same_scene && show.mix > 0.0;
            rp.draw(0..3, scene(show.from, if arriving { shaders::SCENE_LEAVING } else { 0 }));
            if arriving {
                rp.draw(0..3, scene(show.to, shaders::SCENE_ARRIVING));
            }
        }
        let on_screen = Self::objects_on_screen(show);
        // One entry per copy to draw: (model, where, material).
        let drawable: Vec<_> = on_screen
            .iter()
            .filter_map(|(object, material, appear)| {
                models[*object]
                    .as_ref()
                    .map(|m| (m, *object, *material, *appear))
            })
            .flat_map(|(model, object, material, appear)| {
                objects::placements(object, &v.current, appear)
                    .into_iter()
                    .map(move |at| {
                        // The moon is stone, whatever the look's material.
                        let material = if OBJECTS.get(object) == Some(&"moon") { 2 } else { material };
                        (model, at, material)
                    })
            })
            .take(MAX_OBJECT_DRAWS)
            .collect();
        if !drawable.is_empty() {
            let (w, h) = t.scene_size;
            let size = wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            };
            encoder.copy_texture_to_texture(
                t.scene.texture.as_image_copy(),
                t.behind.texture.as_image_copy(),
                size,
            );
            let camera = objects::Camera::new(&v.current);
            let view_proj = camera.view_proj(w as f32 / h as f32);
            for (slot, (_, placed, material)) in drawable.iter().enumerate() {
                let data = ObjectUniforms {
                    model: placed.at.to_cols_array_2d(),
                    view_proj: view_proj.to_cols_array_2d(),
                    eye: camera.eye.extend(1.0).into(),
                    params: [*material as f32, placed.body, placed.glow, placed.sparkle],
                };
                gpu.queue
                    .write_buffer(&v.object_bufs[slot], 0, bytemuck::bytes_of(&data));
            }
            // Drawn into the multisampled picture and averaged back into `scene`.
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("objects"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.multisampled,
                    depth_slice: None,
                    resolve_target: Some(&t.scene.view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(0, uniforms, &[]);
            rp.set_bind_group(1, &t.behind.bind, &[]);
            // The backdrop shares the object layout, so it needs group 2 set
            // too, though it reads nothing from it.
            rp.set_bind_group(2, &v.object_binds[0], &[]);
            rp.set_pipeline(&p.object_backdrop); // the scene first, as the backdrop
            rp.draw(0..3, 0..1);
            rp.set_pipeline(&p.object);
            for (slot, (model, ..)) in drawable.iter().enumerate() {
                rp.set_bind_group(2, &v.object_binds[slot], &[]);
                rp.set_vertex_buffer(0, model.vertices.slice(..));
                rp.set_index_buffer(model.indices.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..model.count, 0, 0..1);
            }
        }
        // The atmosphere: the outgoing one fading out as the incoming one
        // fades in, over a clear (fully transparent) start.
        {
            let (from, to) = (show.from.wrapped().atmos, show.to.wrapped().atmos);
            let mut rp = begin_pass_clear(encoder, &t.atmos.view, wgpu::Color::TRANSPARENT);
            rp.set_bind_group(0, uniforms, &[]);
            let constant = |c: f64| wgpu::Color {
                r: c,
                g: c,
                b: c,
                a: c,
            };
            if let Some(pipeline) = &p.atmos[from] {
                let keep = if to == from || to != 0 {
                    1.0
                } else {
                    1.0 - show.mix as f64
                };
                rp.set_pipeline(pipeline);
                rp.set_blend_constant(constant(keep));
                rp.draw(0..3, 0..1);
            }
            if to != from
                && let Some(pipeline) = &p.atmos[to]
            {
                rp.set_pipeline(pipeline);
                rp.set_blend_constant(constant(show.mix as f64));
                rp.draw(0..3, 0..1);
            }
        }
        pass(
            encoder,
            &t.feedback[now].view,
            &p.post,
            &[uniforms, &t.post_bind[now]],
        );
        pass(
            encoder,
            &t.glow_a.view,
            &p.glow_across,
            &[uniforms, &t.feedback[now].bind],
        );
        pass(
            encoder,
            &t.glow_b.view,
            &p.glow_down,
            &[uniforms, &t.glow_a.bind],
        );
        if let Some(screen) = screen {
            pass(
                encoder,
                screen,
                &p.present,
                &[uniforms, &t.present_bind[now]],
            );
        }
        v.flip = 1 - now;
    }
}

fn texture_layout(device: &wgpu::Device, textures: u32) -> wgpu::BindGroupLayout {
    let mut entries: Vec<_> = (0..textures)
        .map(|binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        })
        .collect();
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: textures,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    });
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("textures"),
        entries: &entries,
    })
}

impl Gpu {
    fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    // Stating the size here makes the GPU compare it with the
                    // shader's `struct U` when a pipeline is built. Without
                    // it a mismatch only shows up on the first draw.
                    min_binding_size: std::num::NonZeroU64::new(size_of::<Uniforms>() as u64),
                },
                count: None,
            }],
        });
        // wgpu's default reaction to an error nobody caught is to abort the
        // program. At a party a bad frame is better than no picture.
        // The same mistake repeats every frame, so only the first few are
        // printed, in full (the short form says just "Validation Error").
        let printed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        device.on_uncaptured_error(Arc::new(move |error| {
            let n = printed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 3 {
                eprintln!("GPU error: {error:?}");
            } else if n == 3 {
                eprintln!("(further GPU errors are not shown)");
            }
        }));
        let one_texture_layout = texture_layout(&device, 1);
        let two_texture_layout = texture_layout(&device, 2);
        let three_texture_layout = texture_layout(&device, 3);
        let object_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(size_of::<ObjectUniforms>() as u64),
                },
                count: None,
            }],
        });
        // Mirrored repeat: when the feedback pass reads beyond the edge of the
        // previous frame it gets a reflection rather than a smeared border.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            device,
            queue,
            uniform_layout,
            one_texture_layout,
            two_texture_layout,
            three_texture_layout,
            object_layout,
            sampler,
        }
    }

    fn upload(&self, name: &str, m: &model::Model) -> GpuModel {
        let make = |contents: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(name),
                    contents,
                    usage,
                })
        };
        GpuModel {
            vertices: make(
                bytemuck::cast_slice(&m.vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: make(bytemuck::cast_slice(&m.indices), wgpu::BufferUsages::INDEX),
            count: m.indices.len() as u32,
        }
    }

    fn target(&self, width: u32, height: u32) -> Target {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            // Copyable too: the scene is copied into `behind` for glass to see through.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind = self.bind(&[&view]);
        Target {
            texture,
            view,
            bind,
        }
    }

    /// A bind group that lets a pass read the given textures.
    fn bind(&self, views: &[&wgpu::TextureView]) -> wgpu::BindGroup {
        let mut entries: Vec<_> = views
            .iter()
            .enumerate()
            .map(|(i, view)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: views.len() as u32,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textures"),
            layout: match views.len() {
                1 => &self.one_texture_layout,
                2 => &self.two_texture_layout,
                _ => &self.three_texture_layout,
            },
            entries: &entries,
        })
    }

    fn targets(&self, width: u32, height: u32) -> Targets {
        let scene_size = if height > MAX_SCENE_HEIGHT {
            ((width * MAX_SCENE_HEIGHT / height).max(1), MAX_SCENE_HEIGHT)
        } else {
            (width, height)
        };
        let scene = self.target(scene_size.0, scene_size.1);
        let behind = self.target(scene_size.0, scene_size.1);
        let attachment = |label, format| {
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: scene_size.0,
                        height: scene_size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: OBJECT_SAMPLES,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let depth = attachment("depth", DEPTH_FORMAT);
        let multisampled = attachment("objects", HDR_FORMAT);
        let feedback = [self.target(width, height), self.target(width, height)];
        let (glow_w, glow_h) = (
            (width / GLOW_DOWNSCALE).max(1),
            (height / GLOW_DOWNSCALE).max(1),
        );
        let glow_a = self.target(glow_w, glow_h);
        let glow_b = self.target(glow_w, glow_h);
        let atmos = self.target(
            (scene_size.0 / ATMOS_DOWNSCALE).max(1),
            (scene_size.1 / ATMOS_DOWNSCALE).max(1),
        );
        let post_bind = [
            self.bind(&[&scene.view, &feedback[1].view, &atmos.view]),
            self.bind(&[&scene.view, &feedback[0].view, &atmos.view]),
        ];
        let present_bind = [
            self.bind(&[&feedback[0].view, &glow_b.view]),
            self.bind(&[&feedback[1].view, &glow_b.view]),
        ];
        Targets {
            scene,
            behind,
            depth,
            multisampled,
            scene_size,
            atmos,
            feedback,
            glow_a,
            glow_b,
            post_bind,
            present_bind,
        }
    }

    /// Builds every pipeline from the shader folder, or returns the first
    /// shader error as text.
    fn pipelines(&self, dir: &Path, screen: wgpu::TextureFormat) -> Result<Pipelines, String> {
        let text = |r: Result<shaders::Assembled>| r.map_err(|e| format!("{e:#}"));
        let scene_source = text(shaders::assemble_scene(dir, &BACKGROUNDS, &PATTERNS))?;
        let scene = self.pipeline("scene", &scene_source, "fs_main", HDR_FORMAT, Reads::Nothing, Some(SCENE_BLEND))?;
        let post = text(shaders::assemble_pass(dir, "post"))?;
        let glow = text(shaders::assemble_pass(dir, "glow"))?;
        let present = text(shaders::assemble_pass(dir, "present"))?;
        let object = text(shaders::assemble_pass(dir, "object"))?;
        let mut atmos = vec![None];
        for name in ATMOSPHERES.iter().skip(1) {
            let source = text(shaders::assemble_atmos(dir, name))?;
            let label = format!("atmosphere {name}");
            atmos.push(Some(self.pipeline(
                &label,
                &source,
                "fs_main",
                HDR_FORMAT,
                Reads::Nothing,
                Some(CROSS_FADE),
            )?));
        }
        Ok(Pipelines {
            atmos,
            object: self.object_pipeline(&object, false)?,
            object_backdrop: self.object_pipeline(&object, true)?,
            scene,
            post: self.pipeline(
                "post",
                &post,
                "fs_main",
                HDR_FORMAT,
                Reads::ThreeTextures,
                None,
            )?,
            glow_across: self.pipeline(
                "glow",
                &glow,
                "fs_across",
                HDR_FORMAT,
                Reads::OneTexture,
                None,
            )?,
            glow_down: self.pipeline(
                "glow",
                &glow,
                "fs_down",
                HDR_FORMAT,
                Reads::OneTexture,
                None,
            )?,
            present: self.pipeline(
                "present",
                &present,
                "fs_main",
                screen,
                Reads::TwoTextures,
                None,
            )?,
        })
    }

    fn pipeline(
        &self,
        label: &str,
        source: &shaders::Assembled,
        fragment_entry: &str,
        format: wgpu::TextureFormat,
        reads: Reads,
        blend: Option<wgpu::BlendState>,
    ) -> Result<wgpu::RenderPipeline, String> {
        source.validate().map_err(|e| format!("{label}: {e}"))?;
        // The check above knows the WGSL language, but not what this pipeline
        // needs from the shader (the right entry points, bindings and output
        // type). Those errors come from the GPU, and an error scope collects
        // them instead of letting them abort the program.
        let errors = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = self.create_pipeline(label, source, fragment_entry, format, reads, blend);
        match pollster::block_on(errors.pop()) {
            Some(error) => Err(format!("{label}: {error}")),
            None => Ok(pipeline),
        }
    }

    /// The pipeline for 3D objects: real geometry with a depth test, unlike
    /// the full-screen passes.
    /// With `backdrop`, the full-screen pass that lays the scene down first.
    fn object_pipeline(
        &self,
        source: &shaders::Assembled,
        backdrop: bool,
    ) -> Result<wgpu::RenderPipeline, String> {
        source.validate().map_err(|e| format!("object: {e}"))?;
        let errors = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("object"),
                source: wgpu::ShaderSource::Wgsl(source.source.as_str().into()),
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("object"),
                bind_group_layouts: &[
                    Some(&self.uniform_layout),
                    Some(&self.one_texture_layout),
                    Some(&self.object_layout),
                ],
                immediate_size: 0,
            });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32];
        let vertex_layout = [Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<model::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("object"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(if backdrop { "vs_main" } else { "vs_object" }),
                    compilation_options: Default::default(),
                    buffers: if backdrop { &[] } else { &vertex_layout },
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(if backdrop { "fs_backdrop" } else { "fs_object" }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                // Both sides are drawn: an exporter that flips the winding of a
                // mesh would otherwise make half of it vanish. The depth test
                // keeps the nearer side on top.
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                // The backdrop sits behind everything and leaves the depth alone.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!backdrop),
                    depth_compare: Some(if backdrop {
                        wgpu::CompareFunction::Always
                    } else {
                        wgpu::CompareFunction::Less
                    }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: OBJECT_SAMPLES,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            });
        match pollster::block_on(errors.pop()) {
            Some(error) => Err(format!("object: {error}")),
            None => Ok(pipeline),
        }
    }

    fn create_pipeline(
        &self,
        label: &str,
        source: &shaders::Assembled,
        fragment_entry: &str,
        format: wgpu::TextureFormat,
        reads: Reads,
        blend: Option<wgpu::BlendState>,
    ) -> wgpu::RenderPipeline {
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.source.as_str().into()),
            });
        let layouts: &[Option<&wgpu::BindGroupLayout>] = match reads {
            Reads::Nothing => &[Some(&self.uniform_layout)],
            Reads::OneTexture => &[Some(&self.uniform_layout), Some(&self.one_texture_layout)],
            Reads::TwoTextures => &[Some(&self.uniform_layout), Some(&self.two_texture_layout)],
            Reads::ThreeTextures => &[Some(&self.uniform_layout), Some(&self.three_texture_layout)],
        };
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: layouts,
                immediate_size: 0,
            });
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(source.vertex),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
    }
}

/// Runs one full-screen pass.
fn pass(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bind_groups: &[&wgpu::BindGroup],
) {
    let mut rp = begin_pass(encoder, target);
    rp.set_pipeline(pipeline);
    for (i, group) in bind_groups.iter().enumerate() {
        rp.set_bind_group(i as u32, *group, &[]);
    }
    rp.draw(0..3, 0..1);
}

/// Starts drawing into `target`, cleared to black.
fn begin_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
) -> wgpu::RenderPass<'a> {
    begin_pass_clear(encoder, target, wgpu::Color::BLACK)
}

/// Starts drawing into `target`, cleared to `clear`.
fn begin_pass_clear<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    clear: wgpu::Color,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

    /// A GPU device with no window. `None` on a machine without a usable GPU,
    /// in which case these tests have nothing to check.
    fn headless() -> Option<Gpu> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        Some(Gpu::new(device, queue))
    }

    /// A private copy of the shader folder with one file replaced or appended to.
    fn shaders_with(tag: &str, file: &str, edit: impl FnOnce(String) -> String) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("noctiluca-gpu-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for entry in std::fs::read_dir(shaders::default_dir()).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
        }
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, edit(text)).unwrap();
        dir
    }

    #[test]
    fn the_real_shaders_build() {
        let Some(gpu) = headless() else { return };
        gpu.pipelines(&shaders::default_dir(), SCREEN)
            .unwrap_or_else(|e| panic!("{e}"));
    }

    /// Edits that are valid WGSL on their own but that the GPU must refuse.
    /// Each has to come back as an error; before this was fixed each one
    /// aborted the whole program.
    #[test]
    fn shaders_the_gpu_rejects_are_errors_not_crashes() {
        let Some(gpu) = headless() else { return };
        type Case = (&'static str, &'static str, fn(String) -> String);
        let cases: [Case; 5] = [
            ("empty-scene", "scene.wgsl", |_| String::new()),
            ("empty-present", "present.wgsl", |_| String::new()),
            ("half-glow", "glow.wgsl", |text| {
                text.split("// Down,").next().unwrap().to_string()
            }),
            ("extra-texture", "bg_tunnel.wgsl", |text| {
                format!("@group(1) @binding(0) var extra: texture_2d<f32>;\n{text}").replace(
                    "return col;",
                    "return col + textureLoad(extra, vec2i(0), 0).rgb;",
                )
            }),
            ("wrong-output", "scene.wgsl", |text| {
                text.replace("-> @location(0) vec4f", "-> @location(0) vec3f")
                    .replace("return vec4f(max(col, vec3f(0.0)), 1.0);", "return col;")
            }),
        ];
        for (tag, file, edit) in cases {
            let dir = shaders_with(tag, file, edit);
            let result = gpu.pipelines(&dir, SCREEN);
            std::fs::remove_dir_all(&dir).unwrap();
            assert!(
                result.is_err(),
                "{tag}: the GPU accepted a shader it should refuse"
            );
        }
    }

    /// The uniform block in the shader growing out of step with the Rust
    /// struct used to build fine and then abort on the first frame.
    #[test]
    fn a_uniform_block_that_outgrew_the_buffer_is_an_error() {
        let Some(gpu) = headless() else { return };
        let dir = shaders_with("big-uniforms", "common.wgsl", |text| {
            text.replace(
                "    wave: array<vec4f, 64>,",
                "    wave: array<vec4f, 64>,\n    extra: vec4f,",
            )
        });
        let result = gpu.pipelines(&dir, SCREEN);
        std::fs::remove_dir_all(&dir).unwrap();
        let message = result
            .err()
            .expect("a larger uniform block must be refused");
        assert!(!message.is_empty());
    }
}
