//! noctiluca: listens to music and draws it.

use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clap::Parser;
use ringbuf::traits::Consumer;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowId};

use noctiluca::analysis::{Analyzer, Features, Mood};
use noctiluca::app_state::{Action, AppState};
use noctiluca::audio::{self, AudioIn, Recorder, marks_path};
use noctiluca::burst::Burst;
use noctiluca::director::DirectorOptions;
use noctiluca::director::{
    ATMOSPHERES, BACKGROUNDS, Director, DirectorOut, Look, MATERIALS, OBJECTS, PALETTE_NAMES,
    PATTERNS, POST_NAMES, THEMES,
};
use noctiluca::lightning::Lightning;
use noctiluca::output::{OutputChange, Reason, reconcile};
use noctiluca::quality::Governor;
use noctiluca::render::Renderer;
use noctiluca::render::compositor::{Compositor, Pattern, SurfaceDraw};
use noctiluca::render::projector::{MAIN_VIEW, Projector, ViewSet, Views};
use noctiluca::render::shaders;
use noctiluca::render::uniforms::{FLOOR_FADE_SECS, Flow, FrameInfo, Uniforms, fade_toward};
use noctiluca::render::window_target::WindowTarget;
use noctiluca::settings::OutputPattern;
use noctiluca::settings::{self, Saver, Settings};
use noctiluca::stage;
use noctiluca::strobe::StrobeLimiter;
use noctiluca::sync::BeatClock;
use noctiluca::ui::{self, Live, Ui, UiAction};

#[derive(Parser)]
#[command(name = "noctiluca", about = "Noctiluca: a live club visualizer")]
struct Args {
    /// Play and analyse a WAV file instead of listening to an input device.
    #[arg(long)]
    file: Option<PathBuf>,

    /// With --file: analyse without sending sound to the speakers.
    #[arg(long)]
    mute: bool,

    /// Input device to listen to (part of its name is enough).
    #[arg(long)]
    device: Option<String>,

    /// Print the input devices and exit.
    #[arg(long)]
    list_devices: bool,

    /// Folder with the .wgsl shader files.
    #[arg(long)]
    shaders: Option<PathBuf>,

    /// Folder with the .glb 3D models.
    #[arg(long)]
    models: Option<PathBuf>,

    /// The program always starts in a window now; accepted so that older
    /// commands that pass it still work.
    #[arg(long, hide = true)]
    windowed: bool,

    /// The projector's screen, as numbered by --list-monitors (the same as
    /// choosing it in the menu).
    #[arg(long)]
    monitor: Option<usize>,

    /// A network screen (a box running the Noctiluca Screen app) to use as
    /// an output, by its name (the same as choosing it in Mapping).
    #[arg(long)]
    screen: Option<String>,

    /// Start the show at once, as pressing Start does.
    #[arg(long)]
    play: bool,

    /// Print the connected screens and exit.
    #[arg(long)]
    list_monitors: bool,

    /// Print the frame rate every two seconds.
    #[arg(long)]
    stats: bool,

    /// Background to start with: 0 tunnel, 1 grid, 2 liquid, 3 shapes, 4 void.
    #[arg(long)]
    bg: Option<usize>,

    /// Pattern to start with: 0 rings, 1 scope, 2 kaleido, 3 bars, 4 stripes, 5 frames,
    /// 6 radar, 7 spiral, 8 halftone.
    #[arg(long)]
    pattern: Option<usize>,

    /// Palette to start with: 0 neon, 1 acid, 2 fire, 3 ice, 4 ultraviolet,
    /// 5 red, 6 white, 7 terminal, 8 violet, 9 amber.
    #[arg(long)]
    palette: Option<usize>,

    /// Effect preset to start with: 0 clean, 1 trails, 2 zoom, 3 kaleido, 4 drift,
    /// 5 crt, 6 prism, 7 floor, 8 vhs, 9 thermal, 10 film.
    #[arg(long)]
    post: Option<usize>,

    /// 3D object to start with: 0 none, 1 mirror ball, 2 skull, 3 prism, 4 gem,
    /// 5 knot, 6 eye, 7 monolith, 8 chain, 9 speakers, 10 crystal, 11 planet,
    /// 12 corridor.
    #[arg(long)]
    object: Option<usize>,

    /// Atmosphere to start with: 0 none, 1 storm, 2 smoke, 3 fire, 4 beams, 5 aurora.
    #[arg(long)]
    atmos: Option<usize>,

    /// What the object is made of: 0 chrome, 1 glass.
    #[arg(long)]
    material: Option<usize>,

    /// Keep the starting look; nothing changes on its own (L toggles this).
    #[arg(long)]
    lock: bool,

    /// Makes the show repeatable: the same seed and music give the same changes.
    #[arg(long)]
    seed: Option<u64>,

    /// Never flash the screen.
    #[arg(long)]
    no_strobe: bool,

    /// Brightest a flash may get, 0..1.
    #[arg(long, default_value_t = 0.6)]
    strobe_max: f32,

    /// Save one frame as a PNG and exit (no window is shown).
    #[arg(long)]
    snapshot: Option<PathBuf>,

    /// With --snapshot: picture width in pixels.
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// With --snapshot: picture height in pixels.
    #[arg(long, default_value_t = 720)]
    height: u32,

    /// With --snapshot: draw as fast as possible and report the time per frame.
    #[arg(long)]
    bench: bool,

    /// Save what the app hears to this WAV file, for analysing later.
    /// While recording, M marks a moment (say, where the kick leaves or returns).
    #[arg(long)]
    record: Option<PathBuf>,

    /// Start the animation clocks at this value, as if the app had been
    /// running for hours. For checking how the visuals age.
    #[arg(long, hide = true)]
    flow_start: Option<f32>,
    /// Snapshots only: draw the overlay (HUD) in the picture.
    #[arg(long, hide = true)]
    snapshot_hud: bool,

    /// Snapshots only: save a page of the menu instead of the show
    /// ("menu", "settings" or "mapping"), drawn at two pixels per point, or
    /// "output": what a projector would show, at --width x --height.
    #[arg(long, hide = true)]
    snapshot_page: Option<String>,

    /// Snapshots only: use a demonstration layout of three surfaces (left
    /// and right Complement, Main in the middle, keystoned), not the saved one.
    #[arg(long, hide = true)]
    snapshot_demo_layout: bool,

    /// Snapshots only: what the demonstration projector shows ("grid" etc.).
    #[arg(long, hide = true)]
    snapshot_pattern: Option<String>,

    /// Snapshots only: the mapping page's side tab ("transform", "look", "output").
    #[arg(long, hide = true)]
    snapshot_mapping_tab: Option<String>,

    /// Snapshots only: freeze the picture part way through a change to
    /// another background and pattern ("fade", "ink", "zoom").
    #[arg(long, hide = true)]
    snapshot_transition: Option<String>,

    /// Snapshots only: how far the change has come, 0..1.
    #[arg(long, hide = true, default_value_t = 0.5)]
    snapshot_mix: f32,

    /// With --snapshot: how many seconds to run before saving.
    #[arg(long, default_value_t = 3.0)]
    at: f32,
}

/// The accepted safety limit for flashing content.
const MAX_FLASHES_PER_SEC: f32 = 3.0;
const RECONNECT_AFTER_SECS: f32 = 3.0;
/// The overlay stays this long after the mouse stops, then fades out.
const HUD_IDLE_SECS: f32 = 3.0;
const HUD_FADE_SECS: f32 = 0.8;
/// How long a message stays on screen.
const LABEL_SECS: f32 = 4.0;
const RECONNECT_EVERY_SECS: f32 = 5.0;

/// The shortest time between frames: 60 a second. A little under 1/60 s,
/// so that on a 120 Hz screen every other refresh is caught, not every third.
const FRAME_INTERVAL: Duration = Duration::from_micros(15_500);
/// ...and while the window is hidden, five a second.
const HIDDEN_FRAME_INTERVAL: Duration = Duration::from_millis(200);
/// The menu's live preview of the show, in pixels.
const PREVIEW_SIZE: (u32, u32) = (960, 540);
/// How fast the visuals move through a breakdown, and how quickly they get there.
const BREAKDOWN_PACE: f32 = 0.4;
const PACE_EASE_SECS: f32 = 1.2;
/// Input devices, displays and venues are looked up this often while the menu shows.
const REFRESH_LISTS: Duration = Duration::from_secs(4);
/// How long a menu message stays.
const MESSAGE_SECS: f32 = 6.0;
/// Displays are looked at this often, to notice a projector plugged in or out.
const CHECK_DISPLAYS: Duration = Duration::from_secs(1);

/// The mapping page's picture of what the projector shows.
#[derive(Default)]
struct OutputPreview {
    projector: Option<Projector>,
    texture: Option<(wgpu::Texture, wgpu::TextureView, (u32, u32))>,
}

/// The mapping page's preview is this wide; its height follows the projector's shape.
const OUTPUT_PREVIEW_WIDTH: u32 = 1280;

impl OutputPreview {
    /// Draws what the projector shows (or would show) into the preview
    /// texture. With a projector running its pictures are reused; otherwise
    /// they are drawn here, small.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        r: &mut Renderer,
        encoder: &mut wgpu::CommandEncoder,
        settings: &Settings,
        output: usize,
        output_px: (u32, u32),
        frame_views: Option<&Views>,
        u: &Uniforms,
        show: &DirectorOut,
    ) -> Option<wgpu::TextureView> {
        if self.projector.is_none() {
            self.projector = Projector::new(r.device(), r.shader_dir(), r.surface_format()).ok();
        }
        let size = (
            OUTPUT_PREVIEW_WIDTH,
            (OUTPUT_PREVIEW_WIDTH as f32 * output_px.1 as f32 / output_px.0.max(1) as f32)
                .round()
                .max(1.0) as u32,
        );
        if self.texture.as_ref().is_none_or(|(_, _, s)| *s != size) {
            let texture = r.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("output preview"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: r.surface_format(),
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.texture = Some((texture, view, size));
        }
        let target = self.texture.as_ref()?.1.clone();
        let conf = settings.output.outputs.get(output)?;
        let planned = stage::plan(
            &settings.output.surfaces,
            output,
            settings.strobe,
            u.flow[3],
        );
        // The preview always shows the show in the surfaces, even while the
        // projector shows a pattern: that is what is being placed. With the
        // projectors running, their pictures (drawn this frame) are reused.
        let views = match frame_views {
            Some(v) => v.clone(),
            None => {
                let backgrounds: &[(u32, u32)] = if conf.black_outside { &[] } else { &[size] };
                Projector::render_views(
                    r,
                    encoder,
                    ViewSet::Now,
                    &[(&planned, size)],
                    backgrounds,
                    size.1,
                    u,
                    show,
                )
            }
        };
        let main_size = r.view_size(MAIN_VIEW).unwrap_or(size);
        let shown = if conf.showing == OutputPattern::Grid {
            Pattern::Grid
        } else {
            Pattern::Show
        };
        self.projector.as_mut()?.compose(
            r.device(),
            r.queue(),
            encoder,
            &target,
            size,
            &planned,
            Some(&views),
            main_size,
            shown,
            conf.black_outside,
            30,
            None,
        );
        Some(target)
    }
}

/// The projector's window, on its own display.
struct OutputWindow {
    /// Which output of the settings it is.
    index: usize,
    window: Arc<Window>,
    target: WindowTarget,
    display: String,
    projector: Projector,
}

struct App {
    args: Args,
    audio: AudioIn,
    analyzer: Analyzer,
    flow: Flow,
    /// How fast the visuals' clocks run: slower through a breakdown, as
    /// if the room held its breath (1 normal).
    pace: f32,
    /// Reused every frame so draining the ring does not allocate.
    scratch: Vec<f32>,
    start: Instant,
    last_frame: Instant,
    /// The window is fully hidden (minimised, or covered by other windows).
    hidden: bool,
    heard_anything: bool,
    warned_silent: bool,
    /// The data overlay stays on (H); otherwise it shows while the mouse moves.
    hud: bool,
    /// When the mouse last moved over the window.
    last_mouse: Option<Instant>,
    /// How present the wet floor is right now, faded slowly.
    floor: f32,
    lightning: Lightning,
    burst: Burst,
    /// Runs the beat ahead of what was heard, so visuals land with the sound.
    clock: BeatClock,
    director: Director,
    /// The look last announced in the terminal.
    announced: Option<Look>,
    strobe: StrobeLimiter,
    /// Frames drawn and seconds spent drawing them, for --bench.
    bench: (u32, f32),
    stats: Stats,
    /// Open while --record is on.
    recorder: Option<Recorder>,
    recorded_samples: u64,
    marks: u32,
    /// How many audio device problems have been printed already.
    seen_audio_errors: usize,
    /// When a lost input was last retried; `None` while audio is flowing.
    last_reconnect: Option<Instant>,
    /// Keeps the display awake for as long as it lives.
    _awake: Option<KeepAwake>,
    /// A message on screen, and when it appeared.
    label: Option<(String, Instant)>,
    /// The screen full-screen mode uses; `None` means the one the window is on.
    monitor: Option<MonitorHandle>,
    // Created once the event loop is running; `None` until then.
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    ui: Option<Ui>,
    state: AppState,
    /// What the menu sets; applied to the parts of the program as it changes.
    settings: Settings,
    saver: Saver,
    /// Slow to ask for, so looked up now and then: inputs, displays, venues.
    inputs: Vec<String>,
    displays: Vec<String>,
    /// The venue files: those in the venue folder, then the ones opened
    /// from elsewhere lately.
    venues: Vec<std::path::PathBuf>,
    /// Each venue file as it is on disk: to show what it holds and to tell
    /// whether the set-up now differs from the one in use.
    venue_cache: HashMap<std::path::PathBuf, Settings>,
    /// The Mac's Open or Save panel, while one is open.
    file_panel: Option<noctiluca::files::Panel>,
    lists_at: Option<Instant>,
    /// A message for the menu, and when it came.
    message: Option<(String, Instant)>,
    /// The latest analysis, for the menu's meters.
    features: Features,
    fps: f32,
    modifiers: ModifiersState,
    /// The control window is showing the show full screen (not the menu).
    showing_here: bool,
    /// One window per output with a display connected, in no set order.
    outputs: Vec<OutputWindow>,
    /// Projector displays that could not be opened: not tried again until
    /// the displays chosen change.
    output_failed: Vec<String>,
    /// Connected displays by name, looked at every second.
    monitors: Vec<(String, MonitorHandle)>,
    monitors_at: Option<Instant>,
    /// How tall the projector's pictures are drawn right now.
    governor: Governor,
    /// Shows the main picture in the control window while a projector runs.
    control_blit: Option<Compositor>,
    output_preview: OutputPreview,
    /// The pictures the projectors were given this frame: the previews
    /// reuse them instead of drawing the same views again at another size.
    frame_views: Option<Views>,
    /// The input at the chosen sensitivity, for the analysis.
    gained: Vec<f32>,
    /// The set's last twenty minutes, for the Live page.
    timeline: noctiluca::timeline::Timeline,
    /// --play: the show starts once the screens are there (or soon anyway).
    play_at: Option<Instant>,
    /// Network screens: the ones heard of, the links to those used as outputs,
    /// and what installing on each box found out (by address).
    discovery: Option<noctiluca::net::discovery::Discovery>,
    net: noctiluca::net::outputs::NetOutputs,
    boxes: Arc<Mutex<HashMap<String, BoxJob>>>,
    /// DMX lights: the interface's sender, the engine, each light now.
    dmx: Option<noctiluca::dmx::DmxOut>,
    light_engine: noctiluca::lights::Engine,
    lights_now: Vec<noctiluca::lights::LightOut>,
    lights_at: Instant,
    dmx_ports: Vec<String>,
    /// (frames sent, when) a second ago, for the rate shown.
    dmx_count: (u64, Instant, f32),
}

/// What installing on one box is doing, or found out.
#[derive(Clone, Debug, Default)]
struct BoxJob {
    state: Option<noctiluca::net::adb::BoxState>,
    busy: Option<String>,
    error: Option<String>,
}

/// Three keystoned surfaces side by side, as a projector aimed at three
/// panels from below might need: for pictures of the output.
fn demo_layout() -> noctiluca::settings::OutputSetup {
    use noctiluca::settings::{ComplementStyle, Content, OutputSetup, Surface};
    let surface = |name: &str, corners, content| Surface {
        name: name.into(),
        corners,
        content,
        style: ComplementStyle::MirrorMotion,
        feather_px: 6.0,
        ..Surface::default()
    };
    OutputSetup {
        surfaces: vec![
            surface(
                "Left",
                [[0.03, 0.17], [0.30, 0.09], [0.29, 0.88], [0.04, 0.80]],
                Content::Complement,
            ),
            surface(
                "Centre",
                [
                    [0.355, 0.075],
                    [0.664, 0.072],
                    [0.672, 0.915],
                    [0.345, 0.92],
                ],
                Content::Main,
            ),
            surface(
                "Right",
                [[0.70, 0.09], [0.97, 0.17], [0.96, 0.80], [0.71, 0.88]],
                Content::Complement,
            ),
        ],
        ..OutputSetup::default()
    }
}

/// Full screen on the window's own display, without a new Space (macOS's
/// "simple" full screen). Native full screen opens a Space, and with
/// "Displays have separate Spaces" off in macOS that Space blacks out every
/// other display: turning the projector on would hide the menu on the Mac
/// and leave nothing but black.
fn set_full_screen(window: &Window, on: bool) {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowExtMacOS;
        window.set_simple_fullscreen(on);
    }
    #[cfg(not(target_os = "macos"))]
    window.set_fullscreen(
        on.then(|| winit::window::Fullscreen::Borderless(window.current_monitor())),
    );
}

fn is_full_screen(window: &Window) -> bool {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowExtMacOS;
        window.simple_fullscreen()
    }
    #[cfg(not(target_os = "macos"))]
    window.fullscreen().is_some()
}

fn pattern_of(p: OutputPattern) -> Pattern {
    match p {
        OutputPattern::Show => Pattern::Show,
        OutputPattern::Grid => Pattern::Grid,
        OutputPattern::White => Pattern::White,
        OutputPattern::Black => Pattern::Black,
    }
}

/// The director's options as the settings have them.
fn director_options(s: &Settings) -> DirectorOptions {
    DirectorOptions {
        themes: s.themes.clone(),
        follow_vibe: s.follow_vibe,
        variety: s.variety,
    }
}

impl App {
    fn new(args: Args, audio: AudioIn, settings: Settings, message: Option<String>) -> Self {
        let now = Instant::now();
        // Read these before `audio` and `args` are moved into the struct below.
        let connected = audio.connected;
        let audio_label = audio.label.clone();
        let args_snapshot_free = args.snapshot.is_none();
        let dmx_port = settings.lighting.port.clone();
        let args_settings_snapshot = args.snapshot_page.as_deref() == Some("settings");
        let flow = args
            .flow_start
            .map_or_else(Flow::default, Flow::starting_at);
        let recorder = args.record.as_deref().and_then(|path| {
            match Recorder::create(path, audio.sample_rate) {
                Ok(rec) => {
                    println!("recording to {} (press M to mark a moment)", path.display());
                    Some(rec)
                }
                Err(e) => {
                    eprintln!("not recording: {e:#}");
                    None
                }
            }
        });
        // Without a seed, use the clock so every night is different.
        let seed = args.seed.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
        let mut director = Director::new(seed);
        let random = director.look();
        director = director.with_look(Look {
            bg: args.bg.unwrap_or(random.bg),
            pattern: args.pattern.unwrap_or(random.pattern),
            palette: args.palette.unwrap_or(random.palette),
            post: args.post.unwrap_or(random.post),
            object: args.object.unwrap_or(random.object),
            material: args.material.unwrap_or(random.material),
            atmos: args.atmos.unwrap_or(random.atmos),
        });
        if args.lock {
            director.toggle_lock();
        }
        director.set_options(director_options(&settings));
        let strobe_on = settings.strobe && !args.no_strobe;
        let quality_ceiling = settings.quality.view_height();
        let mut strobe =
            StrobeLimiter::new(MAX_FLASHES_PER_SEC, args.strobe_max.min(settings.max_flash));
        strobe.set_enabled(strobe_on);
        // Snapshots and benchmarks have no menu: straight to the show.
        let state = if args.snapshot.is_some() {
            AppState {
                running: true,
                show_here: true,
                ..AppState::default()
            }
        } else {
            AppState::default()
        };
        Self {
            analyzer: Analyzer::new(audio.sample_rate),
            audio,
            hud: args.snapshot_hud,
            last_mouse: start_overlay(&args).then_some(now),
            floor: 0.0,
            burst: Burst::new(),
            clock: BeatClock::new(settings.sync_offset_ms),
            lightning: {
                let mut l = Lightning::new();
                l.set_enabled(strobe_on);
                l
            },
            director,
            announced: None,
            strobe,
            bench: (0, 0.0),
            stats: Stats::default(),
            recorder,
            recorded_samples: 0,
            marks: 0,
            seen_audio_errors: 0,
            last_reconnect: None,
            _awake: args.snapshot.is_none().then(KeepAwake::start).flatten(),
            // Which input is listening, for a few seconds (not on saved pictures).
            label: start_overlay(&args).then(|| (format!("input: {}", audio_label), now)),
            monitor: None,
            args,
            flow,
            pace: 1.0,
            scratch: vec![0.0; 4096],
            start: now,
            last_frame: now,
            hidden: false,
            heard_anything: false,
            // Idle mode already said there is no audio; do not say it twice.
            warned_silent: !connected,
            window: None,
            renderer: None,
            ui: None,
            state,
            settings,
            saver: Saver::new(),
            inputs: Vec::new(),
            displays: Vec::new(),
            venues: Vec::new(),
            venue_cache: HashMap::new(),
            file_panel: None,
            lists_at: None,
            message: message.map(|m| (m, now)),
            features: Features::default(),
            fps: 0.0,
            modifiers: ModifiersState::default(),
            showing_here: false,
            outputs: Vec::new(),
            output_failed: Vec::new(),
            monitors: Vec::new(),
            monitors_at: None,
            governor: Governor::new(quality_ceiling),
            control_blit: None,
            output_preview: OutputPreview::default(),
            frame_views: None,
            play_at: None,
            timeline: Default::default(),
            gained: Vec::new(),
            // Listening only: snapshots of the settings show what is found too
            // (they never connect to anything).
            discovery: (args_snapshot_free || args_settings_snapshot).then(noctiluca::net::discovery::Discovery::start),
            net: Default::default(),
            boxes: Default::default(),
            dmx: args_snapshot_free.then(|| noctiluca::dmx::DmxOut::start(dmx_port)),
            light_engine: Default::default(),
            lights_now: Vec::new(),
            lights_at: now,
            dmx_ports: Vec::new(),
            dmx_count: (0, now, 0.0),
        }
    }

    /// Moves everything forward by one frame and returns what to draw.
    fn tick(&mut self) -> Option<(Uniforms, DirectorOut)> {
        let (width, height) = self.renderer.as_ref()?.size();
        let now = Instant::now();
        let real_dt = (now - self.last_frame).as_secs_f32();
        // After a stall (a dragged window, a sleeping laptop) pretend only a
        // tenth of a second passed, so nothing jumps.
        let dt = real_dt.min(0.1);
        self.last_frame = now;
        if self.args.stats && self.stats.frame(real_dt) {
            for n in &self.net.list {
                let st = n.stream.status();
                println!(
                    "  {}: {} · {:.0} ms ahead · rtt {:.1} ms · shown {} late {} dropped {} (here {}) · decode {:.1} ms{}",
                    n.name,
                    if st.connected { "streaming" } else { "not connected" },
                    st.latency_ms,
                    st.round_trip_ms,
                    st.stats.shown,
                    st.stats.late,
                    st.stats.dropped,
                    st.dropped_here,
                    st.stats.decode_ms,
                    st.error.map(|e| format!(" · {e}")).unwrap_or_default()
                );
            }
        }
        let time = (now - self.start).as_secs_f32();

        let mut got_audio = false;
        loop {
            let n = self.audio.rx.pop_slice(&mut self.scratch);
            if n == 0 {
                break;
            }
            got_audio = true;
            self.heard_anything |= self.scratch[..n].iter().any(|s| *s != 0.0);
            // The recording keeps the sound as it came; the analysis hears it
            // at the sensitivity chosen (a quiet room, a loud booth).
            let gain = self.settings.input_gain;
            if (gain - 1.0).abs() > 1e-3 {
                self.gained.clear();
                self.gained.extend(self.scratch[..n].iter().map(|s| (s * gain).clamp(-1.0, 1.0)));
                self.analyzer.push(&self.gained);
            } else {
                self.analyzer.push(&self.scratch[..n]);
            }
            if let Some(rec) = &mut self.recorder {
                self.recorded_samples += n as u64;
                if let Err(e) = rec.write(&self.scratch[..n]) {
                    eprintln!("recording stopped: {e:#}");
                    self.recorder = None;
                }
            }
        }
        let errors = self.audio.stream_errors();
        if errors != self.seen_audio_errors {
            self.seen_audio_errors = errors;
            eprintln!(
                "audio: \"{}\" has reported {errors} problem(s) so far",
                self.audio.label
            );
        }
        if !got_audio {
            // Tell the analyser time is passing, so a dead input fades to
            // silence instead of freezing the last beat on screen.
            self.analyzer.idle(dt);
            self.reconnect_if_lost();
        }
        if !self.heard_anything && !self.warned_silent && time > 3.0 {
            self.warned_silent = true;
            eprintln!(
                "No sound is arriving from \"{}\". If this is a microphone, check \
                 System Settings > Privacy & Security > Microphone.",
                self.audio.label
            );
        }

        let heard = self.analyzer.features();
        let features = self.clock.update(&heard, dt);
        self.features = features;
        if real_dt > 0.0 {
            self.fps += (1.0 / real_dt - self.fps) * 0.05;
        }
        self.saver.tick(now);
        if !self.outputs.is_empty() && self.state.running {
            self.governor.frame(real_dt * 1000.0);
        }
        let show = self.director.update(&features, dt);
        {
            let effect = self.light_engine.effect();
            let effect = noctiluca::lights::Effect::ALL.iter().position(|e| *e == effect).unwrap_or(0) as u8;
            let section = match features.mood {
                Mood::Silence => 0,
                Mood::Groove => 1,
                Mood::Build => 2,
                Mood::Drop => 3,
                Mood::Breakdown => 4,
            };
            self.timeline.record(dt, noctiluca::timeline::Sample { section, theme: self.director.theme() as u8, effect });
        }
        // A drop, or the first frames of a cross-fade: the picture is changing anyway.
        let covered = features.drop || (show.from != show.to && show.mix < 0.05);
        // Slow motion through a breakdown, eased in and out.
        let target = if features.mood == Mood::Breakdown { BREAKDOWN_PACE } else { 1.0 };
        self.pace += (target - self.pace) * (1.0 - (-dt / PACE_EASE_SECS).exp());
        self.flow.advance(&features, dt * self.pace, covered);
        if self.announced != Some(show.to) {
            self.announced = Some(show.to);
            let l = show.to;
            println!(
                "{time:6.1}s  beat {:<4} {:<9} {:<14} [{}] {} + {}, palette {}, effect {}, {}, {}",
                features.beat_count,
                mood_name(features.mood),
                features.vibe.name(),
                THEMES[show.theme].name,
                BACKGROUNDS[l.bg],
                PATTERNS[l.pattern],
                PALETTE_NAMES[l.palette],
                POST_NAMES[l.post],
                if l.object == 0 {
                    "no object".to_string()
                } else {
                    format!("{} {}", MATERIALS[l.material], OBJECTS[l.object])
                },
                if l.atmos == 0 {
                    "no atmosphere"
                } else {
                    ATMOSPHERES[l.atmos]
                },
            );
        }
        let strobe = self.strobe.update(show.flash, dt);
        let storm = show.to.wrapped().atmos == 1 || show.from.wrapped().atmos == 1;
        let (strike, bolt) = self.lightning.update(&features, storm, dt);
        let (burst, shake) = self.burst.update(&features, dt);
        // A message stays for a few seconds and fades out over the last one.
        let label = self.label.as_ref().and_then(|(text, since)| {
            let age = since.elapsed().as_secs_f32();
            (age < LABEL_SECS).then(|| (text.as_str(), (LABEL_SECS - age).min(1.0)))
        });
        let frame = FrameInfo {
            width,
            height,
            time,
            dt,
            hud: self.hud_opacity(),
            locked: self.director.locked(),
            show,
            strobe,
            label,
            events: [strike, bolt, burst, shake],
        };
        let mut uniforms = Uniforms::build(&frame, &features, &self.flow);
        // The floor has its own slow fade, longer than the look's cross-fade.
        self.floor = fade_toward(self.floor, uniforms.post[2][0], dt, FLOOR_FADE_SECS);
        uniforms.post[2][0] = self.floor;
        Some((uniforms, show))
    }

    /// How visible the data overlay is: fully while pinned with H or for a
    /// few seconds after the mouse moves, then fading out, so the screen is
    /// all visuals when nobody touches anything.
    fn hud_opacity(&self) -> f32 {
        hud_opacity(self.hud, self.last_mouse.map(|t| t.elapsed().as_secs_f32()))
    }

    /// Moves the visuals earlier (`]`) or later (`[`) against the sound, for
    /// this room and screen; remembered for next time. The overlay comes up
    /// too, so its beat dots show whether the kick and the flash now meet.
    fn nudge_sync(&mut self, steps: i32) {
        let mut s = self.settings.clone();
        s.sync_offset_ms = (s.sync_offset_ms + steps as f32 * noctiluca::sync::OFFSET_STEP_MS)
            .clamp(-300.0, 300.0);
        self.apply_settings(s);
        let offset = self.clock.offset_ms();
        self.last_mouse = Some(Instant::now());
        let lead = (self.clock.lead_secs() * 1000.0).round();
        self.show_message(format!("SYNC {offset:+.0} MS, {lead:.0} MS EARLY"));
    }

    fn show_message(&mut self, text: String) {
        println!("{text}");
        self.label = Some((text, Instant::now()));
    }

    /// Switches to the next input device and says which one on screen.
    fn next_input(&mut self) {
        if let Some(file) = &self.args.file {
            let name = file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            self.show_message(format!("playing a file: {name}"));
            return;
        }
        let names = audio::list_inputs();
        if names.is_empty() {
            self.show_message("no input devices".into());
            return;
        }
        let current = names.iter().position(|n| *n == self.audio.label);
        let next = current.map_or(0, |i| (i + 1) % names.len());
        let name = names[next].clone();
        if self.switch_input(Some(name.clone())) {
            self.show_message(format!("input {}/{}: {name}", next + 1, names.len()));
        }
    }

    /// Listens to input `name` from now on (`None`: the best one there is),
    /// and remembers the choice. False if it could not be opened.
    fn switch_input(&mut self, name: Option<String>) -> bool {
        if self.args.file.is_some() {
            self.notify("Playing a file: the input cannot change".into());
            return false;
        }
        match audio::open_input(name.as_deref()) {
            Ok(audio) => {
                if audio.sample_rate != self.audio.sample_rate && self.recorder.take().is_some() {
                    eprintln!("recording stopped: the new input runs at a different sample rate");
                }
                // A new source: start the analysis afresh, and reconnect to
                // this device (not the old one) if it drops out.
                self.analyzer = Analyzer::new(audio.sample_rate);
                println!("listening to: {} ({} Hz)", audio.label, audio.sample_rate);
                self.audio = audio;
                self.args.device = name.clone();
                self.seen_audio_errors = 0;
                self.last_reconnect = None;
                let mut s = self.settings.clone();
                s.input_device = name;
                self.apply_settings(s);
                true
            }
            Err(e) => {
                let shown = name.unwrap_or_else(|| "an input".into());
                eprintln!("cannot open {shown}: {e:#}");
                self.notify(format!("Cannot open {shown}"));
                false
            }
        }
    }

    /// A message in the menu (and the terminal).
    fn notify(&mut self, text: String) {
        println!("{text}");
        self.message = Some((text, Instant::now()));
    }

    /// Makes the settings `new` the program's own: each part that changed is
    /// passed on, and the settings are saved shortly after.
    fn apply_settings(&mut self, new: Settings) {
        let new = new.validated();
        if new == self.settings {
            return;
        }
        let old = std::mem::replace(&mut self.settings, new);
        let s = &self.settings;
        if s.sync_offset_ms != old.sync_offset_ms {
            self.clock.set_offset(s.sync_offset_ms);
        }
        let strobe_on = s.strobe && !self.args.no_strobe;
        self.strobe.set_enabled(strobe_on);
        self.lightning.set_enabled(strobe_on);
        self.strobe
            .set_max_intensity(self.args.strobe_max.min(s.max_flash));
        self.governor.set_ceiling(s.quality.view_height());
        let displays = |s: &Settings| {
            s.output
                .outputs
                .iter()
                .map(|o| o.display.clone())
                .collect::<Vec<_>>()
        };
        if displays(s) != displays(&old) {
            self.output_failed.clear();
        }
        if s.lighting.port != old.lighting.port
            && let Some(d) = &self.dmx
        {
            d.set_port(s.lighting.port.clone());
        }
        if s.themes != old.themes || s.follow_vibe != old.follow_vibe || s.variety != old.variety {
            self.director.set_options(director_options(s));
        }
        self.saver.changed(&self.settings, Instant::now());
    }

    /// Carries out what a page asked for.
    fn act(&mut self, action: UiAction, event_loop: &ActiveEventLoop) {
        match action {
            UiAction::App(a) => {
                self.state.apply(a, self.has_output());
                self.follow_state();
            }
            UiAction::Quit => self.quit(event_loop),
            UiAction::SelectInput(name) => {
                self.switch_input(name);
            }
            UiAction::LoadVenue(path) => self.load_venue(&path, event_loop),
            UiAction::SaveVenue => match self.settings.venue_file.clone().filter(|p| p.exists()) {
                Some(path) => self.save_venue_to(&path),
                None => self.act(UiAction::SaveVenueAs, event_loop),
            },
            UiAction::SaveVenueTo(path) => self.save_venue_to(&path),
            UiAction::SaveVenueAs => {
                if self.file_panel.is_none() {
                    let dir = self.venue_folder();
                    let name = self.settings.last_venue.clone().unwrap_or_default();
                    self.file_panel = Some(noctiluca::files::save_venue_as(&dir, &name));
                }
            }
            UiAction::OpenVenue => {
                if self.file_panel.is_none() {
                    self.file_panel = Some(noctiluca::files::open_venue(&self.venue_folder()));
                }
            }
            UiAction::RevealVenue(path) => noctiluca::files::reveal(&path),
            UiAction::RenameVenue(path, to) => match settings::rename_venue_file(&path, &to) {
                Ok(dest) => {
                    let mut s = self.settings.clone();
                    for r in &mut s.recent_venues {
                        if *r == path {
                            *r = dest.clone();
                        }
                    }
                    if s.venue_file.as_ref() == Some(&path) {
                        s.set_venue_file(Some(dest.clone()));
                    }
                    self.apply_settings(s);
                    self.reload_venues();
                    self.notify(format!("Renamed to “{}”", settings::venue_name(&dest)));
                }
                Err(e) => self.notify(format!("{e:#}")),
            },
            UiAction::TrashVenue(path) => match settings::trash_venue_file(&path) {
                Ok(()) => {
                    let mut s = self.settings.clone();
                    s.recent_venues.retain(|r| *r != path);
                    if s.venue_file.as_ref() == Some(&path) {
                        s.venue_file = None;
                        s.last_venue = None;
                    }
                    self.apply_settings(s);
                    self.reload_venues();
                    self.notify(format!("Moved “{}” to the Trash", settings::venue_name(&path)));
                }
                Err(e) => self.notify(format!("{e:#}")),
            },
            UiAction::NetInstall(ip) => self.box_job(ip, "Installing…", noctiluca::net::adb::install),
            UiAction::NetOpen(ip) => self.box_job(ip, "Opening…", noctiluca::net::adb::open),
            UiAction::NetUse(name) => self.use_network_screen(&name),
            UiAction::LightIdentify(i) => self.light_engine.identify(i),
            UiAction::NextLook => self.director.next(),
            UiAction::HoldLook => {
                let held = self.director.toggle_lock();
                self.notify(if held { "Look held: nothing changes on its own".into() } else { "Look released".into() });
            }
            UiAction::NextPalette => self.director.next_palette(),
        }
    }

    /// A projector window is open.
    fn has_output(&self) -> bool {
        !self.outputs.is_empty() || self.net.active()
    }

    /// Makes the network screen `name` an output's display: the current
    /// mapping tab's output if it has none, else the first without one, else
    /// a new output. Nothing changes if it is one already.
    fn use_network_screen(&mut self, name: &str) {
        if self.settings.output.outputs.iter().any(|o| o.display.as_deref() == Some(settings::network_display(name).as_str())) {
            return;
        }
        // The current mapping tab's output if it has no display, else
        // the first without one, else a new output.
        let display = settings::network_display(name);
        let mut s = self.settings.clone();
        let tab = self.ui.as_ref().map_or(0, |u| u.pages.mapping.output);
        let free = |s: &Settings, k: usize| s.output.outputs.get(k).is_some_and(|o| o.display.is_none());
        let k = if free(&s, tab) {
            Some(tab)
        } else {
            (0..s.output.outputs.len()).find(|&k| free(&s, k))
        };
        let k = match k {
            Some(k) => k,
            None if s.output.outputs.len() < settings::MAX_OUTPUTS => {
                let n = s.output.outputs.len();
                s.output.outputs.push(settings::OutputConf { name: format!("Output {}", n + 1), ..Default::default() });
                n
            }
            None => {
                self.notify(format!("All {} outputs have a display: free one in Mapping first", settings::MAX_OUTPUTS));
                return;
            }
        };
        s.output.outputs[k].display = Some(display);
        // An output with nothing on it would stay black: give it the show,
        // over the whole screen.
        if !s.output.surfaces.iter().any(|x| x.output == k) && s.output.surfaces.len() < settings::MAX_SURFACES {
            s.output.surfaces.push(settings::Surface {
                name: name.to_string(),
                output: k,
                content: settings::Content::Same,
                ..Default::default()
            });
        }
        self.apply_settings(s);
        self.monitors_at = None;
        self.notify(format!("{name} is output {}", k + 1));
    }

    /// Reads the venue files again: the venue folder's, then the ones
    /// opened from elsewhere lately; and what each holds.
    fn reload_venues(&mut self) {
        let library = settings::venue_library();
        let mut list = settings::venues_in(&library);
        for p in &self.settings.recent_venues {
            if !list.contains(p) && p.is_file() {
                list.push(p.clone());
            }
        }
        self.venue_cache = list.iter().filter_map(|p| Some((p.clone(), settings::load_venue_file(p).ok()?.venue_preset()))).collect();
        self.venues = list;
    }

    /// Where the Open and Save panels start: the folder of the venue in
    /// use, else the venue folder (made if it is not there yet).
    fn venue_folder(&self) -> std::path::PathBuf {
        let library = settings::venue_library();
        let _ = std::fs::create_dir_all(&library);
        self.settings.venue_file.as_deref().and_then(|p| p.parent()).filter(|d| d.is_dir()).map(|d| d.to_path_buf()).unwrap_or(library)
    }

    /// Loads a venue file over the set-up now (the input device stays).
    fn load_venue(&mut self, path: &std::path::Path, _event_loop: &ActiveEventLoop) {
        let name = settings::venue_name(path);
        match settings::load_venue_file(path) {
            Ok(venue) => {
                let s = self.settings.with_venue(venue, path);
                let input = s.input_device.clone();
                self.apply_settings(s);
                if input != self.args.device && self.args.file.is_none() {
                    self.switch_input(input);
                }
                self.reload_venues();
                self.notify(format!("Opened “{name}”"));
            }
            Err(e) => self.notify(format!("Cannot open “{name}”: {e:#}")),
        }
    }

    /// Saves the set-up into a venue file, which becomes the one in use.
    fn save_venue_to(&mut self, path: &std::path::Path) {
        match settings::save_venue_file(path, &self.settings) {
            Ok(dest) => {
                let mut s = self.settings.clone();
                s.set_venue_file(Some(dest.clone()));
                self.apply_settings(s);
                self.reload_venues();
                self.notify(format!("Saved “{}”", settings::venue_name(&dest)));
            }
            Err(e) => self.notify(format!("Cannot save: {e:#}")),
        }
    }

    /// A file dropped on the window, or chosen in the Open panel: opened
    /// if it is a venue, asking first over unsaved changes.
    fn open_venue_file(&mut self, path: std::path::PathBuf, event_loop: &ActiveEventLoop) {
        if !settings::is_venue_file(&path) && settings::load_venue_file(&path).is_err() {
            self.notify(format!("“{}” is not a venue file (they end in _nocvenue.json)", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
            return;
        }
        let live = self.live();
        match &mut self.ui {
            Some(ui) => {
                let mut actions = Vec::new();
                ui::venues::ask_load(path, &live, &mut ui.pages.venues, &mut actions);
                if !actions.is_empty() || !matches!(self.state.page, noctiluca::app_state::Page::Venues) {
                    // Show the page, where the question (if any) is asked.
                    self.state.page = noctiluca::app_state::Page::Venues;
                }
                for a in actions {
                    self.act(a, event_loop);
                }
            }
            None => self.load_venue(&path, event_loop),
        }
    }

    /// The answer of an Open or Save panel, once it closes.
    fn poll_file_panel(&mut self, event_loop: &ActiveEventLoop) {
        let Some(answer) = self.file_panel.as_ref().and_then(|p| p.poll()) else { return };
        let purpose = self.file_panel.take().map(|p| p.purpose);
        let Some(path) = answer else { return };
        match purpose {
            Some(noctiluca::files::Purpose::OpenVenue) => self.open_venue_file(path, event_loop),
            Some(noctiluca::files::Purpose::SaveVenueAs) => self.save_venue_to(&path),
            None => {}
        }
    }

    /// Something must be drawn every frame even with the Mac's window hidden.
    fn feeding_screens(&self) -> bool {
        !self.outputs.is_empty() || self.net.active()
    }

    /// Runs an install step for the box at `ip` in the background, and
    /// checks where it stands afterwards.
    fn box_job(&mut self, ip: String, busy: &str, step: fn(std::net::IpAddr) -> anyhow::Result<()>) {
        let Ok(addr) = ip.parse::<std::net::IpAddr>() else { return };
        let boxes = self.boxes.clone();
        {
            let mut b = boxes.lock().unwrap();
            let job = b.entry(ip.clone()).or_default();
            if job.busy.is_some() {
                return;
            }
            job.busy = Some(busy.to_string());
            job.error = None;
        }
        std::thread::spawn(move || {
            use noctiluca::net::adb::{self, BoxState};
            // Where it stands first: the TV may need to allow this Mac.
            let before = adb::check(addr);
            let result = match &before {
                Ok(BoxState::NoDebugging) | Ok(BoxState::WaitingForAllow) => Ok(()),
                Ok(_) => step(addr),
                Err(e) => Err(anyhow::anyhow!("{e:#}")),
            };
            let after = adb::check(addr).ok();
            let mut b = boxes.lock().unwrap();
            let job = b.entry(ip).or_default();
            job.busy = None;
            job.state = after.or(before.ok());
            job.error = result.err().map(|e| format!("{e:#}"));
        });
    }

    /// Opens or closes the projectors' windows to match the settings and the
    /// displays that are connected: one window per output with its display
    /// there. Closing comes first, so a display handed from one output to
    /// another is free before it is opened again.
    fn check_output(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .monitors_at
            .is_some_and(|t| t.elapsed() < CHECK_DISPLAYS)
            || self.args.snapshot.is_some()
        {
            return;
        }
        self.monitors_at = Some(Instant::now());
        if let Some(d) = &self.discovery {
            let found = d.found();
            self.net.reconcile(&self.settings, &found);
        }
        self.monitors = event_loop
            .available_monitors()
            .filter_map(|m| Some((m.name()?, m)))
            .collect();
        let names: Vec<String> = self.monitors.iter().map(|(n, _)| n.clone()).collect();
        let control = self
            .window
            .as_ref()
            .and_then(|w| w.current_monitor())
            .and_then(|m| m.name());
        let had_any = !self.outputs.is_empty();
        // Windows of outputs that were removed.
        let count = self.settings.output.outputs.len();
        self.outputs.retain(|o| o.index < count);
        let mut opens = Vec::new();
        for k in 0..count {
            let open = self
                .outputs
                .iter()
                .find(|o| o.index == k)
                .map(|o| o.display.clone());
            let wanted = self.settings.output.outputs[k].display.clone();
            match reconcile(
                wanted.as_deref(),
                &names,
                control.as_deref(),
                open.as_deref(),
            ) {
                OutputChange::Keep => {}
                OutputChange::Open(name) => opens.push((k, name)),
                OutputChange::Close(reason) => {
                    self.outputs.retain(|o| o.index != k);
                    let name = &self.settings.output.outputs[k].name;
                    match reason {
                        Reason::Unplugged => {
                            self.notify(format!("{name}: display unplugged, the show goes on"))
                        }
                        Reason::SameAsControl => self
                            .notify(format!("{name}: that display is the one this window is on")),
                        Reason::TurnedOff | Reason::Moved => {}
                    }
                }
            }
        }
        for (k, name) in opens {
            if self.output_failed.contains(&name) || self.outputs.iter().any(|o| o.display == name)
            {
                continue;
            }
            match self.open_output(event_loop, &name, k) {
                Ok(out) => {
                    self.outputs.push(out);
                    self.notify(format!(
                        "{} on {name}",
                        self.settings.output.outputs[k].name
                    ));
                    // A show already full screen here moves to the projector.
                    if self.state.running && self.state.show_here {
                        self.state.show_here = false;
                        self.follow_state();
                    }
                }
                Err(e) => {
                    self.notify(format!("Cannot open the output on {name}: {e:#}"));
                    self.output_failed.push(name);
                }
            }
        }
        // Only the projectors wait for their screens' refresh; the Mac's
        // window must not add a second wait to every frame.
        if had_any != !self.outputs.is_empty()
            && let Some(r) = &mut self.renderer
        {
            r.set_vsync(self.outputs.is_empty());
        }
    }

    fn open_output(
        &mut self,
        event_loop: &ActiveEventLoop,
        name: &str,
        index: usize,
    ) -> Result<OutputWindow> {
        let monitor = self
            .monitors
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m.clone())
            .ok_or_else(|| anyhow::anyhow!("no display called {name}"))?;
        let renderer = self
            .renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no renderer"))?;
        // A plain window placed on the projector's display, then made to
        // fill it (see set_full_screen: no new Space, the Mac's screen stays).
        let attrs = Window::default_attributes()
            .with_title("Noctiluca Output")
            .with_position(monitor.position())
            .with_inner_size(monitor.size())
            .with_decorations(false);
        let window = Arc::new(event_loop.create_window(attrs)?);
        let on_its_display =
            window.current_monitor().and_then(|m| m.name()).as_deref() == Some(name);
        let control = self
            .window
            .as_ref()
            .and_then(|w| w.current_monitor())
            .and_then(|m| m.name());
        if !on_its_display || control.as_deref() == Some(name) {
            anyhow::bail!("the window did not land on {name}");
        }
        set_full_screen(&window, true);
        window.set_cursor_visible(false);
        // The keyboard and mouse stay with the menu on the Mac.
        if let Some(control) = &self.window {
            control.focus_window();
        }
        let target = renderer.window_target(window.clone())?;
        let projector = Projector::new(renderer.device(), renderer.shader_dir(), target.format())?;
        Ok(OutputWindow {
            index,
            window,
            target,
            display: name.to_string(),
            projector,
        })
    }

    /// Events for a projector's window.
    fn output_event(&mut self, id: WindowId, event: WindowEvent, event_loop: &ActiveEventLoop) {
        let Some(at) = self.outputs.iter().position(|o| o.window.id() == id) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                // Closed by hand: that output is turned off, the show goes on.
                let k = self.outputs.remove(at).index;
                let mut s = self.settings.clone();
                if let Some(o) = s.output.outputs.get_mut(k) {
                    o.display = None;
                }
                self.apply_settings(s);
                if self.outputs.is_empty()
                    && let Some(r) = &mut self.renderer
                {
                    r.set_vsync(true);
                }
                self.notify("Output turned off".into());
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = &self.renderer {
                    self.outputs[at]
                        .target
                        .resize(r.device(), size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                // Esc on the projector: back to the menu on the Mac.
                self.act(UiAction::App(Action::Escape), event_loop);
                if let Some(control) = &self.window {
                    control.focus_window();
                }
            }
            _ => {}
        }
    }

    /// The time between frames: 60 a second, or 5 while nobody can see the
    /// window and there is no projector to feed.
    fn frame_interval(&self) -> Duration {
        if self.hidden && !self.feeding_screens() {
            HIDDEN_FRAME_INTERVAL
        } else {
            FRAME_INTERVAL
        }
    }

    /// One frame: the analysis moves on, the projector is drawn, and the
    /// Mac's window too unless it is hidden (drawing into a hidden window
    /// can make macOS wait up to a second, which would stall the projector).
    fn draw_frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some((uniforms, show)) = self.tick() else {
            return;
        };
        self.frame_views = None;
        self.update_lights(&uniforms);
        if !self.outputs.is_empty() {
            self.render_outputs(&uniforms, &show);
        }
        if self.net.active() {
            self.render_network(&uniforms, &show);
        }
        if self.feeding_screens() && self.hidden {
            return;
        }
        if self.state.control_shows_show() {
            if !self.outputs.is_empty() {
                self.draw_main_here();
            } else if let Some(renderer) = &mut self.renderer
                && let Err(e) = renderer.frame(&uniforms, &show)
            {
                eprintln!("render error: {e:#}");
            }
        } else {
            self.draw_menu(uniforms, &show, event_loop);
        }
    }

    /// Draws the network screens' pictures (ahead, by their delay) and sends
    /// the ones the GPU has finished.
    fn render_network(&mut self, uniforms: &Uniforms, show: &DirectorOut) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let mut u = *uniforms;
        u.misc[0] = 0.0;
        u.overlay[0] = 0.0;
        let mapping = self.state.page == noctiluca::app_state::Page::Mapping && !self.state.control_shows_show();
        let outlined = match (&self.ui, mapping) {
            (Some(ui), true) => Some(ui.pages.mapping.selected),
            _ => None,
        };
        let strobe = self.settings.strobe && !self.args.no_strobe;
        let echo = stage::echo_frames(self.fps, self.features.bpm);
        let mut encoder = renderer.device().create_command_encoder(&Default::default());
        self.net.render(
            renderer,
            &mut encoder,
            &self.settings,
            strobe,
            &u,
            show,
            self.governor.height(),
            self.state.running || mapping,
            outlined,
            echo,
        );
        renderer.queue().submit([encoder.finish()]);
        self.net.after_submit();
        let format = renderer.surface_format();
        self.net.deliver(renderer.device(), format);
    }

    /// Draws every projector's picture for this frame. The pictures are drawn
    /// once, big enough for all of them; each output then warps its own
    /// surfaces out of them.
    fn render_outputs(&mut self, uniforms: &Uniforms, show: &DirectorOut) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let mut u = *uniforms;
        u.misc[0] = 0.0;
        u.overlay[0] = 0.0;
        let s = &self.settings;
        let strobe = s.strobe && !self.args.no_strobe;
        // While the mapping page is open the projectors show the pictures even
        // before Start, with the selected surface outlined: mapping is done
        // live, against the wall itself.
        let mapping = self.state.page == noctiluca::app_state::Page::Mapping
            && !self.state.control_shows_show();
        let (tab, outlined) = match (&self.ui, mapping) {
            (Some(ui), true) => (
                Some(ui.pages.mapping.output),
                Some(ui.pages.mapping.selected),
            ),
            _ => (None, None),
        };
        let plans: Vec<(usize, Vec<stage::Planned>, (u32, u32))> = self
            .outputs
            .iter()
            .map(|o| {
                (
                    o.index,
                    stage::plan(&s.output.surfaces, o.index, strobe, uniforms.flow[3]),
                    o.target.size(),
                )
            })
            .collect();
        let pattern = |k: usize| {
            s.output
                .outputs
                .get(k)
                .map_or(Pattern::Show, |o| pattern_of(o.showing))
        };
        let black_outside = |k: usize| s.output.outputs.get(k).is_none_or(|o| o.black_outside);
        let mut encoder = renderer
            .device()
            .create_command_encoder(&Default::default());
        let wanted = self.state.running || mapping;
        let views =
            (wanted && plans.iter().any(|(k, ..)| pattern(*k) == Pattern::Show)).then(|| {
                let mut sizing: Vec<(&[stage::Planned], (u32, u32))> =
                    plans.iter().map(|(_, p, px)| (p.as_slice(), *px)).collect();
                // The output on the mapping page's tab may have no window: its
                // preview needs its pictures too.
                let extra = tab
                    .filter(|k| !plans.iter().any(|(j, ..)| j == k))
                    .map(|k| stage::plan(&s.output.surfaces, k, strobe, uniforms.flow[3]));
                if let Some(p) = &extra {
                    sizing.push((p.as_slice(), (1920, 1080)));
                }
                let backgrounds: Vec<(u32, u32)> = plans
                    .iter()
                    .filter(|(k, ..)| !black_outside(*k))
                    .map(|(.., px)| *px)
                    .collect();
                Projector::render_views(
                    renderer,
                    &mut encoder,
                    ViewSet::Now,
                    &sizing,
                    &backgrounds,
                    self.governor.height(),
                    &u,
                    show,
                )
            });
        let main_size = renderer.view_size(MAIN_VIEW).unwrap_or((1920, 1080));
        let echo = stage::echo_frames(self.fps, self.features.bpm);
        let mut frames = Vec::new();
        for (out, (k, planned, size)) in self.outputs.iter_mut().zip(&plans) {
            let Some(frame) = out.target.acquire(renderer.device()) else {
                continue;
            };
            let target = frame.texture.create_view(&Default::default());
            out.projector.compose(
                renderer.device(),
                renderer.queue(),
                &mut encoder,
                &target,
                *size,
                planned,
                views.as_ref(),
                main_size,
                pattern(*k),
                black_outside(*k),
                echo,
                outlined.filter(|i| s.output.surfaces.get(*i).is_some_and(|x| x.output == *k)),
            );
            frames.push(frame);
        }
        renderer.queue().submit([encoder.finish()]);
        for frame in frames {
            renderer.queue().present(frame);
        }
        self.frame_views = views;
    }

    /// While a projector runs, the control window's "show here" is the main
    /// picture already drawn for it, scaled to the window.
    fn draw_main_here(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        if self.control_blit.is_none() {
            self.control_blit = Compositor::new(
                renderer.device(),
                renderer.shader_dir(),
                renderer.surface_format(),
            )
            .ok();
        }
        let (Some(blit), Some(view)) = (&self.control_blit, renderer.view_texture(MAIN_VIEW))
        else {
            return;
        };
        let Some(frame) = renderer.acquire() else {
            return;
        };
        let mut encoder = renderer
            .device()
            .create_command_encoder(&Default::default());
        let target = frame.texture.create_view(&Default::default());
        // In the window's shape, not stretched.
        let aspect = self.frame_views.as_ref().map_or(0.0, |v| v.main_aspect);
        let full = SurfaceDraw::full(view, aspect);
        blit.draw(
            renderer.device(),
            renderer.queue(),
            &mut encoder,
            &target,
            renderer.surface_size(),
            &[full],
            Pattern::Show,
            None,
        );
        renderer.submit(encoder, frame);
    }

    /// Puts the control window in full screen while it shows the show, and
    /// back in a window for the menu.
    fn follow_state(&mut self) {
        let here = self.state.control_shows_show();
        if here == self.showing_here {
            return;
        }
        self.showing_here = here;
        if let Some(window) = &self.window {
            set_full_screen(window, here);
            window.set_cursor_visible(!here);
        }
    }

    fn quit(&mut self, event_loop: &ActiveEventLoop) {
        self.saver.flush();
        // The lights go dark rather than hold the last colour.
        if let Some(d) = &self.dmx {
            d.set(&[0; noctiluca::lights::UNIVERSE]);
            std::thread::sleep(Duration::from_millis(120));
        }
        event_loop.exit();
    }

    /// The lights for this frame: the engine follows the show (while it
    /// plays, or while the Lights page is open), tests take over, and the
    /// values go to the interface.
    fn update_lights(&mut self, uniforms: &Uniforms) {
        use noctiluca::lights::{self, LightOut, UNIVERSE};
        use noctiluca::ui::lights_page::LightTest;
        let now = Instant::now();
        let dt = (now - self.lights_at).as_secs_f32();
        self.lights_at = now;
        let (test, held, on_page) = match &self.ui {
            Some(ui) => (
                ui.pages.lights.test,
                ui.pages.lights.smoke_held,
                self.state.page == noctiluca::app_state::Page::Lights,
            ),
            None => (LightTest::None, false, false),
        };
        self.light_engine.smoke_held = held;
        let cfg = &self.settings.lighting;
        let mut show = lights::Show::from_uniforms(uniforms);
        if self.args.no_strobe || !self.settings.strobe {
            show.flash = 0.0;
        }
        let mut out = self.light_engine.update(&show, cfg, dt);
        let live = self.state.running || on_page;
        if !live {
            for o in &mut out {
                *o = LightOut { fog: o.fog, ..LightOut::default() };
            }
        }
        if let LightTest::Colour(i, c) = test
            && let Some(o) = out.get_mut(i)
        {
            *o = LightOut { rgb: c, level: 1.0, flash: 0.0, fog: 0.0 };
        }
        let mut universe = lights::universe(cfg, &out);
        if let LightTest::Channel(ch, v, hold) = test {
            universe = [0; UNIVERSE];
            if hold > 0 {
                universe[(hold as usize).clamp(1, UNIVERSE) - 1] = 255;
            }
            universe[(ch as usize).clamp(1, UNIVERSE) - 1] = v;
        }
        if let Some(d) = &self.dmx {
            d.set(&universe);
            let st = d.status();
            if now.duration_since(self.dmx_count.1) >= Duration::from_secs(1) {
                let secs = now.duration_since(self.dmx_count.1).as_secs_f32();
                let rate = st.frames.saturating_sub(self.dmx_count.0) as f32 / secs;
                self.dmx_count = (st.frames, now, if st.frames >= self.dmx_count.0 { rate } else { 0.0 });
            }
        }
        self.lights_now = out;
    }

    /// What the menu shows about the program right now.
    fn live(&self) -> Live {
        let f = &self.features;
        // 64 spectrum bins into 24 bars for the meter.
        let levels = (0..24)
            .map(|i| {
                let (a, b) = (i * 64 / 24, ((i + 1) * 64 / 24).max(i * 64 / 24 + 1));
                f.spectrum[a..b].iter().copied().fold(0.0, f32::max) * f.level.max(0.05).sqrt()
            })
            .collect();
        Live {
            input: self.audio.label.clone(),
            inputs: self.inputs.clone(),
            levels,
            kick_found: f.bpm > 0.0 && f.confidence > 0.3,
            bpm: f.bpm,
            locked: f.confidence > 0.5,
            vibe: f.vibe,
            fps: self.fps,
            displays: {
                // Displays, then the network screens with the app open.
                let mut d = self.displays.clone();
                if let Some(disc) = &self.discovery {
                    d.extend(disc.found().into_iter().filter(|f| f.app_open).map(|f| settings::network_display(&f.name)));
                }
                d
            },
            output: {
                let mut open: Vec<(usize, String)> = self.outputs.iter().map(|o| (o.index, o.display.clone())).collect();
                open.extend(self.net.list.iter().filter(|n| n.stream.ready()).map(|n| (n.index, n.name.clone())));
                open.sort_by_key(|o| o.0);
                let names: Vec<String> = open.into_iter().map(|o| o.1).collect();
                (!names.is_empty()).then(|| names.join(" + "))
            },
            preview_aspect: self.frame_views.as_ref().map_or(0.0, |v| v.main_aspect),
            output_sizes: (0..self.settings.output.outputs.len())
                .map(|k| {
                    self.outputs.iter().find(|o| o.index == k).map(|o| o.target.size()).or_else(|| {
                        self.net.list.iter().find(|n| n.index == k && n.stream.ready()).map(|n| n.size())
                    })
                })
                .collect(),
            venues: {
                let library = settings::venue_library();
                self.venues
                    .iter()
                    .map(|p| ui::venues::VenueInfo {
                        name: settings::venue_name(p),
                        summary: self.venue_cache.get(p).map(|v| v.venue_summary()).unwrap_or_default(),
                        path: p.clone(),
                        folder: p.parent().map(noctiluca::files::pretty_folder).unwrap_or_default(),
                        in_library: p.parent() == Some(library.as_path()),
                    })
                    .collect()
            },
            venue_active: self.settings.venue_file.as_ref().filter(|p| self.venue_cache.contains_key(*p)).map(|p| settings::venue_name(p)),
            venue_path: self.settings.venue_file.clone().filter(|p| self.venue_cache.contains_key(p)),
            venue_changed: self.settings.venue_file.as_ref().and_then(|p| self.venue_cache.get(p)).is_some_and(|v| *v != self.settings.venue_preset()),
            venue_library: noctiluca::files::pretty_folder(&settings::venue_library()),
            net_screens: {
                let boxes = self.boxes.lock().map(|b| b.clone()).unwrap_or_default();
                self.discovery
                    .as_ref()
                    .map(|d| d.found())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|f| {
                        let job = boxes.get(&f.ip.to_string()).cloned().unwrap_or_default();
                        let display = settings::network_display(&f.name);
                        ui::NetScreen {
                            used_by: self.settings.output.outputs.iter().position(|o| o.display.as_deref() == Some(display.as_str())),
                            name: f.name,
                            ip: f.ip.to_string(),
                            model: f.model,
                            app_open: f.app_open,
                            android_tv: f.android_tv,
                            app_version: f.app_version,
                            install: job.state,
                            busy: job.busy,
                            error: job.error,
                        }
                    })
                    .collect()
            },
            dmx: self.dmx.as_ref().map(|d| {
                let st = d.status();
                ui::lights_page::DmxLive {
                    port: st.port,
                    kind: st.kind.map(|k| k.name()),
                    sending: st.sending,
                    rate: self.dmx_count.2,
                    error: st.error,
                }
            }),
            dmx_ports: self.dmx_ports.clone(),
            lights: self.lights_now.iter().map(|o| o.seen()).collect(),
            light_effect: Some(self.light_engine.effect()),
            section: ui::live_page::SECTIONS[match f.mood {
                Mood::Silence => 0,
                Mood::Groove => 1,
                Mood::Build => 2,
                Mood::Drop => 3,
                Mood::Breakdown => 4,
            }]
            .to_string(),
            theme: THEMES.get(self.director.theme()).map(|t| ui::live_page::title_case(t.name)).unwrap_or_default(),
            look_held: self.director.locked(),
            bar_beat: (f.beat_count / 4 + 1, f.beat_count % 4 + 1),
            timeline: [
                self.timeline.runs(noctiluca::timeline::Lane::Section),
                self.timeline.runs(noctiluca::timeline::Lane::Theme),
                self.timeline.runs(noctiluca::timeline::Lane::Effect),
            ],
            timeline_secs: self.timeline.secs(),
            net_links: (0..self.settings.output.outputs.len())
                .map(|k| {
                    self.net.status(k).map(|s| ui::NetLink {
                        connected: s.connected,
                        latency_ms: s.latency_ms,
                        round_trip_ms: s.round_trip_ms,
                        shown: s.stats.shown,
                        late: s.stats.late,
                        hdr: s.hello.map(|h| h.hdr).unwrap_or_default(),
                        error: s.error,
                    })
                })
                .collect(),
            message: self
                .message
                .as_ref()
                .filter(|(_, at)| at.elapsed().as_secs_f32() < MESSAGE_SECS)
                .map(|(m, _)| m.clone()),
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    /// Draws one frame of the menu (or another page) over the live preview.
    fn draw_menu(
        &mut self,
        mut uniforms: Uniforms,
        show: &DirectorOut,
        event_loop: &ActiveEventLoop,
    ) {
        // The preview has the menu's own overlay; no HUD or label in it.
        uniforms.misc[0] = 0.0;
        uniforms.overlay[0] = 0.0;
        let live = self.live();
        let (Some(renderer), Some(ui), Some(window)) =
            (&mut self.renderer, &mut self.ui, &self.window)
        else {
            return;
        };
        let Some(frame) = renderer.acquire() else {
            return;
        };
        let mut encoder = renderer
            .device()
            .create_command_encoder(&Default::default());
        // With a projector running, its main picture is the preview; drawing
        // it again at another size would cost as much again.
        let preview = match (&self.frame_views, renderer.view_texture(MAIN_VIEW)) {
            (Some(_), Some(view)) => view,
            _ => renderer.render_preview(&mut encoder, &uniforms, show, PREVIEW_SIZE),
        };
        let preview_id = ui.set_texture(ui::SHOW_TEXTURE, renderer.device(), &preview);
        let output_id = if self.state.page == noctiluca::app_state::Page::Mapping {
            let k = ui
                .pages
                .mapping
                .output
                .min(self.settings.output.outputs.len() - 1);
            let px = live
                .output_sizes
                .get(k)
                .copied()
                .flatten()
                .unwrap_or((1920, 1080));
            self.output_preview
                .draw(
                    renderer,
                    &mut encoder,
                    &self.settings,
                    k,
                    px,
                    self.frame_views.as_ref(),
                    &uniforms,
                    show,
                )
                .map(|view| ui.set_texture(ui::OUTPUT_TEXTURE, renderer.device(), &view))
        } else {
            None
        };
        let target = frame.texture.create_view(&Default::default());
        let mut settings = self.settings.clone();
        let mut actions = Vec::new();
        let mut pages = std::mem::take(&mut ui.pages);
        let state = self.state;
        let size = renderer.surface_size();
        ui.frame(
            window,
            renderer.device(),
            renderer.queue(),
            &mut encoder,
            &target,
            size,
            |eui| {
                ui::draw_page(
                    eui,
                    &state,
                    &mut settings,
                    &live,
                    ui::Previews {
                        show: Some(preview_id),
                        output: output_id,
                    },
                    &mut pages,
                    &mut actions,
                );
            },
        );
        ui.pages = pages;
        renderer.submit(encoder, frame);
        self.apply_settings(settings);
        for action in actions {
            self.act(action, event_loop);
        }
    }

    /// Saves the current page of the menu as a PNG, at the snapshot size.
    fn capture_page(
        &mut self,
        mut uniforms: Uniforms,
        show: &DirectorOut,
        path: &std::path::Path,
    ) -> Result<()> {
        uniforms.misc[0] = 0.0;
        uniforms.overlay[0] = 0.0;
        let live = self.live();
        let state = self.state;
        let mut settings = self.settings.clone();
        let (Some(renderer), Some(ui)) = (&mut self.renderer, &mut self.ui) else {
            anyhow::bail!("no renderer");
        };
        let size = (self.args.width, self.args.height);
        let output_preview = &mut self.output_preview;
        let mapping_tab = self.args.snapshot_mapping_tab.clone();
        renderer.capture(size, path, |r, encoder, target| {
            let preview = r.render_preview(encoder, &uniforms, show, PREVIEW_SIZE);
            let id = ui.set_texture(ui::SHOW_TEXTURE, r.device(), &preview);
            let output = (state.page == noctiluca::app_state::Page::Mapping)
                .then(|| {
                    output_preview.draw(
                        r,
                        encoder,
                        &settings,
                        0,
                        (1920, 1080),
                        None,
                        &uniforms,
                        show,
                    )
                })
                .flatten()
                .map(|view| ui.set_texture(ui::OUTPUT_TEXTURE, r.device(), &view));
            let mut pages = ui::PageState::default();
            pages.mapping.tab = match mapping_tab.as_deref() {
                Some("transform") => ui::mapping_page::PanelTab::Transform,
                Some("look") => ui::mapping_page::PanelTab::Look,
                Some("output") => ui::mapping_page::PanelTab::Output,
                _ => ui::mapping_page::PanelTab::Surfaces,
            };
            let mut actions = Vec::new();
            ui.paint_offscreen(r.device(), r.queue(), encoder, target, size, 2.0, |eui| {
                ui::draw_page(
                    eui,
                    &state,
                    &mut settings,
                    &live,
                    ui::Previews {
                        show: Some(id),
                        output,
                    },
                    &mut pages,
                    &mut actions,
                );
            });
        })
    }

    /// Saves what a projector would show with the current output settings.
    fn capture_output(
        &mut self,
        uniforms: &Uniforms,
        show: &DirectorOut,
        path: &std::path::Path,
    ) -> Result<()> {
        let size = (self.args.width, self.args.height);
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no renderer"))?;
        let mut projector = Projector::new(
            renderer.device(),
            renderer.shader_dir(),
            renderer.surface_format(),
        )?;
        let s = self.settings.clone();
        let planned = stage::plan(&s.output.surfaces, 0, s.strobe, uniforms.flow[3]);
        let conf = s.output.outputs[0].clone();
        let pattern = pattern_of(conf.showing);
        let mut u = *uniforms;
        u.misc[0] = 0.0;
        u.overlay[0] = 0.0;
        let height = s.quality.view_height();
        renderer.capture(size, path, |r, encoder, target| {
            let views = (pattern == Pattern::Show).then(|| {
                let backgrounds: &[(u32, u32)] = if conf.black_outside { &[] } else { &[size] };
                Projector::render_views(
                    r,
                    encoder,
                    ViewSet::Now,
                    &[(&planned, size)],
                    backgrounds,
                    height,
                    &u,
                    show,
                )
            });
            let main_size = r.view_size(MAIN_VIEW).unwrap_or(size);
            projector.compose(
                r.device(),
                r.queue(),
                encoder,
                target,
                size,
                &planned,
                views.as_ref(),
                main_size,
                pattern,
                conf.black_outside,
                30,
                None,
            );
        })
    }

    /// Looks up the inputs, displays and venues again, now and then.
    fn refresh_lists(&mut self, event_loop: &ActiveEventLoop) {
        if self.lists_at.is_some_and(|t| t.elapsed() < REFRESH_LISTS)
            || self.state.control_shows_show()
        {
            return;
        }
        self.lists_at = Some(Instant::now());
        self.inputs = audio::list_inputs();
        let here = self
            .window
            .as_ref()
            .and_then(|w| w.current_monitor())
            .and_then(|m| m.name());
        self.displays = event_loop
            .available_monitors()
            .filter_map(|m| m.name())
            .filter(|name| Some(name) != here.as_ref())
            .collect();
        self.reload_venues();
        self.dmx_ports = noctiluca::dmx::ports();
    }

    /// A live input that has delivered nothing for a few seconds was probably
    /// unplugged or reconfigured. Try to open it again every few seconds.
    fn reconnect_if_lost(&mut self) {
        let live = self.audio.connected && self.args.file.is_none();
        if !live || self.analyzer.starved_secs() < RECONNECT_AFTER_SECS {
            return;
        }
        if self
            .last_reconnect
            .is_some_and(|t| t.elapsed().as_secs_f32() < RECONNECT_EVERY_SECS)
        {
            return;
        }
        if self.last_reconnect.is_none() {
            eprintln!(
                "audio from \"{}\" stopped arriving; trying to reconnect",
                self.audio.label
            );
        }
        self.last_reconnect = Some(Instant::now());
        if let Ok(audio) = audio::open_input(self.args.device.as_deref()) {
            println!("listening to: {} ({} Hz)", audio.label, audio.sample_rate);
            if audio.sample_rate != self.audio.sample_rate {
                self.analyzer = Analyzer::new(audio.sample_rate);
                // A WAV file has one sample rate; it cannot continue at another.
                if self.recorder.take().is_some() {
                    eprintln!("recording stopped: the input came back at a different sample rate");
                }
            }
            // Replacing the old value drops it, which closes the dead stream.
            self.audio = audio;
            self.seen_audio_errors = 0;
            self.last_reconnect = None;
        }
    }

    /// Notes the current position in the recording, in the terminal and in a
    /// text file next to the WAV.
    fn mark(&mut self) {
        // A copy of the path: borrowing it from `self` would block show_message.
        let Some(path) = self.args.record.clone().filter(|_| self.recorder.is_some()) else {
            self.show_message("M marks a moment only while recording (--record)".into());
            return;
        };
        self.marks += 1;
        let secs = self.recorded_samples as f64 / self.audio.sample_rate as f64;
        // On screen too, so it is clear the key reached the program.
        self.show_message(format!("mark {} at {secs:.1} s", self.marks));
        let line = format!("{secs:.2}\n");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(marks_path(&path));
        if let Err(e) = file.and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes())) {
            eprintln!("could not save the mark: {e}");
        }
    }

    fn toggle_fullscreen(&self) {
        if let Some(window) = &self.window {
            let going_full = !is_full_screen(window);
            set_full_screen(window, going_full);
            // A mouse pointer parked in the middle of the visuals ruins them.
            window.set_cursor_visible(!going_full);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
        if self.args.list_monitors {
            for (i, m) in monitors.iter().enumerate() {
                let size = m.size();
                let name = m.name().unwrap_or_else(|| "unnamed".into());
                println!("{i}: {name} ({}x{})", size.width, size.height);
            }
            event_loop.exit();
            return;
        }
        if let Some(n) = self.args.monitor {
            self.monitor = monitors.get(n).cloned();
            if self.monitor.is_none() {
                eprintln!("there is no monitor {n}; see --list-monitors. Using the default.");
            }
        }

        let snapshot = self.args.snapshot.is_some();
        // --monitor names the projector: the output goes there, the menu
        // stays here in a normal window.
        if let Some(name) = self.monitor.as_ref().and_then(|m| m.name())
            && !snapshot
        {
            let mut s = self.settings.clone();
            s.output.outputs[0].display = Some(name);
            self.apply_settings(s);
        }
        if let Some(name) = self.args.screen.clone()
            && !snapshot
        {
            self.use_network_screen(&name);
        }
        if self.args.play && !snapshot {
            self.play_at = Some(Instant::now());
        }
        let attrs = Window::default_attributes()
            .with_title("Noctiluca")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(LogicalSize::new(320.0, 180.0))
            .with_resizable(true)
            .with_visible(!snapshot);
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("cannot open a window"),
        );
        let dir = self
            .args
            .shaders
            .clone()
            .unwrap_or_else(shaders::default_dir);
        let models = self
            .args
            .models
            .clone()
            .unwrap_or_else(noctiluca::render::model::default_dir);
        match Renderer::new(window.clone(), dir, &models) {
            Ok(mut renderer) => {
                if snapshot {
                    renderer.set_render_size(self.args.width, self.args.height);
                }
                if !snapshot || self.args.snapshot_page.is_some() {
                    self.ui = Some(Ui::new(
                        &window,
                        renderer.device(),
                        renderer.surface_format(),
                    ));
                }
                self.renderer = Some(renderer);
                self.window = Some(window);
                self.follow_state();
            }
            Err(e) => {
                eprintln!("cannot start the renderer: {e:#}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::ModifiersChanged(m) = &event {
            self.modifiers = m.state();
        }
        // Cmd+Q quits from anywhere.
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(KeyCode::KeyQ),
                    state: ElementState::Pressed,
                    ..
                },
            ..
        } = &event
            && self.modifiers.super_key()
        {
            self.quit(event_loop);
            return;
        }
        // ⌘S save, ⇧⌘S save as, ⌘O open: the venue, as files in any Mac app.
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(key @ (KeyCode::KeyS | KeyCode::KeyO)),
                    state: ElementState::Pressed,
                    repeat: false,
                    ..
                },
            ..
        } = &event
            && self.modifiers.super_key()
            && self.args.snapshot.is_none()
        {
            let action = match (key, self.modifiers.shift_key()) {
                (KeyCode::KeyO, _) => UiAction::OpenVenue,
                (_, true) => UiAction::SaveVenueAs,
                _ => UiAction::SaveVenue,
            };
            self.act(action, event_loop);
            return;
        }
        // A venue file dropped on any window opens.
        if let WindowEvent::DroppedFile(path) = &event {
            self.open_venue_file(path.clone(), event_loop);
            return;
        }
        if self.outputs.iter().any(|o| o.window.id() == _id) {
            self.output_event(_id, event, event_loop);
            return;
        }
        // On a page of the menu, egui has the keyboard and mouse first.
        if !self.state.control_shows_show()
            && let (Some(ui), Some(window)) = (&mut self.ui, &self.window)
        {
            let consumed = ui.on_event(window, &event);
            if let WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } = &event
                && !consumed
            {
                match key {
                    KeyCode::Escape => self.act(UiAction::App(Action::Escape), event_loop),
                    KeyCode::Space if !self.state.running => {
                        self.act(UiAction::App(Action::Start), event_loop)
                    }
                    KeyCode::BracketLeft => self.nudge_sync(-1),
                    KeyCode::BracketRight => self.nudge_sync(1),
                    _ => {}
                }
                return;
            }
            if matches!(event, WindowEvent::KeyboardInput { .. }) {
                return;
            }
        }
        match event {
            WindowEvent::CloseRequested => self.quit(event_loop),
            WindowEvent::Occluded(hidden) => self.hidden = hidden,
            WindowEvent::CursorMoved { .. } => self.last_mouse = Some(Instant::now()),
            WindowEvent::Resized(size) => {
                // In snapshot mode the picture size comes from the command
                // line, not from the hidden window.
                if let Some(renderer) = &mut self.renderer
                    && self.args.snapshot.is_none()
                {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => match key {
                KeyCode::Escape => self.act(UiAction::App(Action::Escape), event_loop),
                KeyCode::KeyF => self.toggle_fullscreen(),
                KeyCode::KeyH => {
                    self.hud = !self.hud;
                    self.show_message(if self.hud {
                        "overlay: always on".into()
                    } else {
                        "overlay: with the mouse".into()
                    });
                }
                KeyCode::KeyM => self.mark(),
                KeyCode::BracketLeft => self.nudge_sync(-1),
                KeyCode::BracketRight => self.nudge_sync(1),
                KeyCode::KeyI => self.next_input(),
                KeyCode::KeyS => {
                    let mut s = self.settings.clone();
                    s.strobe = !s.strobe;
                    println!("strobe {}", if s.strobe { "on" } else { "off" });
                    self.apply_settings(s);
                }
                KeyCode::Space => self.director.next(),
                KeyCode::KeyL => {
                    let locked = self.director.toggle_lock();
                    println!(
                        "{}",
                        if locked {
                            "locked: nothing changes on its own"
                        } else {
                            "unlocked"
                        }
                    );
                }
                KeyCode::Digit1 => self.director.edit(|l| l.bg = 0),
                KeyCode::Digit2 => self.director.edit(|l| l.bg = 1),
                KeyCode::Digit3 => self.director.edit(|l| l.bg = 2),
                KeyCode::Digit4 => self.director.edit(|l| l.bg = 3),
                KeyCode::Digit5 => self.director.edit(|l| l.bg = 4),
                KeyCode::KeyQ => self.director.edit(|l| l.pattern = 0),
                KeyCode::KeyW => self.director.edit(|l| l.pattern = 1),
                KeyCode::KeyE => self.director.edit(|l| l.pattern = 2),
                KeyCode::KeyR => self.director.edit(|l| l.pattern = 3),
                KeyCode::KeyT => self.director.edit(|l| l.pattern = 4),
                KeyCode::KeyY => self.director.edit(|l| l.pattern = 5),
                KeyCode::KeyZ => self.director.edit(|l| l.pattern = 6),
                KeyCode::KeyX => self.director.edit(|l| l.pattern = 7),
                KeyCode::KeyC => self.director.edit(|l| l.pattern = 8),
                KeyCode::KeyP => self.director.next_palette(),
                KeyCode::KeyO => self.director.edit(|l| l.post += 1),
                KeyCode::KeyG => self.director.edit(|l| l.object += 1),
                KeyCode::KeyB => self.director.edit(|l| l.material += 1),
                KeyCode::KeyA => self.director.edit(|l| l.atmos += 1),
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                // Frames are also drawn from about_to_wait; never twice in one interval.
                if Instant::now() + Duration::from_millis(2)
                    >= self.last_frame + self.frame_interval()
                {
                    self.draw_frame(event_loop);
                }
            }
            _ => {}
        }
    }

    /// Runs after each batch of events. Asking for a redraw here is what
    /// keeps the animation going.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(path) = self.args.snapshot.clone() else {
            // At most 60 frames a second, even on a 120 Hz screen: twice the
            // frames would look the same in a club and cost the GPU double,
            // which matters with DJ software running beside it.
            self.refresh_lists(event_loop);
            self.poll_file_panel(event_loop);
            self.check_output(event_loop);
            // --play: start once the screens are there, or after a few seconds.
            if let Some(at) = self.play_at
                && (self.has_output() && at.elapsed() > Duration::from_millis(500) || at.elapsed() > Duration::from_secs(4))
            {
                self.play_at = None;
                self.act(UiAction::App(Action::Start), event_loop);
            }
            let next = self.last_frame + self.frame_interval();
            if Instant::now() >= next {
                if self.feeding_screens() {
                    // The projector does not wait for the Mac's window, which
                    // macOS may not redraw at all while it is hidden.
                    self.draw_frame(event_loop);
                } else if let Some(window) = &self.window {
                    window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Poll);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(next));
            }
            return;
        };

        // Snapshot mode: a hidden window gets no redraw events, so step here.
        if !self.args.bench {
            std::thread::sleep(Duration::from_millis(16));
        }
        let Some((mut uniforms, mut show)) = self.tick() else {
            return;
        };
        if let Some(kind) = &self.args.snapshot_transition {
            use noctiluca::director::Transition;
            let transition = match kind.as_str() {
                "ink" => Transition::Ink,
                "zoom" => Transition::Zoom,
                _ => Transition::Fade,
            };
            let mut to = show.to;
            to.bg = (to.bg + 7) % BACKGROUNDS.len();
            to.pattern = (to.pattern + 5) % PATTERNS.len();
            let mix = self.args.snapshot_mix.clamp(0.0, 1.0);
            show = DirectorOut { from: show.to, to, mix, transition, ..show };
            uniforms.state[1] = mix;
            uniforms.overlay[3] = transition.code();
        }
        let look = show.to;
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        if self.start.elapsed().as_secs_f32() < self.args.at {
            let began = Instant::now();
            renderer.frame_offscreen(&uniforms, &show);
            if self.args.bench {
                renderer.wait_idle();
                self.bench.0 += 1;
                self.bench.1 += began.elapsed().as_secs_f32();
            }
            return;
        }
        if self.args.bench && self.bench.0 > 0 {
            let ms = 1000.0 * self.bench.1 / self.bench.0 as f32;
            println!(
                "{} + {} at {}x{}: {ms:.2} ms per frame ({:.0} fps possible)",
                BACKGROUNDS[look.bg],
                PATTERNS[look.pattern],
                self.args.width,
                self.args.height,
                1000.0 / ms
            );
        }
        if self.args.snapshot_demo_layout {
            let mut s = self.settings.clone();
            s.output = demo_layout();
            if let Some(p) = &self.args.snapshot_pattern {
                s.output.outputs[0].showing = match p.as_str() {
                    "grid" => OutputPattern::Grid,
                    "white" => OutputPattern::White,
                    "black" => OutputPattern::Black,
                    _ => OutputPattern::Show,
                };
            }
            // For the picture only: never saved.
            self.settings = s;
        }
        if self.args.snapshot_page.as_deref() == Some("output") {
            match self.capture_output(&uniforms, &show, &path) {
                Ok(()) => println!("saved {}", path.display()),
                Err(e) => eprintln!("snapshot failed: {e:#}"),
            }
            event_loop.exit();
            return;
        }
        if let Some(page) = self.args.snapshot_page.clone() {
            self.state.page = match page.as_str() {
                "settings" => noctiluca::app_state::Page::Settings,
                "mapping" => noctiluca::app_state::Page::Mapping,
                "lights" => noctiluca::app_state::Page::Lights,
                "network" => noctiluca::app_state::Page::Network,
                "venues" => noctiluca::app_state::Page::Venues,
                _ => noctiluca::app_state::Page::Live,
            };
            self.state.show_here = false;
            // The menu as it looks before Start.
            self.state.running = false;
            self.lists_at = None;
            self.refresh_lists(event_loop);
            match self.capture_page(uniforms, &show, &path) {
                Ok(()) => println!("saved {}", path.display()),
                Err(e) => eprintln!("snapshot failed: {e:#}"),
            }
            event_loop.exit();
            return;
        }
        match renderer.snapshot(&uniforms, &show, &path) {
            Ok(()) => println!("saved {}", path.display()),
            Err(e) => eprintln!("snapshot failed: {e:#}"),
        }
        event_loop.exit();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.saver.flush();
    }
}

/// Stops macOS from dimming or sleeping the display while the visuals run.
/// Nobody touches the keyboard during a set, so without this the screen
/// would go dark after the usual idle time.
///
/// `caffeinate` ships with macOS; `-d` holds the display awake and `-w`
/// makes it quit by itself when this program does.
struct KeepAwake(std::process::Child);

impl KeepAwake {
    fn start() -> Option<Self> {
        if !cfg!(target_os = "macos") {
            return None;
        }
        std::process::Command::new("caffeinate")
            .args(["-d", "-w", &std::process::id().to_string()])
            .spawn()
            .map(Self)
            .ok()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// How visible the overlay is, given whether it is pinned on and how many
/// seconds ago the mouse last moved (`None`: not since the start).
fn hud_opacity(pinned: bool, idle: Option<f32>) -> f32 {
    if pinned {
        return 1.0;
    }
    let Some(idle) = idle else { return 0.0 };
    (1.0 - (idle - HUD_IDLE_SECS) / HUD_FADE_SECS).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_follows_the_mouse_unless_pinned() {
        assert_eq!(hud_opacity(false, None), 0.0, "no mouse yet: nothing");
        assert_eq!(hud_opacity(false, Some(0.5)), 1.0, "just moved: shown");
        assert_eq!(hud_opacity(false, Some(HUD_IDLE_SECS - 0.1)), 1.0);
        let fading = hud_opacity(false, Some(HUD_IDLE_SECS + HUD_FADE_SECS / 2.0));
        assert!(fading > 0.3 && fading < 0.7, "{fading}");
        assert_eq!(
            hud_opacity(false, Some(HUD_IDLE_SECS + HUD_FADE_SECS + 0.1)),
            0.0,
            "gone"
        );
        assert_eq!(hud_opacity(true, Some(60.0)), 1.0, "pinned with H: stays");
    }

    #[test]
    fn the_overlay_shows_at_start_and_then_follows_the_mouse() {
        // No setting for this: the program always starts this way.
        let shown_at_start = start_overlay(&Args::parse_from(["noctiluca"]));
        assert!(shown_at_start);
        assert_eq!(hud_opacity(false, Some(0.0)), 1.0, "shown at the start");
        assert_eq!(
            hud_opacity(false, Some(10.0)),
            0.0,
            "and gone once the mouse rests"
        );
        // A saved picture shows the visuals only.
        assert!(!start_overlay(&Args::parse_from([
            "noctiluca",
            "--snapshot",
            "x.png"
        ])));
    }
}

/// Whether the overlay shows as the program starts, as if the mouse had
/// just moved: always, except when only saving a picture.
fn start_overlay(args: &Args) -> bool {
    args.snapshot.is_none()
}

/// Frame-rate bookkeeping for --stats.
#[derive(Default)]
struct Stats {
    frames: u32,
    secs: f32,
    worst: f32,
}

impl Stats {
    /// Counts a frame; true when it printed.
    fn frame(&mut self, dt: f32) -> bool {
        self.frames += 1;
        self.secs += dt;
        self.worst = self.worst.max(dt);
        if self.secs >= 2.0 {
            println!(
                "{:.0} fps, average frame {:.1} ms, slowest {:.1} ms",
                self.frames as f32 / self.secs,
                1000.0 * self.secs / self.frames as f32,
                1000.0 * self.worst
            );
            *self = Self::default();
            return true;
        }
        false
    }
}

fn mood_name(mood: Mood) -> &'static str {
    match mood {
        Mood::Silence => "silent",
        Mood::Groove => "groove",
        Mood::Build => "build",
        Mood::Drop => "DROP",
        Mood::Breakdown => "breakdown",
    }
}

fn open_audio(args: &Args) -> AudioIn {
    let opened = match &args.file {
        Some(path) => audio::open_file(path, args.mute),
        None => audio::open_input(args.device.as_deref()),
    };
    match opened {
        Ok(audio) => {
            println!("listening to: {} ({} Hz)", audio.label, audio.sample_rate);
            audio
        }
        Err(e) => {
            // No sound is not a reason to show nothing.
            eprintln!("no audio, running in idle mode: {e:#}");
            AudioIn::silent()
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.list_devices {
        for name in audio::list_inputs() {
            println!("{name}");
        }
        return Ok(());
    }

    let (mut settings, message) = settings::load();
    if let Some(m) = &message {
        eprintln!("{m}");
    }
    // Venues from before venue files become files in the venue folder, once.
    let moved = settings::migrate_old_venues(&settings::dir(), &settings::venue_library());
    settings.adopt_venue_files(&settings::venue_library());
    if moved > 0 {
        eprintln!("moved {moved} venues to {}", settings::venue_library().display());
    }
    let mut args = args;
    // The input chosen in the menu last time, unless the command line says otherwise.
    if args.device.is_none() && args.file.is_none() {
        args.device = settings.input_device.clone();
    }
    let audio = open_audio(&args);
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(args, audio, settings, message);
    event_loop.run_app(&mut app)?;
    Ok(())
}
