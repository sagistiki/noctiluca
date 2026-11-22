//! The block of numbers sent to the GPU every frame.

use bytemuck::{Pod, Zeroable};

use crate::analysis::{Features, Mood};
use crate::director::{DirectorOut, PALETTES, POST_PRESETS};

/// Must match `struct U` in shaders/common.wgsl field for field. Everything
/// is a group of four floats, which sidesteps the GPU's alignment rules.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    pub res_time: [f32; 4],
    pub bands: [f32; 4],
    pub beat: [f32; 4],
    pub state: [f32; 4],
    pub misc: [f32; 4],
    pub flow: [f32; 4],
    pub palette: [[f32; 4]; 4],
    pub post: [[f32; 4]; 3],
    pub spectrum: [[f32; 4]; 16],
    pub wave: [[f32; 4]; 64],
    /// [label opacity, label length in characters, treatment strength,
    /// transition (0 fade, 1 ink, 2 zoom through)]
    /// (the treatment itself, VHS, thermal or film, is post[2][1])
    pub overlay: [f32; 4],
    /// A short line of text shown on screen (see `encode_label`).
    pub text: [[f32; 4]; 8],
    /// [lightning light (already limited), bolt shape, crystal burst, camera shake]
    pub events: [f32; 4],
    /// [brightness, intensity, texture, how sure]: the vibe (analysis/vibe.rs).
    pub vibe: [f32; 4],
    /// [drops so far (wraps at 64), seconds since the last drop, unused,
    /// unused]: lets an object keep a change a drop made (the dice's face).
    pub stage: [f32; 4],
}

/// Most characters a label can hold.
pub const LABEL_CHARS: usize = 32;

/// Turns text into what the shader's 3x5 font can draw: upper case, one
/// number per character (its place in the font, which starts at the space).
/// Characters the font lacks become "?", curly apostrophes straight ones,
/// and text that is too long ends in "...".
pub fn encode_label(text: &str) -> ([[f32; 4]; 8], usize) {
    let mut chars: Vec<char> = text
        .chars()
        .map(|c| match c.to_ascii_uppercase() {
            '\u{2019}' | '\u{2018}' | '`' => '\'',
            c if (' '..='_').contains(&c) => c,
            _ => '?',
        })
        .collect();
    if chars.len() > LABEL_CHARS {
        chars.truncate(LABEL_CHARS - 3);
        chars.extend(['.', '.', '.']);
    }
    let mut out = [[0.0; 4]; 8];
    for (i, c) in chars.iter().enumerate() {
        out[i / 4][i % 4] = (*c as u32 - ' ' as u32) as f32;
    }
    (out, chars.len())
}

/// Clocks that run faster when the music is louder. A shader that multiplied
/// plain time by the bass level would jump backwards whenever the bass fell;
/// adding up small steps here keeps the motion continuous.
/// Wind a clock back once it passes this, on a frame that hides it...
const WRAP_AT: f32 = 2048.0;
/// ...or once it passes this, hidden or not.
const FORCE_WRAP_AT: f32 = 4096.0;
/// A multiple of 1024, the distance after which `hash21` in common.wgsl repeats.
const WRAP_BY: f32 = 2048.0;

#[derive(Default)]
pub struct Flow {
    bass: f32,
    mid: f32,
    level: f32,
    /// Drops heard so far, and the seconds since the last one.
    drops: u32,
    since_drop: f32,
}

impl Flow {
    /// Clocks that start as if the app had already been running a while.
    /// For checking how the visuals hold up late in the night.
    pub fn starting_at(value: f32) -> Self {
        Self {
            bass: value,
            mid: value,
            level: value,
            ..Self::default()
        }
    }
}

impl Flow {
    /// `covered` says the picture is changing anyway on this frame (a drop,
    /// the start of a cross-fade), so a small jump would go unseen.
    pub fn advance(&mut self, f: &Features, dt: f32, covered: bool) {
        self.bass += dt * (0.15 + 0.9 * f.bands[1].max(f.bands[0]));
        self.mid += dt * (0.3 + 1.2 * f.bands[2]);
        self.level += dt * (0.2 + f.level);
        if f.drop {
            self.drops = self.drops.wrapping_add(1);
            self.since_drop = 0.0;
        } else {
            self.since_drop = (self.since_drop + dt).min(600.0);
        }

        // A GPU works in 32-bit floats: the bigger a number, the coarser its
        // steps. Left alone these clocks pass 20,000 in a night and the
        // visuals lose their fine detail and start to stutter. So they are
        // wound back, by a distance after which the tiled backgrounds look
        // identical, and preferably at a moment when nobody can tell.
        for clock in [&mut self.bass, &mut self.mid, &mut self.level] {
            if *clock >= FORCE_WRAP_AT || (covered && *clock >= WRAP_AT) {
                *clock -= WRAP_BY;
            }
        }
    }
}

pub struct FrameInfo<'a> {
    pub width: u32,
    pub height: u32,
    pub time: f32,
    pub dt: f32,
    /// How visible the data overlay is, 0..1.
    pub hud: f32,
    /// The look is locked; shown in the overlay.
    pub locked: bool,
    /// What the director wants on screen.
    pub show: DirectorOut,
    /// Flash brightness, already through the safety limiter.
    pub strobe: f32,
    /// Text to show on screen, and how visible it is (0..1).
    pub label: Option<(&'a str, f32)>,
    /// See `Uniforms::events`.
    pub events: [f32; 4],
}

/// The kick pulse fades like this (as in sync.rs).
const PULSE_RELEASE_SECS: f32 = 0.15;

impl Uniforms {
    /// The picture as it should be `secs` from now, for a screen that shows
    /// it that much later (a network screen): the clock and the beat grid
    /// move on by `secs`, and the kick pulse is the one due then. What reacts
    /// to the sound itself (levels, spectrum, the strobe) cannot be known
    /// ahead and stays as it is.
    pub fn ahead(&self, secs: f32) -> Uniforms {
        let mut u = *self;
        if !(secs > 0.0 && secs.is_finite()) {
            return u;
        }
        u.res_time[2] += secs;
        let bpm = u.beat[3];
        if bpm > 0.0 {
            let moved = secs * bpm / 60.0;
            let phase = u.beat[2] + moved;
            let whole = phase.floor();
            u.beat[2] = phase - whole;
            u.misc[1] = (u.misc[1] + whole).rem_euclid(1024.0);
            u.flow[3] = (u.flow[3] + moved).rem_euclid(1024.0);
            // A beat crossed on the way: the pulse fires there and fades (as the
            // beat clock's does) while the kicks are landing (a live pulse now).
            u.beat[1] = if whole >= 1.0 && u.beat[1] > 0.02 {
                let since = u.beat[2] * 60.0 / bpm;
                (-since / PULSE_RELEASE_SECS).exp()
            } else {
                u.beat[1] * (-secs / PULSE_RELEASE_SECS).exp()
            };
        }
        u
    }

    pub fn build(frame: &FrameInfo, f: &Features, flow: &Flow) -> Self {
        let mood = match f.mood {
            Mood::Silence => 0.0,
            Mood::Groove => 1.0,
            Mood::Build => 2.0,
            Mood::Drop => 3.0,
            Mood::Breakdown => 4.0,
        };
        let beats = (f.beat_count % 1024) as f32 + f.beat_phase;
        let (from, to, mix) = (
            frame.show.from.wrapped(),
            frame.show.to.wrapped(),
            frame.show.mix,
        );
        // Palettes and effects are just numbers, so a cross-fade blends the numbers.
        let palette = blend(PALETTES[from.palette], PALETTES[to.palette], mix);
        let mut post = blend(POST_PRESETS[from.post], POST_PRESETS[to.post], mix);
        // Except the kaleidoscope's mirror count: 0 to 6 by way of 2.7 mirrors
        // looks broken, so that one switches halfway.
        let switch = if mix < 0.5 { from.post } else { to.post };
        post[0][3] = POST_PRESETS[switch][0][3];
        // The same for the treatment (VHS, thermal, film): one goes out over
        // the first half of the fade and the next comes in over the second.
        let (treat_from, treat_to) = (POST_PRESETS[from.post][2][1], POST_PRESETS[to.post][2][1]);
        post[2][1] = POST_PRESETS[switch][2][1];
        let treatment = if treat_from == treat_to {
            1.0
        } else if mix < 0.5 {
            1.0 - 2.0 * mix
        } else {
            2.0 * mix - 1.0
        };
        let (text, overlay) = match frame.label {
            Some((label, opacity)) => {
                let (text, len) = encode_label(label);
                (text, [opacity.clamp(0.0, 1.0), len as f32, treatment, frame.show.transition.code()])
            }
            None => ([[0.0; 4]; 8], [0.0, 0.0, treatment, frame.show.transition.code()]),
        };
        Self {
            res_time: [
                frame.width as f32,
                frame.height as f32,
                frame.time,
                frame.dt,
            ],
            bands: f.bands,
            beat: [f.level, f.pulse, f.beat_phase, f.bpm],
            state: [mood, mix, frame.show.tension, frame.strobe],
            misc: [
                frame.hud.clamp(0.0, 1.0),
                (f.beat_count % 1024) as f32,
                f.confidence,
                frame.locked as u8 as f32,
            ],
            flow: [flow.bass, flow.mid, flow.level, beats],
            palette,
            post,
            spectrum: bytemuck::cast(f.spectrum),
            wave: bytemuck::cast(f.wave),
            overlay,
            text,
            events: frame.events,
            vibe: [
                f.vibe.brightness,
                f.vibe.intensity,
                f.vibe.texture,
                f.vibe.sure,
            ],
            stage: [(flow.drops % 64) as f32, flow.since_drop, 0.0, 0.0],
        }
    }
}

/// How long the wet floor takes to come in or go out.
pub const FLOOR_FADE_SECS: f32 = 3.0;

/// Moves `current` toward `target` at an even pace that covers the whole
/// range of the floor (0 to its full strength of 0.55) in `secs`: a slow,
/// steady fade rather than a jump when the look changes.
pub fn fade_toward(current: f32, target: f32, dt: f32, secs: f32) -> f32 {
    let step = 0.55 * dt / secs.max(1e-3);
    current + (target - current).clamp(-step, step)
}

/// Blends two tables of numbers: `a` at t = 0, `b` at t = 1.
fn blend<const N: usize>(a: [[f32; 4]; N], b: [[f32; 4]; N], t: f32) -> [[f32; 4]; N] {
    let mut out = a;
    for (row, target) in out.iter_mut().zip(b) {
        for (v, w) in row.iter_mut().zip(target) {
            // Written this way, not as v + (w - v) * t, so that t = 1 gives
            // exactly `w` with no rounding left over.
            *v = *v * (1.0 - t) + w * t;
        }
    }
    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_late_screen_gets_the_picture_due_when_it_shows() {
        let mut u: Uniforms = bytemuck::Zeroable::zeroed();
        u.res_time[2] = 10.0;
        u.beat = [0.5, 0.8, 0.9, 120.0]; // 120 BPM, just before a beat, kicks landing
        u.misc[1] = 7.0;
        u.flow[3] = 7.9;
        // 150 ms later at 2 beats a second is 0.3 beats: past the next beat.
        let a = u.ahead(0.15);
        assert!((a.res_time[2] - 10.15).abs() < 1e-5);
        assert!((a.beat[2] - 0.2).abs() < 1e-4 && a.misc[1] == 8.0 && (a.flow[3] - 8.2).abs() < 1e-4);
        assert!((a.beat[1] - (-0.1f32 / 0.15).exp()).abs() < 1e-4, "the pulse of the beat crossed");
        // No tempo: only the clock moves; no lead: nothing changes.
        u.beat[3] = 0.0;
        let b = u.ahead(0.15);
        assert_eq!(b.beat[2], 0.9);
        assert_eq!(bytemuck::bytes_of(&u.ahead(0.0)), bytemuck::bytes_of(&u));
        assert_eq!(bytemuck::bytes_of(&u.ahead(f32::NAN)), bytemuck::bytes_of(&u));
    }

    use super::*;
    use crate::director::Look;

    fn frame(from: Look, to: Look, mix: f32) -> FrameInfo<'static> {
        FrameInfo {
            width: 1280,
            height: 720,
            time: 0.0,
            dt: 0.016,
            hud: 0.0,
            locked: false,
            show: DirectorOut {
                from,
                to,
                mix,
                flash: 0.0,
                tension: 0.0,
                theme: 0,
                complement_from: from,
                complement_to: to,
                transition: crate::director::Transition::Fade,
            },
            strobe: 0.0,
            label: None,
            events: [0.0; 4],
        }
    }

    /// Back from font codes to text, for checking.
    fn decode(text: [[f32; 4]; 8], len: usize) -> String {
        (0..len)
            .map(|i| char::from_u32(text[i / 4][i % 4] as u32 + ' ' as u32).unwrap())
            .collect()
    }

    #[test]
    fn device_names_become_drawable_text() {
        let (text, len) = encode_label("Dana\u{2019}s iPhone Microphone");
        assert_eq!(decode(text, len), "DANA'S IPHONE MICROPHONE");
        let (text, len) = encode_label("Mic (2): ×€");
        assert_eq!(decode(text, len), "MIC (2): ??");
        // Every code is inside the 64-character font.
        assert!(text.iter().flatten().all(|c| (0.0..64.0).contains(c)));
    }

    #[test]
    fn long_names_are_cut_with_dots() {
        let (text, len) = encode_label("Scarlett 18i20 USB Multichannel Line Input 3-4");
        assert_eq!(len, LABEL_CHARS);
        assert!(decode(text, len).ends_with("..."), "{}", decode(text, len));
    }

    #[test]
    fn a_label_reaches_the_shader() {
        let mut info = frame(Look::default(), Look::default(), 1.0);
        info.label = Some(("input 1/2: mac", 0.7));
        let u = Uniforms::build(&info, &Features::default(), &Flow::default());
        assert_eq!(u.overlay[0], 0.7);
        assert_eq!(u.overlay[1], 14.0);
        let without = Uniforms::build(
            &frame(Look::default(), Look::default(), 1.0),
            &Features::default(),
            &Flow::default(),
        );
        assert_eq!(without.overlay[0], 0.0);
    }

    #[test]
    fn the_floor_fades_in_and_out_over_a_few_seconds() {
        let dt = 1.0 / 60.0;
        let mut floor = 0.0;
        let mut steps = 0;
        while floor < 0.55 {
            floor = fade_toward(floor, 0.55, dt, FLOOR_FADE_SECS);
            steps += 1;
            assert!(steps < 10_000);
        }
        let secs = steps as f32 * dt;
        assert!(
            (secs - FLOOR_FADE_SECS).abs() < 0.05,
            "came in over {secs} s"
        );
        assert_eq!(
            fade_toward(0.55, 0.55, dt, FLOOR_FADE_SECS),
            0.55,
            "stays put once there"
        );
        let going = fade_toward(0.55, 0.0, dt, FLOOR_FADE_SECS);
        assert!(going < 0.55 && going > 0.5, "goes out gently: {going}");
    }

    #[test]
    fn size_matches_the_shader_struct() {
        // Read `struct U` out of the shader itself and add up its fields:
        // every one is a vec4f or an array of them.
        let wgsl =
            std::fs::read_to_string(crate::render::shaders::default_dir().join("common.wgsl"))
                .unwrap();
        let body = wgsl
            .split("struct U {")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        let mut vec4s = 0;
        for line in body
            .lines()
            .map(|l| l.split("//").next().unwrap().trim())
            .filter(|l| !l.is_empty())
        {
            let ty = line.split(':').nth(1).unwrap().trim().trim_end_matches(',');
            vec4s += match ty.strip_prefix("array<vec4f,") {
                Some(n) => n.trim_end_matches('>').trim().parse::<usize>().unwrap(),
                None => {
                    assert_eq!(ty, "vec4f", "unexpected field type in struct U: {line}");
                    1
                }
            };
        }
        assert_eq!(size_of::<Uniforms>(), vec4s * 16);
    }

    /// Runs the clocks for `hours` at 60 fps with the music at a steady level.
    /// `cover_every` is how often, in seconds, a look change or drop comes by.
    fn run_flow(hours: f32, cover_every: Option<f32>) -> (Flow, f32, usize) {
        let f = Features {
            bands: [0.6; 4],
            level: 0.6,
            ..Default::default()
        };
        let (mut flow, dt) = (Flow::default(), 1.0 / 60.0);
        let (mut highest, mut uncovered_wraps) = (0.0f32, 0);
        let mut since_cover = 0.0;
        for _ in 0..(hours * 3600.0 * 60.0) as usize {
            since_cover += dt;
            let covered = cover_every.is_some_and(|every| since_cover >= every);
            if covered {
                since_cover = 0.0;
            }
            let before = flow.bass;
            flow.advance(&f, dt, covered);
            if flow.bass < before && !covered {
                uncovered_wraps += 1;
            }
            highest = highest.max(flow.bass).max(flow.mid).max(flow.level);
        }
        (flow, highest, uncovered_wraps)
    }

    #[test]
    fn clocks_stay_small_all_night() {
        // Big numbers lose their fine detail on a GPU, and the visuals with
        // them. Ten hours in, the clocks must be no bigger than in hour one.
        let (flow, highest, _) = run_flow(10.0, None);
        assert!(highest < 4200.0, "a clock reached {highest}");
        assert!(flow.bass.is_finite() && flow.mid.is_finite() && flow.level.is_finite());
    }

    #[test]
    fn clocks_wrap_under_cover_when_they_can() {
        // A wrap can show as a small jump, so it should happen on a frame
        // where the picture is changing anyway.
        let (_, highest, uncovered_wraps) = run_flow(10.0, Some(7.5));
        assert_eq!(uncovered_wraps, 0);
        assert!(highest < 2100.0, "a clock reached {highest}");
    }

    #[test]
    fn a_wrap_moves_a_clock_by_a_whole_number_of_tiles() {
        // The tiled backgrounds repeat every 1024 tiles, so a clock that
        // jumps back by a multiple of that lands on an identical picture.
        let f = Features {
            bands: [0.6; 4],
            level: 0.6,
            ..Default::default()
        };
        let mut flow = Flow::default();
        let mut prev = flow.bass;
        for _ in 0..(3.0 * 3600.0 * 60.0) as usize {
            flow.advance(&f, 1.0 / 60.0, false);
            if flow.bass < prev {
                let jump = prev - flow.bass;
                assert!(
                    (jump % 1024.0).min(1024.0 - jump % 1024.0) < 0.1,
                    "jumped by {jump}"
                );
            }
            prev = flow.bass;
        }
    }

    #[test]
    fn cross_fade_blends_palette_and_effect() {
        let a = Look::default();
        let b = Look {
            bg: 1,
            pattern: 1,
            palette: 2,
            post: 3,
            ..Look::default()
        };
        let (f, flow) = (Features::default(), Flow::default());
        assert_eq!(
            Uniforms::build(&frame(a, b, 0.0), &f, &flow).palette,
            PALETTES[0]
        );
        assert_eq!(
            Uniforms::build(&frame(a, b, 1.0), &f, &flow).palette,
            PALETTES[2]
        );
        let half = Uniforms::build(&frame(a, b, 0.5), &f, &flow);
        assert_eq!(
            half.palette[0][0],
            (PALETTES[0][0][0] + PALETTES[2][0][0]) / 2.0
        );
        // The mirror count never takes an in-between value.
        let early = Uniforms::build(&frame(a, b, 0.4), &f, &flow);
        assert_eq!(early.post[0][3], POST_PRESETS[0][0][3]);
        assert_eq!(half.post[0][3], POST_PRESETS[3][0][3]);
    }

    #[test]
    fn treatments_go_out_then_the_next_comes_in() {
        use crate::director::POST_NAMES;
        let post = |name| POST_NAMES.iter().position(|n| *n == name).unwrap();
        let vhs = Look {
            post: post("vhs"),
            ..Look::default()
        };
        let film = Look {
            post: post("film"),
            ..Look::default()
        };
        let clean = Look::default();
        let (f, flow) = (Features::default(), Flow::default());
        let at = |a, b, mix| {
            let u = Uniforms::build(&frame(a, b, mix), &f, &flow);
            (u.post[2][1], u.overlay[2])
        };
        // VHS to film: VHS fades out, nothing at the middle, film fades in.
        assert_eq!(at(vhs, film, 0.0), (1.0, 1.0));
        assert_eq!(at(vhs, film, 0.25), (1.0, 0.5));
        assert_eq!(at(vhs, film, 0.75), (3.0, 0.5));
        assert_eq!(at(vhs, film, 1.0), (3.0, 1.0));
        // The treatment number never takes an in-between value.
        for i in 0..=20 {
            let (which, _) = at(vhs, film, i as f32 / 20.0);
            assert!(which == 1.0 || which == 3.0);
        }
        // From nothing, it just fades in; and holding one keeps it full.
        assert_eq!(at(clean, vhs, 0.75), (1.0, 0.5));
        assert_eq!(at(vhs, vhs, 0.3), (1.0, 1.0));
        assert_eq!(at(clean, clean, 0.3).0, 0.0);
    }

    #[test]
    fn out_of_range_looks_do_not_panic() {
        let wild = Look {
            bg: 99,
            pattern: 99,
            palette: 99,
            post: 99,
            object: 99,
            material: 99,
            atmos: 99,
        };
        Uniforms::build(
            &frame(wild, wild, 0.5),
            &Features::default(),
            &Flow::default(),
        );
    }
}
