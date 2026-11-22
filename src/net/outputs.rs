//! Network screens as outputs: one link per output whose display is a
//! network screen, its surfaces composited here, read back, encoded and
//! sent. Their pictures are drawn ahead by the screens' delay, so what a
//! screen shows, later, is what the music is doing then.

use std::net::SocketAddr;
use std::time::Duration;

use crate::director::DirectorOut;
use crate::render::Renderer;
use crate::render::projector::{Projector, ViewSet, Views};
use crate::render::readback::Readback;
use crate::render::uniforms::Uniforms;
use crate::settings::{OutputPattern, Settings, network_name};
use crate::stage::{self, Planned};

use super::clock::now_ns;
use super::discovery::Found;
use super::stream::{Picture, ScreenStream, Status};

/// Pictures for network screens are at most this big (the boxes' video
/// decoders and Wi-Fi are happiest here).
pub const MAX_SIZE: (u32, u32) = (1920, 1080);
pub const FPS: u32 = 60;
pub const BITRATE: u32 = 12_000_000;
/// A link down this long is moved to wherever its box is heard now.
pub const MOVE_AFTER: Duration = Duration::from_secs(3);
/// A link down this long asks for the network to be searched again.
pub const SEARCH_AFTER: Duration = Duration::from_secs(5);

/// Where a box named `name` is heard now, if not at `at`: a box that left
/// the Wi-Fi for a moment, or restarted, often comes back at another address.
pub fn moved_to(name: &str, at: SocketAddr, found: &[Found]) -> Option<SocketAddr> {
    found.iter().filter(|f| f.name == name && f.app_open).map(|f| SocketAddr::new(f.ip, f.port)).find(|a| *a != at)
}

pub struct NetOutput {
    /// Which output of the settings.
    pub index: usize,
    /// The screen's name, as found on the network.
    pub name: String,
    pub stream: ScreenStream,
    projector: Option<Projector>,
    readback: Option<Readback>,
}

impl NetOutput {
    /// The size its pictures are drawn at: the screen's, at most 1080p.
    pub fn size(&self) -> (u32, u32) {
        let (w, h) = self.stream.status().hello.map(|h| (h.width, h.height)).filter(|s| s.0 > 0 && s.1 > 0).unwrap_or(MAX_SIZE);
        let scale = (MAX_SIZE.0 as f32 / w as f32).min(MAX_SIZE.1 as f32 / h as f32).min(1.0);
        // Even sizes: the encoder wants them.
        (((w as f32 * scale) as u32) & !1, ((h as f32 * scale) as u32) & !1)
    }
}

#[derive(Default)]
pub struct NetOutputs {
    pub list: Vec<NetOutput>,
}

/// Swaps red and blue in place (an RGBA target, the encoder takes BGRA).
fn swap_red_blue(bytes: &mut [u8]) {
    for p in bytes.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
}

impl NetOutputs {
    /// Links to the network screens the settings name, started or stopped to match.
    pub fn reconcile(&mut self, settings: &Settings, found: &[Found]) {
        let wanted: Vec<(usize, String)> = settings
            .output
            .outputs
            .iter()
            .enumerate()
            .filter_map(|(k, o)| Some((k, network_name(o.display.as_deref()?)?.to_string())))
            .collect();
        self.list.retain(|n| wanted.iter().any(|(k, name)| *k == n.index && *name == n.name));
        for (k, name) in wanted {
            if self.list.iter().any(|n| n.index == k) {
                continue;
            }
            // Started once the screen is heard of with its app open; the link
            // then reconnects by itself if the box drops off for a while.
            if let Some(f) = found.iter().find(|f| f.name == name && f.app_open) {
                let addr = SocketAddr::new(f.ip, f.port);
                self.list.push(NetOutput { index: k, name, stream: ScreenStream::start(addr, FPS, BITRATE), projector: None, readback: None });
            }
        }
        // A link that is down follows its box to a new address.
        for n in &mut self.list {
            let down = n.stream.status().down_since.is_some_and(|t| t.elapsed() >= MOVE_AFTER);
            if let Some(addr) = moved_to(&n.name, n.stream.addr, found).filter(|_| down) {
                n.stream = ScreenStream::start(addr, FPS, BITRATE);
            }
        }
        for n in &self.list {
            if let Some(o) = settings.output.outputs.get(n.index) {
                n.stream.set_extra_ms(o.delay_ms);
                n.stream.set_game_mode(o.tv_game_mode);
            }
        }
    }

    /// Some link has been down long enough to look for its box again.
    pub fn wants_search(&self) -> bool {
        self.list.iter().any(|n| n.stream.status().down_since.is_some_and(|t| t.elapsed() >= SEARCH_AFTER))
    }

    pub fn active(&self) -> bool {
        self.list.iter().any(|n| n.stream.ready())
    }

    /// How far ahead the pictures are drawn: the slowest screen's delay (the
    /// others show theirs at the same moment, so they stay together).
    pub fn lead_secs(&self) -> f32 {
        self.list.iter().filter(|n| n.stream.ready()).map(|n| n.stream.latency_secs()).fold(0.0, f32::max)
    }

    pub fn status(&self, output: usize) -> Option<Status> {
        self.list.iter().find(|n| n.index == output).map(|n| n.stream.status())
    }

    /// Draws every ready screen's picture for the moment it will be shown,
    /// and copies it for reading back. `outlined`: the surface the mapping
    /// page has selected. Returns the pictures drawn (for the previews).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        r: &mut Renderer,
        encoder: &mut wgpu::CommandEncoder,
        settings: &Settings,
        strobe: bool,
        u: &Uniforms,
        show: &DirectorOut,
        max_height: u32,
        show_pictures: bool,
        outlined: Option<usize>,
        echo_frames: usize,
    ) -> Option<Views> {
        let lead = self.lead_secs();
        let u = u.ahead(lead);
        let show_at = now_ns() + (lead * 1e9) as i64;
        let ready: Vec<usize> = (0..self.list.len()).filter(|&i| self.list[i].stream.ready() && self.list[i].readback.as_ref().is_none_or(|rb| rb.has_room())).collect();
        if ready.is_empty() {
            return None;
        }
        let plans: Vec<(usize, Vec<Planned>, (u32, u32))> = ready
            .iter()
            .map(|&i| {
                let n = &self.list[i];
                (i, stage::plan(&settings.output.surfaces, n.index, strobe, u.flow[3]), n.size())
            })
            .collect();
        let indexes: Vec<usize> = self.list.iter().map(|n| n.index).collect();
        let conf = |i: usize| settings.output.outputs.get(indexes[i]);
        let pattern = |i: usize| match conf(i).map_or(OutputPattern::Show, |o| o.showing) {
            OutputPattern::Show => crate::render::compositor::Pattern::Show,
            OutputPattern::Grid => crate::render::compositor::Pattern::Grid,
            OutputPattern::White => crate::render::compositor::Pattern::White,
            OutputPattern::Black => crate::render::compositor::Pattern::Black,
        };
        let black_outside = |i: usize| conf(i).is_none_or(|o| o.black_outside);
        let wants_views = show_pictures && plans.iter().any(|(i, ..)| pattern(*i) == crate::render::compositor::Pattern::Show);
        let views = wants_views.then(|| {
            let sizing: Vec<(&[Planned], (u32, u32))> = plans.iter().map(|(_, p, px)| (p.as_slice(), *px)).collect();
            let backgrounds: Vec<(u32, u32)> = plans.iter().filter(|(i, ..)| !black_outside(*i)).map(|(.., px)| *px).collect();
            Projector::render_views(r, encoder, ViewSet::Ahead, &sizing, &backgrounds, max_height, &u, show)
        });
        let main_size = r.view_size(ViewSet::Ahead.main()).unwrap_or(MAX_SIZE);
        let format = r.surface_format();
        for (i, planned, size) in &plans {
            let (pat, outside) = (pattern(*i), black_outside(*i));
            let k = indexes[*i];
            let n = &mut self.list[*i];
            if n.readback.as_ref().is_none_or(|rb| rb.size != *size) {
                n.readback = Some(Readback::new(r.device(), *size, format));
            }
            if n.projector.is_none() {
                n.projector = Projector::new(r.device(), r.shader_dir(), format).ok();
            }
            let (Some(projector), Some(rb)) = (n.projector.as_mut(), n.readback.as_mut()) else { continue };
            let outline = outlined.filter(|s| settings.output.surfaces.get(*s).is_some_and(|x| x.output == k));
            projector.compose(r.device(), r.queue(), encoder, rb.target(), *size, planned, views.as_ref(), main_size, pat, outside, echo_frames, outline);
            rb.copy(encoder, show_at);
        }
        views
    }

    /// After the frame's commands are submitted.
    pub fn after_submit(&mut self) {
        for n in &mut self.list {
            if let Some(rb) = n.readback.as_mut() {
                rb.after_submit();
            }
        }
    }

    /// Sends the pictures the GPU has finished. Never waits.
    pub fn deliver(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        let rgba = matches!(format, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb);
        for n in &mut self.list {
            let Some(rb) = n.readback.as_mut() else { continue };
            for mut p in rb.collect(device) {
                if rgba {
                    swap_red_blue(&mut p.bytes);
                }
                n.stream.offer(Picture { bgra: p.bytes, stride: p.stride, width: p.size.0, height: p.size.1, show_at: p.tag });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(name: &str, ip: [u8; 4], app_open: bool) -> Found {
        Found {
            name: name.into(),
            ip: std::net::IpAddr::from(ip),
            app_open,
            android_tv: true,
            model: String::new(),
            size: None,
            port: 7341,
            app_version: None,
        }
    }

    #[test]
    fn a_box_back_at_a_new_address_is_followed_there() {
        let at: SocketAddr = "10.0.0.5:7341".parse().unwrap();
        // Still where it was: nothing to do.
        assert_eq!(moved_to("Bar TV", at, &[heard("Bar TV", [10, 0, 0, 5], true)]), None);
        // Heard again at another address: go there.
        let found = [heard("Bar TV", [10, 0, 0, 5], true), heard("Bar TV", [10, 0, 0, 9], true)];
        assert_eq!(moved_to("Bar TV", at, &found), Some("10.0.0.9:7341".parse().unwrap()));
        // Another box, or the box without the app open, is not it.
        assert_eq!(moved_to("Bar TV", at, &[heard("Stage TV", [10, 0, 0, 7], true)]), None);
        assert_eq!(moved_to("Bar TV", at, &[heard("Bar TV", [10, 0, 0, 9], false)]), None);
    }

    #[test]
    fn red_and_blue_swap() {
        let mut px = vec![1, 2, 3, 4, 5, 6, 7, 8];
        swap_red_blue(&mut px);
        assert_eq!(px, vec![3, 2, 1, 4, 7, 6, 5, 8]);
    }
}
