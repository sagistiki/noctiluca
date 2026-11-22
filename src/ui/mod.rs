//! The control window, drawn with egui on the same GPU as the show: a
//! toolbar, a sidebar of sections (Live, Screens, Lights, Network, Venues,
//! Settings) and the page, in the Pro Suite language (style.rs).

pub mod icons;
pub mod lights_page;
pub mod live_page;
pub mod mapping_page;
pub mod network_page;
pub mod settings_page;
pub mod venues;
pub mod widgets;
pub mod style;

use std::sync::Arc;

use winit::window::Window;

use crate::analysis::Vibe;
use crate::app_state::{Action, AppState, Page};
use crate::settings::Settings;

/// What the pages show about the running program, gathered each frame.
#[derive(Clone, Debug, Default)]
pub struct Live {
    /// The input being listened to, and the ones that could be.
    pub input: String,
    pub inputs: Vec<String>,
    /// Loudness bands for the meter, 0..1, bass first.
    pub levels: Vec<f32>,
    pub kick_found: bool,
    pub bpm: f32,
    pub locked: bool,
    pub vibe: Vibe,
    pub fps: f32,
    /// Screens other than the control window's, by name.
    pub displays: Vec<String>,
    /// The connected outputs' displays, for the top bar ("XG2705 + LG").
    pub output: Option<String>,
    /// Per output (as in the settings): its picture size, while its window is open.
    pub output_sizes: Vec<Option<(u32, u32)>>,
    /// The show preview's width over height (0: 16:9). With projectors on
    /// it is their main picture, whatever shape their surfaces give it.
    pub preview_aspect: f32,
    /// The venue files (in the venue folder, and opened from elsewhere
    /// lately), and the one in use (with changes since it was saved, or not).
    pub venues: Vec<venues::VenueInfo>,
    pub venue_active: Option<String>,
    pub venue_path: Option<std::path::PathBuf>,
    pub venue_changed: bool,
    /// The venue folder as people read it: "~/Documents/Noctiluca Venues".
    pub venue_library: String,
    /// Screens on the network (Android TVs, boxes with the app open).
    pub net_screens: Vec<NetScreen>,
    /// Per output: its network link, when its display is a network screen.
    pub net_links: Vec<Option<NetLink>>,
    /// A search for the network screens has just started.
    pub net_searching: bool,
    /// DMX: the interface, the ports there are, each light's colour now.
    pub dmx: Option<lights_page::DmxLive>,
    pub dmx_ports: Vec<String>,
    pub lights: Vec<[f32; 3]>,
    /// The lights' effect playing now.
    pub light_effect: Option<crate::lights::Effect>,
    /// The show now: section ("Drop"), theme ("Red room"), held by hand.
    pub section: String,
    pub theme: String,
    pub look_held: bool,
    /// The beat: bar and beat in it (1-based), for the toolbar.
    pub bar_beat: (u64, u64),
    /// The set's last twenty minutes: sections, themes, light effects.
    pub timeline: [Vec<crate::timeline::Run>; 3],
    pub timeline_secs: usize,
    /// A message from the program, shown under the top bar.
    pub message: Option<String>,
    pub version: &'static str,
}

/// A screen on the network, as the settings page lists it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetScreen {
    pub name: String,
    pub ip: String,
    pub model: String,
    pub app_open: bool,
    pub android_tv: bool,
    /// The app's version when it is open.
    pub app_version: Option<String>,
    /// What installing found out, when it was asked (adb).
    pub install: Option<crate::net::adb::BoxState>,
    /// Working on it ("Installing…"), or what went wrong.
    pub busy: Option<String>,
    pub error: Option<String>,
    /// The output it is the display of.
    pub used_by: Option<usize>,
}

/// A network output's link, for the mapping page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetLink {
    pub connected: bool,
    pub latency_ms: f32,
    pub round_trip_ms: f32,
    pub shown: u64,
    pub late: u64,
    /// HDR kinds the TV takes; empty: SDR only.
    pub hdr: Vec<String>,
    pub error: Option<String>,
}

/// What a page asks the program to do. Plain settings changes are made
/// directly on the `Settings` the page is given.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    App(Action),
    /// `None`: choose the best input by itself.
    SelectInput(Option<String>),
    /// Open this venue file.
    LoadVenue(std::path::PathBuf),
    /// Save into the venue file in use (Save As when there is none).
    SaveVenue,
    /// Save into this venue file, replacing what it held.
    SaveVenueTo(std::path::PathBuf),
    /// The Mac's Save panel, for a new venue file.
    SaveVenueAs,
    /// The Mac's Open panel, for a venue file from anywhere.
    OpenVenue,
    RevealVenue(std::path::PathBuf),
    RenameVenue(std::path::PathBuf, String),
    TrashVenue(std::path::PathBuf),
    /// Install (or update) the screen app on the box at this address, and open it.
    NetInstall(String),
    /// Open the app on the box (it was installed already).
    NetOpen(String),
    /// Use this network screen as an output's display (a new output if all are taken).
    NetUse(String),
    /// Look for the network screens again from scratch.
    NetSearch,
    /// Start the lights' interface afresh (it stopped answering, or was swapped).
    LightsReconnect,
    /// Flash light `i` white for a moment, to see which one it is.
    LightIdentify(usize),
    /// The show: a new look now; hold the look (or let go); the next palette.
    NextLook,
    HoldLook,
    NextPalette,
    Quit,
}

/// egui wired to a winit window and a wgpu device.
pub struct Ui {
    pub ctx: egui::Context,
    state: egui_winit::State,
    painter: egui_wgpu::Renderer,
    /// Textures shown by the pages: the show's preview, and the projector's.
    textures: [Option<egui::TextureId>; 2],
    pub pages: PageState,
}

/// What the pages remember from frame to frame.
#[derive(Clone, Debug, Default)]
pub struct PageState {
    pub venues: venues::VenuesState,
    pub mapping: mapping_page::MappingState,
    pub lights: lights_page::LightsState,
    pub live_tab: live_page::Tab,
}

/// The textures a page may show.
#[derive(Clone, Copy, Debug, Default)]
pub struct Previews {
    /// The show (the menu's big card).
    pub show: Option<egui::TextureId>,
    /// What the projector shows (the mapping page's canvas).
    pub output: Option<egui::TextureId>,
}

pub const SHOW_TEXTURE: usize = 0;
pub const OUTPUT_TEXTURE: usize = 1;

impl Ui {
    pub fn new(window: &Window, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        style::apply(&ctx);
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        let painter =
            egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
        Self {
            ctx,
            state,
            painter,
            textures: [None; 2],
            pages: PageState::default(),
        }
    }

    /// Hands a window event to egui; true if egui used it (a click on a
    /// button, typing in a field) and the show should not see it.
    pub fn on_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    /// Makes `view` available to the pages as texture `slot` (SHOW_TEXTURE
    /// or OUTPUT_TEXTURE).
    pub fn set_texture(
        &mut self,
        slot: usize,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
    ) -> egui::TextureId {
        match self.textures[slot] {
            Some(id) => {
                self.painter.update_egui_texture_from_wgpu_texture(
                    device,
                    view,
                    wgpu::FilterMode::Linear,
                    id,
                );
                id
            }
            None => {
                let id =
                    self.painter
                        .register_native_texture(device, view, wgpu::FilterMode::Linear);
                self.textures[slot] = Some(id);
                id
            }
        }
    }

    /// Runs the pages for one frame and paints them over a cleared `target`.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        window: &Arc<Window>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        build: impl FnMut(&mut egui::Ui),
    ) {
        let input = self.state.take_egui_input(window);
        let platform = self.paint(input, device, queue, encoder, target, size, build);
        self.state.handle_platform_output(window, platform);
    }

    /// Runs the pages once on `input` and paints them over a cleared
    /// `target`; returns what egui wants from the platform (cursor, clipboard).
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        input: egui::RawInput,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        build: impl FnMut(&mut egui::Ui),
    ) -> egui::PlatformOutput {
        let out = self.ctx.run_ui(input, build);
        let jobs = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.0, size.1],
            pixels_per_point: out.pixels_per_point,
        };
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                self.painter.update_texture(device, queue, *id, delta);
            }
        }
        self.painter
            .update_buffers(device, queue, encoder, &jobs, &screen);
        {
            let bg = style::BG.to_normalized_gamma_f32();
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // The sketch's near-black, as the surface (sRGB) expects it.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: srgb_to_linear(bg[0]) as f64,
                            g: srgb_to_linear(bg[1]) as f64,
                            b: srgb_to_linear(bg[2]) as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.painter
                .render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        for id in &out.textures_delta.free {
            self.painter.free_texture(id);
        }
        out.platform_output
    }

    /// Paints a page off-screen at `size` pixels, `scale` pixels per point,
    /// with no input: for saved pictures of the pages. Laid out twice first,
    /// as egui settles some sizes over more than one pass.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_offscreen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        scale: f32,
        mut build: impl FnMut(&mut egui::Ui),
    ) {
        let points = egui::vec2(size.0 as f32, size.1 as f32) / scale;
        let input = || {
            let mut raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, points)),
                ..Default::default()
            };
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .native_pixels_per_point = Some(scale);
            raw
        };
        // Every pass is painted: the first ones also deliver the font
        // texture, which later passes rely on.
        for _ in 0..2 {
            self.paint(input(), device, queue, encoder, target, size, &mut build);
        }
        self.paint(input(), device, queue, encoder, target, size, build);
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

const TOOLBAR_H: f32 = 46.0;
const SIDEBAR_W: f32 = 208.0;
const SIDEBAR_NARROW: f32 = 56.0;

/// A page's name, as the sidebar and the toolbar say it.
pub fn page_name(page: Page) -> &'static str {
    match page {
        Page::Live => "Live",
        Page::Mapping => "Screens",
        Page::Lights => "Lights",
        Page::Network => "Network screens",
        Page::Venues => "Venues",
        Page::Settings => "Settings",
    }
}

/// Draws the window: toolbar, sidebar, and the page the state asks for.
pub fn draw_page(
    ui: &mut egui::Ui,
    state: &AppState,
    settings: &mut Settings,
    live: &Live,
    previews: Previews,
    pages: &mut PageState,
    actions: &mut Vec<UiAction>,
) {
    use egui::{Rect, pos2, vec2};
    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, 0.0, style::BG);
    // The toolbar.
    let bar = Rect::from_min_size(full.min, vec2(full.width(), TOOLBAR_H));
    ui.painter().rect_filled(bar, 0.0, style::TOOLBAR);
    ui.painter().line_segment([bar.left_bottom(), bar.right_bottom()], egui::Stroke::new(1.0, style::LINE));
    toolbar(ui, bar, state, live, pages, actions);
    // The sidebar: names when there is room, icons when not.
    let narrow = full.width() < 980.0;
    let side_w = if narrow { SIDEBAR_NARROW } else { SIDEBAR_W };
    let body = Rect::from_min_max(pos2(full.left(), bar.bottom() + 1.0), full.max);
    let side = Rect::from_min_size(body.min, vec2(side_w, body.height()));
    ui.painter().rect_filled(side, 0.0, style::CARD);
    ui.painter().line_segment([side.right_top(), side.right_bottom()], egui::Stroke::new(1.0, style::LINE));
    ui.scope_builder(egui::UiBuilder::new().max_rect(side.shrink2(vec2(8.0, 10.0))), |ui| sidebar(ui, state, settings, live, narrow, actions));
    // The page.
    let content = Rect::from_min_max(pos2(side.right() + 1.0, body.top()), body.max).shrink2(vec2(18.0, 16.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
        ui.set_clip_rect(content.expand(4.0));
        match state.page {
            Page::Live => live_page::draw(ui, state, settings, live, previews.show, pages, actions),
            Page::Mapping => mapping_page::draw(ui, settings, live, previews.output, &mut pages.mapping, actions),
            Page::Lights => {
                ui.label(style::title("Lights"));
                ui.add_space(8.0);
                let area = ui.available_rect_before_wrap();
                lights_page::draw(ui, area, settings, live, &mut pages.lights, actions);
            }
            Page::Network => network_page::draw(ui, live, actions),
            Page::Venues => venues::page(ui, live, &mut pages.venues, actions),
            Page::Settings => settings_page::draw(ui, settings, live, actions),
        }
    });
    // A message from the program: a toast at the bottom, for a few seconds.
    if let Some(m) = &live.message {
        let p = ui.painter();
        let galley = p.layout(style::visual(m), egui::FontId::proportional(13.0), style::TEXT, 560.0);
        let r = Rect::from_center_size(pos2(content.center().x, full.bottom() - 34.0), galley.size() + vec2(28.0, 16.0));
        p.rect(r, egui::CornerRadius::same(9), style::TOOLBAR, egui::Stroke::new(1.0, style::LINE), egui::StrokeKind::Inside);
        p.galley(r.min + vec2(14.0, 8.0), galley, style::TEXT);
    }
    venues::dialogs(ui.ctx(), live, &mut pages.venues, actions);
    ui.advance_cursor_after_rect(full);
}

fn toolbar(ui: &mut egui::Ui, bar: egui::Rect, state: &AppState, live: &Live, pages: &mut PageState, actions: &mut Vec<UiAction>) {
    use egui::{Rect, RichText, pos2, vec2};
    let inner = bar.shrink2(vec2(14.0, 0.0));
    // Left: the app and where you are.
    let left = Rect::from_min_max(inner.min, pos2(inner.center().x - 190.0, inner.bottom()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(left).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.label(style::semibold("Noctiluca", 13.5));
        let venue = live.venue_active.as_deref().map(style::visual).unwrap_or_else(|| "No venue".into());
        ui.label(RichText::new(format!("{venue} · {}", page_name(state.page))).size(12.5).color(style::MUTED));
    });
    // Middle: the transport and the beat.
    let mid = Rect::from_center_size(inner.center(), vec2(380.0, inner.height()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(mid).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let running = state.running;
        let (icon, tip) = if running { (icons::Icon::Stop, "Stop the show") } else { (icons::Icon::Play, "Start the show") };
        let (r, resp) = ui.allocate_exact_size(vec2(44.0, 28.0), egui::Sense::click());
        ui.painter().rect_filled(r, egui::CornerRadius::same(7), if running { style::LINE } else { style::ACCENT });
        icons::paint(ui, Rect::from_center_size(r.center(), vec2(12.0, 12.0)), icon, egui::Color32::WHITE);
        if resp.on_hover_text(tip).clicked() {
            actions.push(UiAction::App(if running { Action::Stop } else { Action::Start }));
        }
        let beat = if live.bpm > 0.0 {
            format!("{:.1} BPM · {}.{} · {}", live.bpm, live.bar_beat.0, live.bar_beat.1, if live.locked { "locked" } else { "searching" })
        } else {
            "No tempo yet · listening".to_string()
        };
        egui::Frame::new().fill(style::WELL).corner_radius(egui::CornerRadius::same(7)).inner_margin(egui::Margin::symmetric(12, 5)).show(ui, |ui| {
            ui.set_min_width(250.0);
            ui.horizontal(|ui| {
                style::dot(ui, if live.locked { style::GREEN } else if live.bpm > 0.0 { style::AMBER } else { style::TERTIARY });
                ui.label(RichText::new(beat).size(12.0));
            });
        });
    });
    // Right: the venue, saved or changed.
    let right = Rect::from_min_max(pos2(inner.center().x + 200.0, inner.top()), inner.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(right).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
        venues::status_right(ui, live, &mut pages.venues, actions);
    });
}

fn sidebar(ui: &mut egui::Ui, state: &AppState, settings: &Settings, live: &Live, narrow: bool, actions: &mut Vec<UiAction>) {
    use egui::{Rect, vec2};
    use icons::Icon;
    let outputs_ok = live.output_sizes.iter().any(|s| s.is_some());
    let outputs_wanted = settings.output.outputs.iter().any(|o| o.display.is_some());
    let screens_dot = if outputs_ok { Some(style::GREEN) } else if outputs_wanted { Some(style::AMBER) } else { None };
    let lights_dot = match &live.dmx {
        Some(d) if d.sending => Some(style::GREEN),
        Some(_) if !settings.lighting.fixtures.is_empty() => Some(style::AMBER),
        _ => None,
    };
    let net_dot = live.net_screens.iter().any(|s| s.app_open).then_some(style::GREEN);
    type Item = (Page, Icon, Option<egui::Color32>);
    let groups: [(&str, Vec<Item>); 3] = [
        ("Show", vec![(Page::Live, Icon::Pulse, None)]),
        ("Room", vec![(Page::Mapping, Icon::Display, screens_dot), (Page::Lights, Icon::Spot, lights_dot), (Page::Network, Icon::Wifi, net_dot)]),
        ("Library", vec![(Page::Venues, Icon::Pin, None), (Page::Settings, Icon::Gear, None)]),
    ];
    for (title, items) in groups {
        if !narrow {
            ui.add_space(6.0);
            ui.label(style::semibold(title, 11.0).color(style::TERTIARY));
        } else {
            ui.add_space(10.0);
        }
        for (page, icon, dot) in items {
            let on = state.page == page;
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), egui::Sense::click());
            let fill = if on { style::ACCENT } else if resp.hovered() { style::LINE } else { egui::Color32::TRANSPARENT };
            ui.painter().rect_filled(r, egui::CornerRadius::same(6), fill);
            let fg = if on { egui::Color32::WHITE } else { style::TEXT };
            let icon_rect = if narrow { Rect::from_center_size(r.center(), vec2(16.0, 16.0)) } else { Rect::from_center_size(r.left_center() + vec2(18.0, 0.0), vec2(16.0, 16.0)) };
            icons::paint(ui, icon_rect, icon, fg);
            if !narrow {
                ui.painter().text(r.left_center() + vec2(36.0, 0.0), egui::Align2::LEFT_CENTER, page_name(page), egui::FontId::proportional(13.0), fg);
            }
            if let Some(c) = dot {
                let at = if narrow { r.right_top() + vec2(-9.0, 8.0) } else { r.right_center() - vec2(12.0, 0.0) };
                ui.painter().circle_filled(at, 3.5, if on { egui::Color32::WHITE } else { c });
            }
            let resp = if narrow { resp.on_hover_text(page_name(page)) } else { resp };
            if resp.clicked() && !on {
                actions.push(UiAction::App(Action::Open(page)));
            }
        }
    }
    // At the foot: the version, and Quit.
    let foot = egui::Rect::from_min_max(ui.max_rect().left_bottom() - vec2(0.0, 52.0), ui.max_rect().right_bottom());
    ui.scope_builder(egui::UiBuilder::new().max_rect(foot), |ui| {
        if !narrow {
            ui.label(style::muted(format!("Noctiluca {}", live.version)));
        }
        let quit = if narrow {
            let (r, resp) = ui.allocate_exact_size(vec2(40.0, 30.0), egui::Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, egui::CornerRadius::same(6), style::LINE);
            }
            icons::paint(ui, Rect::from_center_size(r.center(), vec2(15.0, 15.0)), Icon::Power, style::MUTED);
            resp
        } else {
            ui.add(egui::Button::new(egui::RichText::new("Quit").size(12.5)).fill(egui::Color32::TRANSPARENT))
        };
        if quit.on_hover_text("Quit Noctiluca (Cmd+Q)").clicked() {
            actions.push(UiAction::Quit);
        }
    });
}



#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>, state: &AppState, settings: &mut Settings, pages: &mut PageState) -> Vec<UiAction> {
        let live = Live { version: "test", output_sizes: vec![None], net_links: vec![None], ..Live::default() };
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), events, ..Default::default() };
        let mut actions = Vec::new();
        let _ = ctx.run_ui(input, |ui| draw_page(ui, state, settings, &live, Previews::default(), pages, &mut actions));
        actions
    }

    /// Every page draws at a big, a laptop and a small window without
    /// falling over.
    #[test]
    fn every_page_draws_at_any_window_size() {
        let ctx = egui::Context::default();
        style::apply(&ctx);
        for page in [Page::Live, Page::Mapping, Page::Lights, Page::Network, Page::Venues, Page::Settings] {
            for size in [egui::vec2(1440.0, 900.0), egui::vec2(1024.0, 640.0), egui::vec2(760.0, 520.0)] {
                let state = AppState { page, ..AppState::default() };
                let mut settings = Settings::default();
                let mut pages = PageState::default();
                for _ in 0..2 {
                    frame(&ctx, size, vec![], &state, &mut settings, &mut pages);
                }
            }
        }
    }

    /// The toolbar's play button starts the show; the sidebar opens pages.
    #[test]
    fn the_toolbar_starts_and_the_sidebar_navigates() {
        let ctx = egui::Context::default();
        style::apply(&ctx);
        let size = egui::vec2(1440.0, 900.0);
        let state = AppState::default();
        let mut settings = Settings::default();
        let mut pages = PageState::default();
        frame(&ctx, size, vec![], &state, &mut settings, &mut pages);
        let click = |at: egui::Pos2| vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }, egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }];
        // The play button: left of the middle of the toolbar.
        let play = egui::pos2(size.x / 2.0 - 190.0 + 22.0, TOOLBAR_H / 2.0);
        let a = frame(&ctx, size, click(play), &state, &mut settings, &mut pages);
        assert!(a.contains(&UiAction::App(Action::Start)), "{a:?}");
        // The sidebar's "Lights" row: Show header, Live, Room header, Screens, Lights.
        let mut found = false;
        for y in (60..340).step_by(6) {
            let a = frame(&ctx, size, click(egui::pos2(80.0, y as f32)), &state, &mut settings, &mut pages);
            if a.contains(&UiAction::App(Action::Open(Page::Lights))) {
                found = true;
                break;
            }
        }
        assert!(found, "no sidebar row opened Lights");
    }
}
