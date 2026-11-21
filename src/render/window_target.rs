//! A window the renderer can draw into besides its own: the projector's.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use winit::window::Window;

pub struct WindowTarget {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl WindowTarget {
    pub(super) fn new(
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        window: Arc<Window>,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let size = window.inner_size();
        let surface = instance.create_surface(window)?;
        let mut config = surface
            .get_default_config(adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow!("the projector's surface is not supported by this GPU"))?;
        let formats = surface.get_capabilities(adapter).formats;
        // The same format as the control window, so one set of pipelines serves both.
        config.format = if formats.contains(&format) {
            format
        } else {
            formats[0]
        };
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(device, &config);
        Ok(Self { surface, config })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);
    }

    /// The next picture to draw into, or `None` this frame.
    pub fn acquire(&mut self, device: &wgpu::Device) -> Option<wgpu::SurfaceTexture> {
        use wgpu::CurrentSurfaceTexture as Acquired;
        match self.surface.get_current_texture() {
            Acquired::Success(frame) | Acquired::Suboptimal(frame) => Some(frame),
            Acquired::Outdated | Acquired::Lost => {
                self.surface.configure(device, &self.config);
                None
            }
            _ => None,
        }
    }
}
