//! DMX lights that move with the show (docs/superpowers/specs/2026-11-21-dmx-lighting-design.md).
//!
//! The setup (lights, their addresses and channels, what each does) is part
//! of the settings; the engine turns the show's colours, beat and vibe into
//! each light's colour and brightness, every change faded, and those into
//! the 512 DMX channels the interface sends.

use serde::{Deserialize, Serialize};

use crate::render::uniforms::Uniforms;

pub const UNIVERSE: usize = 512;
pub const MAX_FIXTURES: usize = 32;

/// What one DMX channel of a light does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    Dimmer,
    Red,
    Green,
    Blue,
    White,
    Amber,
    Uv,
    /// The light's own strobe: kept at 0 (flashes are made with the colour,
    /// so they stay within the flash limit).
    Strobe,
    /// A smoke machine's output.
    Fog,
    /// Always 0 (a mode or speed channel: 0 is "follow DMX" on most lights).
    Zero,
    /// Always 255.
    Full,
}

impl Channel {
    pub const ALL: [Channel; 11] = [
        Channel::Dimmer,
        Channel::Red,
        Channel::Green,
        Channel::Blue,
        Channel::White,
        Channel::Amber,
        Channel::Uv,
        Channel::Strobe,
        Channel::Fog,
        Channel::Zero,
        Channel::Full,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Channel::Dimmer => "Dimmer",
            Channel::Red => "Red",
            Channel::Green => "Green",
            Channel::Blue => "Blue",
            Channel::White => "White",
            Channel::Amber => "Amber",
            Channel::Uv => "UV",
            Channel::Strobe => "Strobe (kept off)",
            Channel::Fog => "Fog",
            Channel::Zero => "Always 0",
            Channel::Full => "Always 255",
        }
    }
}

/// Which channels a light has, in order from its start address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    /// R, G, B.
    Rgb,
    /// Dimmer, R, G, B.
    DimRgb,
    /// Dimmer, R, G, B, strobe, mode, speed: the common cheap RGB spot.
    Spot7,
    /// R, G, B, W.
    Rgbw,
    /// Dimmer, R, G, B, W.
    DimRgbw,
    /// One channel: fog.
    Smoke,
    /// Channels as listed on the light.
    Custom,
}

impl Profile {
    pub const ALL: [Profile; 7] = [Profile::Spot7, Profile::DimRgb, Profile::Rgb, Profile::DimRgbw, Profile::Rgbw, Profile::Smoke, Profile::Custom];

    pub fn name(self) -> &'static str {
        match self {
            Profile::Rgb => "RGB · 3 ch",
            Profile::DimRgb => "Dimmer + RGB · 4 ch",
            Profile::Spot7 => "RGB spot · 7 ch",
            Profile::Rgbw => "RGBW · 4 ch",
            Profile::DimRgbw => "Dimmer + RGBW · 5 ch",
            Profile::Smoke => "Smoke machine · 1 ch",
            Profile::Custom => "Custom",
        }
    }

    pub fn channels(self) -> Vec<Channel> {
        use Channel::*;
        match self {
            Profile::Rgb => vec![Red, Green, Blue],
            Profile::DimRgb => vec![Dimmer, Red, Green, Blue],
            Profile::Spot7 => vec![Dimmer, Red, Green, Blue, Strobe, Zero, Zero],
            Profile::Rgbw => vec![Red, Green, Blue, White],
            Profile::DimRgbw => vec![Dimmer, Red, Green, Blue, White],
            Profile::Smoke => vec![Fog],
            Profile::Custom => vec![],
        }
    }
}

/// What a light does in the show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// The show's colours, a soft bump on every kick, the vibe's brightness.
    Follow,
    /// The colours only, slowly: a calm wash, no bumps.
    Wash,
    /// Takes turns with the other chase lights on the beat.
    Chase,
    Off,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Follow, Role::Wash, Role::Chase, Role::Off];
    pub fn name(self) -> &'static str {
        match self {
            Role::Follow => "Follow",
            Role::Wash => "Wash",
            Role::Chase => "Chase",
            Role::Off => "Off",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fixture {
    pub name: String,
    /// The DMX start address, 1..=512, as set on the light's display.
    pub address: u16,
    pub profile: Profile,
    /// The channels, for a custom light.
    pub custom: Vec<Channel>,
    pub role: Role,
    /// 0..1.
    pub brightness: f32,
    pub enabled: bool,
}

impl Default for Fixture {
    fn default() -> Self {
        Fixture { name: "Spot 1".into(), address: 1, profile: Profile::Spot7, custom: Vec::new(), role: Role::Follow, brightness: 1.0, enabled: true }
    }
}

impl Fixture {
    pub fn channels(&self) -> Vec<Channel> {
        match self.profile {
            Profile::Custom => self.custom.clone(),
            p => p.channels(),
        }
    }

    pub fn is_smoke(&self) -> bool {
        self.channels().contains(&Channel::Fog)
    }

    /// The channels it takes, as 1-based addresses.
    pub fn span(&self) -> std::ops::RangeInclusive<u16> {
        let n = self.channels().len().max(1) as u16;
        self.address..=(self.address + n - 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmokeMode {
    Off,
    /// Only while the button is held.
    Hand,
    /// A burst on a drop or at the top of a build.
    Auto,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    /// The interface's port; `None`: the first one found.
    pub port: Option<String>,
    pub fixtures: Vec<Fixture>,
    /// 0..1, over every light.
    pub master: f32,
    pub blackout: bool,
    /// White flashes on the show's strobe (within the same limit).
    pub flashes: bool,
    /// Brightness follows the section and the vibe; off: steady.
    pub follow_vibe: bool,
    /// How long a colour change takes, seconds.
    pub fade_secs: f32,
    pub smoke: SmokeMode,
    pub smoke_secs: f32,
    /// Never more often than this, seconds.
    pub smoke_gap_secs: f32,
    /// 0..1: how much fog while it is on.
    pub smoke_amount: f32,
    /// The effect for the lights in Follow; `None`: Auto (follows the show).
    pub effect: Option<Effect>,
    /// 0 subtle .. 0.5 normal .. 1 wild: how bright, how hard the bumps and
    /// chases hit, how vivid the colours.
    pub drive: f32,
    /// The lights' own white hits.
    pub strobe: LightStrobe,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting {
            port: None,
            fixtures: vec![Fixture::default()],
            master: 1.0,
            blackout: false,
            flashes: true,
            follow_vibe: true,
            fade_secs: 0.6,
            smoke: SmokeMode::Off,
            smoke_secs: 3.0,
            smoke_gap_secs: 60.0,
            smoke_amount: 1.0,
            effect: None,
            drive: 0.5,
            strobe: LightStrobe::Drops,
        }
    }
}

fn limit(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

impl Lighting {
    pub fn validated(mut self) -> Self {
        self.fixtures.truncate(MAX_FIXTURES);
        for f in &mut self.fixtures {
            f.address = f.address.clamp(1, UNIVERSE as u16);
            f.brightness = limit(f.brightness, 0.0, 1.0, 1.0);
            f.custom.truncate(32);
            if f.name.trim().is_empty() {
                f.name = "Light".into();
            }
        }
        self.master = limit(self.master, 0.0, 1.0, 1.0);
        self.fade_secs = limit(self.fade_secs, 0.05, 5.0, 0.6);
        self.smoke_secs = limit(self.smoke_secs, 0.5, 15.0, 3.0);
        self.smoke_gap_secs = limit(self.smoke_gap_secs, 10.0, 600.0, 60.0);
        self.smoke_amount = limit(self.smoke_amount, 0.0, 1.0, 1.0);
        self.drive = limit(self.drive, 0.0, 1.0, 0.5);
        self
    }

    /// The next free address after the lights there are.
    pub fn next_address(&self) -> u16 {
        self.fixtures.iter().map(|f| *f.span().end() + 1).max().unwrap_or(1).min(UNIVERSE as u16)
    }

    /// Lights whose channels overlap another's (likely a wrong address).
    pub fn overlaps(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (i, a) in self.fixtures.iter().enumerate() {
            for (j, b) in self.fixtures.iter().enumerate().skip(i + 1) {
                if a.span().start() <= b.span().end() && b.span().start() <= a.span().end() {
                    out.push((i, j));
                }
            }
        }
        out
    }
}

/// What the show is doing, for the lights.
#[derive(Clone, Copy, Debug, Default)]
pub struct Show {
    /// The show's cosine palette (a + b cos 2π(c t + d)).
    pub palette: [[f32; 4]; 4],
    pub beats: f32,
    pub pulse: f32,
    pub bpm: f32,
    /// 0 silence, 1 groove, 2 build, 3 drop, 4 breakdown.
    pub mood: u8,
    /// 0..1, how far into a build.
    pub tension: f32,
    /// The show's flash, already limited (0..1).
    pub flash: f32,
    /// -1 calm .. 1 intense.
    pub intensity: f32,
}

impl Show {
    pub fn from_uniforms(u: &Uniforms) -> Self {
        Show {
            palette: u.palette,
            beats: u.flow[3],
            pulse: u.beat[1],
            bpm: u.beat[3],
            mood: u.state[0].round().clamp(0.0, 4.0) as u8,
            tension: u.state[2],
            flash: u.state[3],
            intensity: u.vibe[1],
        }
    }

    /// The palette's colour at `t`, made vivid for a light: lights look
    /// washed out with the soft colours a screen can carry.
    pub fn colour(&self, t: f32) -> [f32; 3] {
        self.colour_vivid(t, 0.6)
    }

    /// As `colour`, `saturate` of the way to the pure hue (0..1).
    pub fn colour_vivid(&self, t: f32, saturate: f32) -> [f32; 3] {
        let p = &self.palette;
        let mut c = [0.0f32; 3];
        for (k, v) in c.iter_mut().enumerate() {
            *v = (p[0][k] + p[1][k] * (std::f32::consts::TAU * (p[2][k] * t + p[3][k])).cos()).max(0.0);
        }
        vivid_by(c, saturate)
    }
}

/// The colour at full strength, and more saturated: lights look washed
/// out with the soft colours a screen can carry. White stays white.
pub fn vivid(c: [f32; 3]) -> [f32; 3] {
    vivid_by(c, 0.6)
}

/// `vivid`, `saturate` of the way toward the pure hue (0: as it is, 1: no
/// white in it at all).
pub fn vivid_by(c: [f32; 3], saturate: f32) -> [f32; 3] {
    let saturate = saturate.clamp(0.0, 1.0);
    let max = c.iter().copied().fold(0.0, f32::max);
    if max < 1e-4 {
        return [1.0, 1.0, 1.0];
    }
    let n = c.map(|v| v / max);
    let min = n.iter().copied().fold(1.0, f32::min);
    if min > 0.97 {
        return n;
    }
    n.map(|v| {
        let pure = (v - min) / (1.0 - min);
        (v + (pure - v) * saturate).clamp(0.0, 1.0)
    })
}

/// One light's state this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightOut {
    /// The colour, 0..1 each, at full strength.
    pub rgb: [f32; 3],
    /// How bright, 0..1.
    pub level: f32,
    /// White mixed in by a flash, 0..1.
    pub flash: f32,
    /// A smoke machine's output, 0..1.
    pub fog: f32,
}

impl LightOut {
    /// What it looks like: the colour at its level, with the flash.
    pub fn seen(&self) -> [f32; 3] {
        let c = self.rgb.map(|v| v * self.level);
        c.map(|v| v + (1.0 - v) * self.flash)
    }
}

/// Seconds the brightness takes to follow the section.
const LEVEL_SECS: f32 = 0.8;
const PHRASE: f32 = 16.0;
/// A light may go from dark to bright and back at most this often: faster
/// than that it is a flash, and flashes are limited to three a second.
const MAX_SWING_HZ: f32 = 3.0;
/// Faster changes are kept this shallow (a shimmer, not a flash).
const SHIMMER_DEPTH: f32 = 0.08;

/// How bright a section is with the drive: subtle keeps it soft and caps the
/// top, normal is as the section says, wild lifts everything toward full.
pub fn drive_level(section: f32, drive: f32) -> f32 {
    let d = drive.clamp(0.0, 1.0);
    let s = section.clamp(0.0, 1.0);
    if d < 0.5 {
        s * (0.55 + 0.9 * d)
    } else {
        s + (1.0 - s) * (d - 0.5) * 2.0 * 0.7
    }
}

/// How vivid the colours are with the drive.
pub fn drive_saturation(drive: f32) -> f32 {
    0.3 + 0.6 * drive.clamp(0.0, 1.0)
}

/// What the lights do together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    Pulse,
    Pop,
    Split,
    Wave,
    Chase,
    Breathe,
    Riser,
}

impl Effect {
    pub const ALL: [Effect; 7] = [Effect::Pulse, Effect::Pop, Effect::Split, Effect::Wave, Effect::Chase, Effect::Breathe, Effect::Riser];

    pub fn name(self) -> &'static str {
        match self {
            Effect::Pulse => "Pulse",
            Effect::Pop => "Pop",
            Effect::Split => "Split",
            Effect::Wave => "Wave",
            Effect::Chase => "Chase",
            Effect::Breathe => "Breathe",
            Effect::Riser => "Riser",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Effect::Pulse => "The show's colours, a strong bump on every kick",
            Effect::Pop => "A new colour on every kick, each light out of step",
            Effect::Split => "Opposite colours, swapping every bar",
            Effect::Wave => "Colour flowing across the lights, a ripple travelling",
            Effect::Chase => "One light at a time on the beat",
            Effect::Breathe => "Slow rise and fall over two bars",
            Effect::Riser => "Climbs with a build, faster and brighter, hits on the drop",
        }
    }

    /// How long its colour changes take (Pop jumps, Breathe drifts).
    fn fade(self, base: f32) -> f32 {
        match self {
            Effect::Pop => 0.08,
            Effect::Split => 0.15,
            Effect::Breathe => base * 2.5,
            _ => base,
        }
    }
}

/// When the lights flash white on their own (beyond the show's flashes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightStrobe {
    Off,
    /// The drop's first beat, and bar starts during drops.
    Drops,
    /// Also every beat at the top of a build.
    Free,
}

/// A small, steady random source (the same show gives the same choices).
#[derive(Clone, Copy, Debug)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The effect Auto would choose for this section and vibe, never `last`.
pub fn auto_effect(mood: u8, intensity: f32, last: Option<Effect>, roll: f32) -> Effect {
    use Effect::*;
    let calm = intensity < 0.0;
    let choices: &[(Effect, f32)] = match mood {
        0 => &[(Breathe, 1.0)],
        2 => &[(Riser, 1.0)],
        3 => &[(Pop, 1.0), (Split, 0.8), (Chase, 0.8), (Pulse, 0.6)],
        4 => &[(Breathe, 1.0), (Wave, 0.7)],
        _ if calm => &[(Wave, 1.0), (Pulse, 0.6), (Breathe, 0.4), (Split, 0.3)],
        _ => &[(Pulse, 1.0), (Split, 0.8), (Wave, 0.6), (Pop, 0.6), (Chase, 0.4)],
    };
    let open: Vec<(Effect, f32)> = choices.iter().copied().filter(|(e, _)| Some(*e) != last || choices.len() == 1).collect();
    let total: f32 = open.iter().map(|(_, w)| w).sum();
    let mut at = roll.clamp(0.0, 0.9999) * total;
    for (e, w) in &open {
        if at < *w {
            return *e;
        }
        at -= w;
    }
    open.last().map_or(Pulse, |(e, _)| *e)
}

/// How dark a light goes between hits with the drive: at subtle it hardly
/// dims, at normal it breathes, at wild it falls close to black.
pub fn drive_floor(drive: f32) -> f32 {
    let d = drive.clamp(0.0, 1.0);
    if d < 0.5 { 0.85 - 0.5 * d } else { 0.6 - 1.1 * (d - 0.5) }
}

/// How fast the effects move with the drive: half speed at subtle, double at wild.
pub fn drive_speed(drive: f32) -> f32 {
    2f32.powf((drive.clamp(0.0, 1.0) - 0.5) * 2.0)
}

/// Colour changes with the drive: slow and soft at subtle, snappy at wild.
pub fn drive_fade(fade: f32, drive: f32) -> f32 {
    fade * (1.8 - 1.35 * drive.clamp(0.0, 1.0))
}

/// The finest step (in beats: 1, 1/2, 1/4) a light may take turns at, so
/// that no light goes dark-to-bright more than three times a second: with
/// `n` lights taking turns each lights up once every `n` steps.
fn finest_step(beat_hz: f32, n: usize, want: f32) -> f32 {
    let mut step = 1.0;
    for s in [2.0, 4.0] {
        if s <= want && beat_hz * s / n.max(1) as f32 <= MAX_SWING_HZ {
            step = s;
        }
    }
    step
}

/// One light's look under an effect: its palette position and brightness
/// (relative to the section's), `k` of `n` lights. The drive makes every
/// effect wilder in its own way: deeper dips between hits, faster movement,
/// quicker turns; always within three dark-to-bright swings a second.
fn render(effect: Effect, k: usize, n: usize, show: &Show, kicks: u32, drive: f32, alive: f32) -> (f32, f32) {
    let nf = n.max(1) as f32;
    let kf = k as f32;
    let beats = show.beats;
    let tau = std::f32::consts::TAU;
    let beat_hz = (show.bpm / 60.0).max(0.5);
    let d = drive.clamp(0.0, 1.0);
    let floor = drive_floor(d);
    // A swing a beat (a bump on each kick) is too fast above 180 BPM: then only a shimmer.
    let floor_beat = if beat_hz > MAX_SWING_HZ { 1.0 - SHIMMER_DEPTH } else { floor };
    // Faster movement, but a light's own rise and fall stays under three a second.
    let speed = drive_speed(d);
    let lift = |shape: f32, low: f32| low + (1.0 - low) * shape.clamp(0.0, 1.0);
    // The kick's bump: sharper (shorter) the wilder.
    let kick = show.pulse.clamp(0.0, 1.0).powf(0.7 + 0.8 * d);
    let (t, level) = match effect {
        Effect::Pulse => (kf / nf * 0.5 + beats / 64.0 * speed, lift(kick, floor_beat)),
        Effect::Pop => {
            // A new colour on each kick (every other kick when subtle), the lights apart.
            let every = if d < 0.3 { 2 } else { 1 };
            let step = (kicks as usize / every + k * 2) % 3;
            (step as f32 / 3.0 + beats / 128.0, lift(kick, floor_beat.max(floor + 0.1).min(1.0)))
        }
        Effect::Split => {
            // Swap every bar, every two beats when strong, every beat when wild.
            let span = if d > 0.85 { 1.0 } else if d > 0.6 { 2.0 } else { 4.0 };
            let span = if beat_hz / span > MAX_SWING_HZ { 4.0 } else { span };
            let swap = if ((beats / span).floor() as i64).rem_euclid(2) == 0 { 0.0 } else { 0.5 };
            ((kf * 0.5 + swap).fract() + beats / 128.0 * speed, lift(0.35 + 0.65 * kick, floor_beat))
        }
        Effect::Wave => {
            // The ripple: one rise and fall every two beats at normal, as fast
            // as the drive asks but never past three a second.
            let hz = (beat_hz / 2.0 * speed).min(MAX_SWING_HZ);
            let phase = beats / beat_hz * hz - kf / nf;
            (beats / 16.0 * speed + kf / nf * 0.5, lift(0.5 + 0.5 * (tau * phase).sin(), floor))
        }
        Effect::Chase => {
            // A beat a light, half-beats when strong (if safe), with the lights in turn.
            let want = if d > 0.65 { 2.0 } else { 1.0 };
            let sub = finest_step(beat_hz, n.max(2), want);
            let pos = beats * sub;
            let mine = n == 1 || (pos.floor() as i64).rem_euclid(n as i64) as usize == k;
            let fall = (1.0 - pos.fract()).powf(0.6 + 1.4 * d);
            let low = if n == 1 { floor_beat } else { floor * 0.6 };
            (beats / 64.0 * speed + (beats / 16.0).floor() * 0.25, if mine { lift(fall, low) } else { low })
        }
        Effect::Breathe => {
            // Over two bars at normal; four when subtle, one when wild.
            let period = (8.0 / speed).max(2.0);
            let b = 0.5 - 0.5 * (tau * (beats / period + kf / nf * 0.25)).cos();
            (beats / 32.0 * speed + kf / nf * 0.3, lift(b, floor.min(0.5)))
        }
        Effect::Riser => {
            let tension = show.tension.clamp(0.0, 1.0);
            // Quicker turns as the build climbs (sooner when wild), within the limit.
            let want = if tension > 0.8 - 0.3 * d { 4.0 } else if tension > 0.45 - 0.25 * d { 2.0 } else { 1.0 };
            let sub = finest_step(beat_hz, n.max(2), want);
            let pos = beats * sub;
            let mine = n == 1 || (pos.floor() as i64).rem_euclid(n as i64) as usize == k;
            let shimmer = SHIMMER_DEPTH * tension * (0.5 + 0.5 * (tau * beats * 8.0).sin());
            let low = if n == 1 { 1.0 - SHIMMER_DEPTH } else { floor.max(0.2) };
            let l = (0.45 + 0.55 * tension) * (if mine { 1.0 } else { low }) * (1.0 - shimmer);
            (beats / 16.0 * (1.0 + tension) * speed + kf / nf * 0.5, l)
        }
    };
    // Never quite still: a slow drift, different on every light.
    let drift = 0.06 * alive * (tau * (beats / 13.0 + kf * 0.37)).sin();
    (t + 0.02 * alive * (tau * (beats / 29.0 + kf * 0.61)).sin(), (level + drift).clamp(0.0, 1.0))
}

pub struct Engine {
    colours: Vec<[f32; 3]>,
    levels: Vec<f32>,
    base: f32,
    time: f32,
    smoke_on_until: f32,
    smoke_last: Option<f32>,
    last_mood: u8,
    build_peaked: bool,
    /// The smoke button is held.
    pub smoke_held: bool,
    /// Light `i` flashes white until this time (the Identify button).
    identify: Option<(usize, f32)>,
    /// The effect now, the one before it, and how far the change is (0..1).
    effect: Effect,
    previous: Effect,
    blend: f32,
    phrase: i64,
    mood: u8,
    rng: Rng,
    kicks: u32,
    last_pulse: f32,
    last_beat: i64,
    /// The lights' own flashes, limited like the screens'.
    strobe: crate::strobe::StrobeLimiter,
    hit: f32,
    /// The drive as the lights have it now: eased toward the setting, so a
    /// quick move of the slider is a smooth change on the lights.
    drive: Option<f32>,
}

impl Default for Engine {
    fn default() -> Self {
        Engine {
            colours: Vec::new(),
            levels: Vec::new(),
            base: 0.0,
            time: 0.0,
            smoke_on_until: 0.0,
            smoke_last: None,
            last_mood: 0,
            build_peaked: false,
            smoke_held: false,
            identify: None,
            effect: Effect::Pulse,
            previous: Effect::Pulse,
            blend: 1.0,
            phrase: i64::MIN,
            mood: u8::MAX,
            rng: Rng(0x9E37_79B9_7F4A_7C15),
            kicks: 0,
            last_pulse: 0.0,
            last_beat: 0,
            strobe: crate::strobe::StrobeLimiter::new(MAX_SWING_HZ, 1.0),
            hit: 0.0,
            drive: None,
        }
    }
}

impl Engine {
    /// Light `i` flashes white for two seconds, to find which one it is.
    pub fn identify(&mut self, i: usize) {
        self.identify = Some((i, self.time + 2.0));
    }

    /// The effect playing now.
    pub fn effect(&self) -> Effect {
        self.effect
    }

    /// Chooses the effect: the locked one, or Auto's on a new phrase or section.
    fn choose(&mut self, show: &Show, cfg: &Lighting, dt: f32) {
        let phrase = (show.beats / PHRASE).floor() as i64;
        let want = match cfg.effect {
            Some(e) => (e != self.effect).then_some(e),
            None => {
                let new_section = show.mood != self.mood;
                let new_phrase = phrase != self.phrase;
                let wrong = cfg.effect.is_none() && ((show.mood == 2) != (self.effect == Effect::Riser));
                (new_section || new_phrase || wrong).then(|| auto_effect(show.mood, show.intensity, Some(self.effect), self.rng.next()))
            }
        };
        self.mood = show.mood;
        self.phrase = phrase;
        if let Some(e) = want
            && e != self.effect
        {
            self.previous = self.effect;
            self.effect = e;
            self.blend = 0.0;
        }
        // The change takes about a beat.
        let beat = 60.0 / show.bpm.max(60.0);
        self.blend = (self.blend + dt / beat).min(1.0);
    }

    /// Each light's state for this frame, `dt` seconds after the last.
    pub fn update(&mut self, show: &Show, cfg: &Lighting, dt: f32) -> Vec<LightOut> {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
        self.time += dt;
        let n = cfg.fixtures.len();
        self.colours.resize(n, [0.0; 3]);
        self.levels.resize(n, 0.0);
        if show.pulse > 0.8 && self.last_pulse <= 0.8 {
            self.kicks = self.kicks.wrapping_add(1);
        }
        self.last_pulse = show.pulse;

        // How bright the room is in this section.
        let section = match show.mood {
            0 => 0.15,
            2 => 0.35 + 0.6 * show.tension.clamp(0.0, 1.0),
            3 => 1.0,
            4 => 0.4,
            _ => 0.65,
        };
        let drive = match self.drive {
            None => cfg.drive,
            Some(d) => d + (cfg.drive - d) * (1.0 - (-dt / 0.5).exp()),
        };
        self.drive = Some(drive);
        let section = if cfg.follow_vibe { (section + 0.15 * show.intensity).clamp(0.08, 1.0) } else { 0.85 };
        let target = drive_level(section, drive);
        self.base += (target - self.base) * (1.0 - (-dt / LEVEL_SECS).exp());
        let drop_began = show.mood == 3 && self.last_mood != 3;
        self.choose(show, cfg, dt);
        let smoke = self.smoke(show, cfg);

        // The lights' own white hits.
        let beat = show.beats.floor() as i64;
        let new_beat = beat != self.last_beat;
        self.last_beat = beat;
        let want_hit = match cfg.strobe {
            LightStrobe::Off => false,
            // Wilder: hits more often (the limiter still keeps them three a second at most).
            LightStrobe::Drops => drop_began || (show.mood == 3 && new_beat && beat.rem_euclid(if drive > 0.75 { 2 } else { 4 }) == 0),
            LightStrobe::Free => {
                drop_began
                    || (show.mood == 3 && new_beat && beat.rem_euclid(if drive > 0.75 { 1 } else { 2 }) == 0)
                    || (show.mood == 2 && show.tension > 0.85 - 0.25 * drive && new_beat)
            }
        };
        if want_hit {
            self.hit = 1.0;
        }
        let hit = self.strobe.update(self.hit, dt);
        // A hit is a quick white that dies away within a fraction of a beat.
        self.hit *= (-dt / 0.06).exp();

        let follow: Vec<usize> = (0..n).filter(|&i| cfg.fixtures[i].enabled && cfg.fixtures[i].role == Role::Follow && !cfg.fixtures[i].is_smoke()).collect();
        let chasers: Vec<usize> = (0..n).filter(|&i| cfg.fixtures[i].enabled && cfg.fixtures[i].role == Role::Chase && !cfg.fixtures[i].is_smoke()).collect();
        let saturate = drive_saturation(drive);
        let blend = self.blend * self.blend * (3.0 - 2.0 * self.blend);

        let mut out = Vec::with_capacity(n);
        for (i, f) in cfg.fixtures.iter().enumerate() {
            if f.is_smoke() {
                out.push(LightOut { fog: smoke, ..LightOut::default() });
                continue;
            }
            if !f.enabled || f.role == Role::Off {
                self.levels[i] = 0.0;
                out.push(LightOut::default());
                continue;
            }
            let ((t, level), fade) = match f.role {
                Role::Wash => {
                    let k = i;
                    ((k as f32 * 0.17 + show.beats / 64.0, 0.85), cfg.fade_secs * 3.0)
                }
                Role::Chase => {
                    let k = chasers.iter().position(|&j| j == i).unwrap_or(0);
                    (render(Effect::Chase, k, chasers.len(), show, self.kicks, drive, 1.0), drive_fade(cfg.fade_secs, drive))
                }
                _ => {
                    let k = follow.iter().position(|&j| j == i).unwrap_or(0);
                    let now = render(self.effect, k, follow.len(), show, self.kicks, drive, 1.0);
                    let before = render(self.previous, k, follow.len(), show, self.kicks, drive, 1.0);
                    let t = before.0 + (now.0 - before.0) * blend;
                    let l = before.1 + (now.1 - before.1) * blend;
                    ((t, l), self.effect.fade(drive_fade(cfg.fade_secs, drive)))
                }
            };
            let want = show.colour_vivid(t, saturate);
            let a = 1.0 - (-dt / fade.max(0.02)).exp();
            let c = &mut self.colours[i];
            for ch in 0..3 {
                c[ch] += (want[ch] - c[ch]) * a;
            }
            // Up at once (a hit is a hit), down smoothly: quicker the wilder,
            // never a hard cut.
            let want_level = (level * self.base).clamp(0.0, 1.0);
            if want_level > self.levels[i] {
                self.levels[i] = want_level;
            } else {
                let la = 1.0 - (-dt / (0.08 - 0.05 * drive)).exp();
                self.levels[i] += (want_level - self.levels[i]) * la;
            }
            let mut flash = if cfg.flashes { show.flash.clamp(0.0, 1.0) } else { 0.0 };
            flash = flash.max(hit);
            if let Some((j, until)) = self.identify
                && j == i
                && self.time < until
            {
                flash = if (self.time * 4.0).fract() < 0.5 { 1.0 } else { 0.0 };
            }
            let master = if cfg.blackout { 0.0 } else { cfg.master };
            out.push(LightOut { rgb: *c, level: self.levels[i] * f.brightness * master, flash: flash * f.brightness * master, fog: 0.0 });
        }
        self.last_mood = show.mood;
        out
    }

    /// Fog: by hand, or a burst on a drop or at the top of a build, never
    /// more often than the gap allows.
    fn smoke(&mut self, show: &Show, cfg: &Lighting) -> f32 {
        let now = self.time;
        if cfg.blackout {
            return 0.0;
        }
        match cfg.smoke {
            SmokeMode::Off => {
                self.smoke_on_until = 0.0;
                0.0
            }
            SmokeMode::Hand => {
                if self.smoke_held { cfg.smoke_amount } else { 0.0 }
            }
            SmokeMode::Auto => {
                let drop_began = show.mood == 3 && self.last_mood != 3;
                if show.mood != 2 {
                    self.build_peaked = false;
                }
                let peak = show.mood == 2 && show.tension > 0.9 && !self.build_peaked;
                let rested = self.smoke_last.is_none_or(|t| now - t >= cfg.smoke_gap_secs);
                if (drop_began || peak || self.smoke_held) && rested {
                    self.build_peaked |= peak;
                    self.smoke_last = Some(now);
                    self.smoke_on_until = now + cfg.smoke_secs;
                }
                if now < self.smoke_on_until { cfg.smoke_amount } else { 0.0 }
            }
        }
    }
}

/// The DMX values for the lights' states: each light's channels from its
/// start address. A light with a dimmer gets its colour at full and the
/// brightness on the dimmer (finer fades); one without gets the colour scaled.
pub fn universe(cfg: &Lighting, out: &[LightOut]) -> [u8; UNIVERSE] {
    let mut u = [0u8; UNIVERSE];
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    for (f, o) in cfg.fixtures.iter().zip(out) {
        let channels = f.channels();
        let has_dimmer = channels.contains(&Channel::Dimmer);
        let has_white = channels.contains(&Channel::White);
        let seen = o.seen();
        // Dark is dark on every channel: some lights glow on colour alone.
        if o.level <= 0.0 && o.flash <= 0.0 && o.fog <= 0.0 {
            continue;
        }
        // With a dimmer: colour at full, brightness (and flash) on the dimmer.
        let (colour, dimmer) = if has_dimmer {
            let total = (o.level + (1.0 - o.level) * o.flash).clamp(0.0, 1.0);
            let c = o.rgb.map(|v| v + (1.0 - v) * o.flash);
            (c, total)
        } else {
            (seen, 1.0)
        };
        // A white channel takes the flash; the colour stays the colour.
        let (colour, white) = if has_white {
            let c = if has_dimmer { o.rgb } else { o.rgb.map(|v| v * o.level) };
            (c, o.flash)
        } else {
            (colour, 0.0)
        };
        for (k, ch) in channels.iter().enumerate() {
            let at = f.address as usize - 1 + k;
            if at >= UNIVERSE {
                break;
            }
            u[at] = match ch {
                Channel::Dimmer => byte(dimmer),
                Channel::Red => byte(colour[0]),
                Channel::Green => byte(colour[1]),
                Channel::Blue => byte(colour[2]),
                Channel::White => byte(white),
                Channel::Amber => byte(colour[0].min(colour[1]) * 0.8),
                Channel::Uv => byte(colour[2].min(colour[0]) * 0.5),
                Channel::Strobe | Channel::Zero => 0,
                Channel::Fog => byte(o.fog),
                Channel::Full => 255,
            };
        }
    }
    u
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(mood: u8) -> Show {
        Show {
            // A palette that is red at t=0.
            palette: [[0.5, 0.2, 0.2, 0.0], [0.5, 0.2, 0.2, 0.0], [1.0, 1.0, 1.0, 0.0], [0.0, 0.33, 0.67, 0.0]],
            beats: 3.2,
            pulse: 0.0,
            bpm: 128.0,
            mood,
            tension: 0.0,
            flash: 0.0,
            intensity: 0.0,
        }
    }

    fn run(engine: &mut Engine, s: &Show, cfg: &Lighting, secs: f32) -> Vec<LightOut> {
        let mut out = Vec::new();
        for _ in 0..(secs * 60.0) as usize {
            out = engine.update(s, cfg, 1.0 / 60.0);
        }
        out
    }

    #[test]
    fn colours_fade_instead_of_jumping() {
        let cfg = Lighting::default();
        let mut e = Engine::default();
        let red = show(1);
        let first = e.update(&red, &cfg, 1.0 / 60.0);
        // One frame in, it has barely moved from black toward red.
        assert!(first[0].rgb[0] < 0.1, "{:?}", first[0].rgb);
        let settled = run(&mut e, &red, &cfg, 4.0);
        let want = red.colour(red.beats / 64.0);
        assert!((settled[0].rgb[0] - want[0]).abs() < 0.05, "{:?} vs {want:?}", settled[0].rgb);
    }

    #[test]
    fn a_drop_is_brighter_than_a_breakdown_and_kicks_bump() {
        let cfg = Lighting { effect: Some(Effect::Pulse), ..Lighting::default() };
        let mut e = Engine::default();
        let calm = run(&mut e, &show(4), &cfg, 5.0)[0].level;
        let mut e = Engine::default();
        let drop = run(&mut e, &show(3), &cfg, 5.0)[0].level;
        assert!(drop > calm * 1.8, "drop {drop} breakdown {calm}");
        let mut kick = show(3);
        kick.pulse = 1.0;
        let bumped = run(&mut e, &kick, &cfg, 0.3)[0].level;
        assert!(bumped > drop * 1.15, "bump {bumped} vs {drop}");
    }

    #[test]
    fn flashes_obey_the_setting_and_blackout_is_black() {
        let mut cfg = Lighting::default();
        let mut e = Engine::default();
        let mut s = show(1);
        s.flash = 1.0;
        assert_eq!(run(&mut e, &s, &cfg, 0.5)[0].flash, 1.0);
        cfg.flashes = false;
        assert_eq!(run(&mut e, &s, &cfg, 0.1)[0].flash, 0.0);
        cfg.blackout = true;
        let o = run(&mut e, &s, &cfg, 0.5)[0];
        assert_eq!(universe(&cfg, &[o]), [0u8; UNIVERSE]);
    }

    #[test]
    fn the_spot_gets_its_channels_from_its_address() {
        let cfg = Lighting { fixtures: vec![Fixture { address: 10, ..Fixture::default() }], ..Lighting::default() };
        let o = LightOut { rgb: [1.0, 0.5, 0.0], level: 0.5, flash: 0.0, fog: 0.0 };
        let u = universe(&cfg, &[o]);
        // Dimmer, R, G, B, strobe, mode, speed at 10..16.
        assert_eq!(&u[9..16], &[128, 255, 128, 0, 0, 0, 0]);
        assert!(u[..9].iter().all(|v| *v == 0) && u[16..].iter().all(|v| *v == 0));
        // Without a dimmer the colour carries the brightness.
        let cfg = Lighting { fixtures: vec![Fixture { profile: Profile::Rgb, ..Fixture::default() }], ..Lighting::default() };
        assert_eq!(&universe(&cfg, &[o])[0..3], &[128, 64, 0]);
    }

    #[test]
    fn chase_lights_take_turns_on_the_beat() {
        let cfg = Lighting {
            fixtures: (0..3).map(|i| Fixture { name: format!("{i}"), address: 1 + 7 * i, role: Role::Chase, ..Fixture::default() }).collect(),
            ..Lighting::default()
        };
        let mut e = Engine::default();
        let mut s = show(3);
        for beat in 0..6 {
            s.beats = beat as f32 + 0.05;
            let o = run(&mut e, &s, &cfg, 0.3);
            let brightest = (0..3).max_by(|&a, &b| o[a].level.total_cmp(&o[b].level)).unwrap();
            assert_eq!(brightest, beat % 3, "beat {beat}: {:?}", o.iter().map(|x| x.level).collect::<Vec<_>>());
        }
    }

    #[test]
    fn auto_smoke_bursts_on_a_drop_then_rests() {
        let mut cfg = Lighting::default();
        cfg.fixtures.push(Fixture { name: "Smoke".into(), address: 20, profile: Profile::Smoke, ..Fixture::default() });
        cfg.smoke = SmokeMode::Auto;
        cfg.smoke_secs = 2.0;
        cfg.smoke_gap_secs = 30.0;
        let mut e = Engine::default();
        run(&mut e, &show(1), &cfg, 1.0);
        let on = e.update(&show(3), &cfg, 1.0 / 60.0);
        assert_eq!(on[1].fog, 1.0);
        assert_eq!(run(&mut e, &show(3), &cfg, 3.0)[1].fog, 0.0, "the burst ends");
        // Another drop within the gap: no smoke.
        run(&mut e, &show(4), &cfg, 1.0);
        assert_eq!(e.update(&show(3), &cfg, 1.0 / 60.0)[1].fog, 0.0);
        // The smoke machine's channel.
        let o = e.update(&show(3), &cfg, 1.0 / 60.0);
        assert_eq!(universe(&cfg, &o)[19], 0);
    }

    #[test]
    fn addresses_overlap_and_next_is_free() {
        let mut cfg = Lighting::default();
        cfg.fixtures.push(Fixture { name: "Spot 2".into(), address: 5, ..Fixture::default() });
        assert_eq!(cfg.overlaps(), vec![(0, 1)]);
        cfg.fixtures[1].address = 8;
        assert!(cfg.overlaps().is_empty());
        assert_eq!(cfg.next_address(), 15);
        let v = Lighting { master: f32::NAN, fixtures: vec![Fixture { address: 900, ..Fixture::default() }], ..Lighting::default() }.validated();
        assert_eq!((v.master, v.fixtures[0].address), (1.0, 512));
    }

    fn two_spots() -> Lighting {
        Lighting {
            fixtures: vec![Fixture { name: "Spot 1".into(), address: 30, ..Fixture::default() }, Fixture { name: "Spot 2".into(), address: 40, ..Fixture::default() }],
            ..Lighting::default()
        }
    }

    /// Plays `secs` of steady music in `mood` at 128 BPM from `beats`.
    fn play(e: &mut Engine, cfg: &Lighting, mood: u8, tension: f32, from: f32, secs: f32, mut each: impl FnMut(f32, &[LightOut], Effect)) {
        let dt = 1.0 / 60.0;
        let mut s = show(mood);
        s.tension = tension;
        for k in 0..(secs / dt) as usize {
            let t = k as f32 * dt;
            s.beats = from + t * 128.0 / 60.0;
            s.pulse = (-(s.beats.fract() * 60.0 / 128.0) / 0.15).exp();
            let o = e.update(&s, cfg, dt);
            each(t, &o, e.effect());
        }
    }

    #[test]
    fn auto_changes_effect_only_on_a_phrase_or_a_section_and_never_repeats() {
        let cfg = two_spots();
        let mut e = Engine::default();
        let mut changes = Vec::new();
        let mut last = None;
        play(&mut e, &cfg, 1, 0.0, 0.0, 60.0, |t, _, eff| {
            if last != Some(eff) {
                changes.push((t * 128.0 / 60.0, eff));
                last = Some(eff);
            }
        });
        assert!(changes.len() >= 3, "{changes:?}");
        for w in changes.windows(2).skip(1) {
            assert_ne!(w[0].1, w[1].1);
            // On a phrase boundary (every 16 beats), within a frame.
            let at = w[1].0.rem_euclid(16.0);
            assert!(!(0.1..=15.9).contains(&at), "changed at beat {}", w[1].0);
        }
        // A build is always the Riser, a breakdown calm.
        play(&mut e, &cfg, 2, 0.5, 300.0, 1.0, |_, _, _| {});
        assert_eq!(e.effect(), Effect::Riser);
        play(&mut e, &cfg, 4, 0.0, 400.0, 1.0, |_, _, _| {});
        assert!(matches!(e.effect(), Effect::Breathe | Effect::Wave));
        // Locked: stays.
        let locked = Lighting { effect: Some(Effect::Split), ..two_spots() };
        play(&mut e, &locked, 3, 0.0, 500.0, 20.0, |_, _, eff| assert!(eff == Effect::Split || eff == Effect::Breathe || eff == Effect::Wave));
        assert_eq!(e.effect(), Effect::Split);
    }

    #[test]
    fn no_light_swings_dark_to_bright_more_than_three_times_a_second() {
        // Every effect, the build at its height (the riser at its fastest).
        for effect in Effect::ALL {
            let cfg = Lighting { effect: Some(effect), drive: 1.0, strobe: LightStrobe::Off, ..two_spots() };
            let mut e = Engine::default();
            let mut levels: Vec<Vec<f32>> = vec![Vec::new(); 2];
            play(&mut e, &cfg, 2, 1.0, 0.0, 6.0, |t, o, _| {
                if t > 1.0 {
                    for (k, l) in levels.iter_mut().enumerate() {
                        l.push(o[k].level);
                    }
                }
            });
            for l in &levels {
                // Count rises of more than a third of the range.
                let (lo, hi) = l.iter().fold((1f32, 0f32), |(a, b), v| (a.min(*v), b.max(*v)));
                let swing = (hi - lo) / 3.0;
                // A swing: up by more than `swing`, then down by as much
                // before the next one counts.
                let mut rises = 0;
                let (mut low, mut high, mut up) = (l[0], l[0], false);
                for v in l {
                    if up {
                        high = high.max(*v);
                        if high - v > swing {
                            up = false;
                            low = *v;
                        }
                    } else {
                        low = low.min(*v);
                        if v - low > swing && swing > 0.05 {
                            rises += 1;
                            up = true;
                            high = *v;
                        }
                    }
                }
                assert!(rises as f32 / 5.0 <= 3.05, "{effect:?}: {} swings a second", rises as f32 / 5.0);
            }
        }
    }

    #[test]
    fn strobe_hits_land_on_drops_and_never_more_than_three_a_second() {
        let cfg = Lighting { strobe: LightStrobe::Free, ..two_spots() };
        let mut e = Engine::default();
        play(&mut e, &cfg, 1, 0.0, 0.0, 2.0, |_, o, _| assert_eq!(o[0].flash, 0.0, "no hits in a groove"));
        let mut starts = Vec::new();
        let mut was = 0.0;
        play(&mut e, &cfg, 3, 0.0, 100.0, 8.0, |t, o, _| {
            if o[0].flash > 0.05 && was <= 0.05 {
                starts.push(t);
            }
            was = o[0].flash;
        });
        assert!(starts.first().is_some_and(|t| *t < 0.05), "a hit as the drop lands: {starts:?}");
        for w in starts.windows(2) {
            assert!(w[1] - w[0] >= 1.0 / 3.0, "{starts:?}");
        }
        let off = Lighting { strobe: LightStrobe::Off, flashes: false, ..two_spots() };
        let mut e = Engine::default();
        play(&mut e, &off, 3, 0.0, 0.0, 4.0, |_, o, _| assert_eq!(o[0].flash, 0.0));
    }

    #[test]
    fn effects_cross_fade_without_jumps() {
        let mut cfg = Lighting { effect: Some(Effect::Breathe), ..two_spots() };
        let mut e = Engine::default();
        play(&mut e, &cfg, 1, 0.0, 0.0, 3.0, |_, _, _| {});
        cfg.effect = Some(Effect::Chase);
        let mut last: Option<f32> = None;
        let mut worst = 0f32;
        play(&mut e, &cfg, 1, 0.0, 6.4, 0.4, |_, o, _| {
            if let Some(l) = last {
                worst = worst.max((o[0].level - l).abs());
            }
            last = Some(o[0].level);
        });
        assert!(worst < 0.2, "a jump of {worst} in one frame");
    }

    #[test]
    fn drive_goes_from_subtle_to_wild() {
        // Brightness: subtle is softer and capped, wild lifts the calm parts.
        assert!(drive_level(1.0, 0.0) < 0.6);
        assert_eq!(drive_level(0.65, 0.5), 0.65);
        assert!(drive_level(0.4, 1.0) > 0.8);
        // It only ever rises with the drive.
        for s10 in 0..=10 {
            let s = s10 as f32 / 10.0;
            let mut last = -1.0;
            for d10 in 0..=20 {
                let v = drive_level(s, d10 as f32 / 20.0);
                assert!(v >= last - 1e-6 && v <= 1.0);
                last = v;
            }
        }
        // On the lights: a groove at wild is brighter, its bumps deeper.
        let run_at = |drive: f32| {
            let cfg = Lighting { effect: Some(Effect::Pulse), drive, ..two_spots() };
            let mut e = Engine::default();
            let mut levels = Vec::new();
            play(&mut e, &cfg, 1, 0.0, 0.0, 6.0, |t, o, _| {
                if t > 4.0 {
                    levels.push(o[0].level);
                }
            });
            let (lo, hi) = levels.iter().fold((1f32, 0f32), |(a, b), v| (a.min(*v), b.max(*v)));
            (hi, hi - lo)
        };
        let (soft_top, soft_swing) = run_at(0.0);
        let (wild_top, wild_swing) = run_at(1.0);
        assert!(wild_top > soft_top + 0.3, "{soft_top} {wild_top}");
        assert!(wild_swing > soft_swing * 2.0, "{soft_swing} {wild_swing}");
    }

    #[test]
    fn wild_is_faster_and_deeper_in_every_effect() {
        // Over 8 s of a groove: how far each effect swings, and how often.
        let measure = |effect: Effect, drive: f32| {
            let cfg = Lighting { effect: Some(effect), drive, strobe: LightStrobe::Off, ..two_spots() };
            let mut e = Engine::default();
            let mut l = Vec::new();
            play(&mut e, &cfg, 3, 0.0, 0.0, 10.0, |t, o, _| {
                if t > 2.0 {
                    l.push(o[0].level);
                }
            });
            let (lo, hi) = l.iter().fold((1f32, 0f32), |(a, b), v| (a.min(*v), b.max(*v)));
            let mean = l.iter().sum::<f32>() / l.len() as f32;
            let crossings = l.windows(2).filter(|w| (w[0] - mean).signum() != (w[1] - mean).signum()).count();
            ((hi - lo) / hi.max(1e-3), crossings)
        };
        for effect in [Effect::Pulse, Effect::Pop, Effect::Split, Effect::Wave, Effect::Chase, Effect::Breathe] {
            let (soft_depth, soft_moves) = measure(effect, 0.0);
            let (wild_depth, wild_moves) = measure(effect, 1.0);
            assert!(wild_depth > soft_depth + 0.2, "{effect:?}: depth {soft_depth} → {wild_depth}");
            if matches!(effect, Effect::Wave | Effect::Breathe | Effect::Chase) {
                assert!(wild_moves > soft_moves, "{effect:?}: moves {soft_moves} → {wild_moves}");
            }
        }
        assert!(drive_fade(0.6, 1.0) < drive_fade(0.6, 0.5) && drive_fade(0.6, 0.5) < drive_fade(0.6, 0.0));
    }

    #[test]
    fn moving_the_drive_slider_is_a_smooth_change() {
        let mut cfg = Lighting { effect: Some(Effect::Breathe), drive: 0.0, ..two_spots() };
        let mut e = Engine::default();
        play(&mut e, &cfg, 1, 0.0, 0.0, 3.0, |_, _, _| {});
        cfg.drive = 1.0;
        let mut last: Option<f32> = None;
        let mut worst = 0f32;
        play(&mut e, &cfg, 1, 0.0, 6.4, 1.0, |_, o, _| {
            if let Some(l) = last {
                worst = worst.max((o[0].level - l).abs());
            }
            last = Some(o[0].level);
        });
        assert!(worst < 0.06, "a jump of {worst} in one frame");
    }

    #[test]
    fn at_fast_tempos_kicks_shimmer_instead_of_flashing() {
        let cfg = Lighting { effect: Some(Effect::Pulse), drive: 1.0, ..two_spots() };
        let mut e = Engine::default();
        let mut s = show(3);
        s.bpm = 200.0;
        let mut levels = Vec::new();
        for k in 0..360 {
            s.beats = k as f32 / 60.0 * 200.0 / 60.0;
            s.pulse = (-(s.beats.fract() * 0.3) / 0.15).exp();
            let o = e.update(&s, &cfg, 1.0 / 60.0);
            if k > 120 {
                levels.push(o[0].level);
            }
        }
        // Within any one beat (18 frames at 200 BPM) the light barely moves;
        // only the slow drift goes further, over many beats.
        for w in levels.windows(18) {
            let (lo, hi) = w.iter().fold((1f32, 0f32), |(a, b), v| (a.min(*v), b.max(*v)));
            assert!(hi - lo <= hi * 0.1, "swing {lo}..{hi} within a beat");
        }
    }

    #[test]
    fn colours_are_vivid() {
        let c = vivid([0.6, 0.5, 0.45]);
        assert!(c[0] > 0.99 && c[2] < 0.7, "{c:?}");
        assert_eq!(vivid([0.0; 3]), [1.0; 3]);
    }
}
