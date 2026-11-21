//! A picture drawn on the GPU, read back to the CPU without ever making a
//! frame wait: three buffers take turns; each frame copies into a free one,
//! and a buffer is read a frame or two later, once the GPU is done with it.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const FREE: u8 = 0;
/// Copied into this frame; to be mapped after the submit.
const COPIED: u8 = 1;
const MAPPING: u8 = 2;
const MAPPED: u8 = 3;
const FAILED: u8 = 4;
const SLOTS: usize = 3;

struct Slot {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    /// What came with the picture (its show time).
    tag: i64,
}

pub struct Readback {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pub size: (u32, u32),
    padded_row: u32,
    slots: Vec<Slot>,
}

/// A picture read back: BGRA (or whatever the target's format is), `stride`
/// bytes a row.
pub struct ReadPicture {
    pub bytes: Vec<u8>,
    pub stride: usize,
    pub size: (u32, u32),
    pub tag: i64,
}

impl Readback {
    pub fn new(device: &wgpu::Device, size: (u32, u32), format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("network screen"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let padded_row = (size.0 * 4).div_ceil(256) * 256;
        let slots = (0..SLOTS)
            .map(|_| Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("network screen readback"),
                    size: padded_row as u64 * size.1 as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                state: Arc::new(AtomicU8::new(FREE)),
                tag: 0,
            })
            .collect();
        Readback { texture, view, size, padded_row, slots }
    }

    /// Draw the picture into this.
    pub fn target(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// A buffer is free for this frame's picture (if not, skip the frame:
    /// the screen's link is behind anyway).
    pub fn has_room(&self) -> bool {
        self.slots.iter().any(|s| s.state.load(Ordering::Acquire) == FREE)
    }

    /// Copies the drawn picture into a free buffer, tagged `tag`.
    pub fn copy(&mut self, encoder: &mut wgpu::CommandEncoder, tag: i64) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|s| s.state.load(Ordering::Acquire) == FREE) else {
            return false;
        };
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &self.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &slot.buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.padded_row), rows_per_image: Some(self.size.1) },
            },
            wgpu::Extent3d { width: self.size.0, height: self.size.1, depth_or_array_layers: 1 },
        );
        slot.tag = tag;
        slot.state.store(COPIED, Ordering::Release);
        true
    }

    /// After the frame's commands are submitted: ask for the copies to be mapped.
    pub fn after_submit(&mut self) {
        for slot in &self.slots {
            if slot.state.load(Ordering::Acquire) == COPIED {
                slot.state.store(MAPPING, Ordering::Release);
                let state = slot.state.clone();
                slot.buffer.map_async(wgpu::MapMode::Read, .., move |r| {
                    state.store(if r.is_ok() { MAPPED } else { FAILED }, Ordering::Release);
                });
            }
        }
    }

    /// The pictures the GPU has finished, oldest first. Never waits.
    pub fn collect(&mut self, device: &wgpu::Device) -> Vec<ReadPicture> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut out = Vec::new();
        for slot in &mut self.slots {
            match slot.state.load(Ordering::Acquire) {
                MAPPED => {
                    if let Ok(data) = slot.buffer.get_mapped_range(..) {
                        out.push(ReadPicture { bytes: data.to_vec(), stride: self.padded_row as usize, size: self.size, tag: slot.tag });
                    }
                    slot.buffer.unmap();
                    slot.state.store(FREE, Ordering::Release);
                }
                FAILED => {
                    slot.state.store(FREE, Ordering::Release);
                }
                _ => {}
            }
        }
        out.sort_by_key(|p| p.tag);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A colour drawn on the GPU comes back, a frame later, without waiting.
    #[test]
    fn a_picture_comes_back_without_waiting() {
        let Some((device, queue)) = super::super::compositor::tests::tests_device() else { return };
        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let mut rb = Readback::new(&device, (100, 50), format);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: rb.target(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        assert!(rb.copy(&mut encoder, 42));
        queue.submit([encoder.finish()]);
        rb.after_submit();
        let mut got = Vec::new();
        for _ in 0..200 {
            got = rb.collect(&device);
            if !got.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(got.len(), 1);
        let p = &got[0];
        assert_eq!((p.tag, p.size, p.stride), (42, (100, 50), 512));
        // Red in BGRA.
        assert_eq!(&p.bytes[0..4], &[0, 0, 255, 255]);
        assert!(rb.has_room());
    }
}
