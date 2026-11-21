//! What is on screen: which background, pattern, palette and effect preset,
//! and the director that changes them with the music.

use crate::analysis::{Features, Mood, Vibe};

pub const BACKGROUNDS: [&str; 5] = ["tunnel", "grid", "liquid", "shapes", "void"];
pub const PATTERNS: [&str; 9] = [
    "rings", "scope", "kaleido", "bars", "stripes", "frames", "radar", "spiral", "halftone",
];
pub const PALETTE_NAMES: [&str; 10] = [
    "neon",
    "acid",
    "fire",
    "ice",
    "ultraviolet",
    "red",
    "white",
    "terminal",
    "violet",
    "amber",
];
/// The 3D objects, by the name of their file in assets/models. "none" is no object.
pub const OBJECTS: [&str; 13] = [
    "none",
    "mirrorball",
    "skull",
    "prism",
    "gem",
    "knot",
    "eye",
    "monolith",
    "chain",
    "speakers",
    "crystal",
    "planet",
    "corridor",
];
pub const MATERIALS: [&str; 2] = ["chrome", "glass"];
/// The atmosphere over everything: weather, fire, light. "storm" is clouds
/// with lightning. "none" is no atmosphere.
pub const ATMOSPHERES: [&str; 6] = ["none", "storm", "smoke", "fire", "beams", "aurora"];
pub const POST_NAMES: [&str; 11] = [
    "clean", "trails", "zoom", "kaleido", "drift", "crt", "prism", "floor", "vhs", "thermal",
    "film",
];

/// Cosine palettes (a, b, c, d): colour(t) = a + b * cos(2π(c·t + d)).
/// Four numbers describe a whole endless gradient, and blending two palettes
/// is just blending the numbers.
pub const PALETTES: [[[f32; 4]; 4]; 10] = [
    // neon: magenta and cyan over deep blue
    [
        [0.50, 0.30, 0.55, 0.0],
        [0.50, 0.35, 0.45, 0.0],
        [1.0, 1.0, 0.5, 0.0],
        [0.80, 0.20, 0.55, 0.0],
    ],
    // acid: toxic yellow-green out of black
    [
        [0.38, 0.50, 0.08, 0.0],
        [0.38, 0.50, 0.10, 0.0],
        [1.0, 1.0, 1.0, 0.0],
        [0.58, 0.50, 0.20, 0.0],
    ],
    // fire: red, orange, a little yellow
    [
        [0.60, 0.28, 0.08, 0.0],
        [0.40, 0.28, 0.08, 0.0],
        [1.0, 1.0, 1.0, 0.0],
        [0.00, 0.08, 0.20, 0.0],
    ],
    // ice: blue and cyan up to white
    [
        [0.25, 0.50, 0.70, 0.0],
        [0.25, 0.40, 0.30, 0.0],
        [1.0, 1.0, 0.5, 0.0],
        [0.50, 0.55, 0.10, 0.0],
    ],
    // ultraviolet: violet and hot pink
    [
        [0.50, 0.15, 0.65, 0.0],
        [0.50, 0.15, 0.35, 0.0],
        [1.0, 1.0, 1.0, 0.0],
        [0.00, 0.50, 0.15, 0.0],
    ],
    // The club palettes: one colour on black, as rooms are lit.
    mono(1.00, 0.05, 0.03), // red
    mono(0.92, 0.94, 1.00), // white
    mono(0.30, 1.00, 0.25), // terminal green
    mono(0.55, 0.12, 1.00), // violet
    mono(1.00, 0.50, 0.08), // amber
];

/// A palette that is one colour at varying brightness: as t goes round,
/// it sweeps from a tenth of the colour up to all of it.
const fn mono(r: f32, g: f32, b: f32) -> [[f32; 4]; 4] {
    [
        [0.55 * r, 0.55 * g, 0.55 * b, 0.0],
        [0.45 * r, 0.45 * g, 0.45 * b, 0.0],
        [1.0, 1.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
    ]
}

/// Effect presets. Each is three groups of four numbers, read by
/// shaders/post.wgsl and shaders/present.wgsl:
///   [trail seconds, zoom per second, rotation per second, kaleidoscope segments]
///   [colour split, wobble, glow, extra zoom on each kick]
///   [wet floor, unused, CRT dither, prism]
pub const POST_PRESETS: [[[f32; 4]; 3]; 11] = [
    // clean: the scene as it is
    [
        [0.00, 0.00, 0.00, 0.0],
        [0.002, 0.000, 0.6, 0.0],
        [0.00, 0.0, 0.0, 0.0],
    ],
    // trails: everything leaves an echo
    [
        [0.35, 0.00, 0.00, 0.0],
        [0.003, 0.000, 0.7, 0.0],
        [0.00, 0.0, 0.0, 0.0],
    ],
    // zoom: echoes fly outward, harder on every kick
    [
        [0.50, 0.90, 0.15, 0.0],
        [0.004, 0.000, 0.7, 1.5],
        [0.00, 0.0, 0.0, 0.0],
    ],
    // kaleido: six-fold mirror
    [
        [0.25, 0.30, -0.10, 6.0],
        [0.003, 0.000, 0.8, 0.8],
        [0.00, 0.0, 0.0, 0.0],
    ],
    // drift: long, slow, liquid echoes for breakdowns
    [
        [1.20, 0.25, 0.08, 0.0],
        [0.006, 0.004, 1.0, 0.0],
        [0.00, 0.0, 0.0, 0.0],
    ],
    // crt: one-bit dots on an old monitor
    [
        [0.15, 0.00, 0.00, 0.0],
        [0.000, 0.000, 0.9, 0.0],
        [0.00, 0.0, 1.0, 0.0],
    ],
    // prism: the picture seen through cut glass, split into a rainbow
    [
        [0.20, 0.20, 0.05, 0.0],
        [0.000, 0.000, 0.8, 0.5],
        [0.00, 0.0, 0.0, 1.0],
    ],
    // floor: the scene standing on a wet floor that mirrors it. A special
    // look, used now and then by a few themes, not all the time.
    [
        [0.00, 0.00, 0.00, 0.0],
        [0.002, 0.000, 0.6, 0.0],
        [0.55, 0.0, 0.0, 0.0],
    ],
    // vhs: a worn tape: wobbling lines, colours slipping apart, tape noise
    [
        [0.20, 0.00, 0.00, 0.0],
        [0.002, 0.000, 0.7, 0.0],
        [0.00, 1.0, 0.0, 0.0],
    ],
    // thermal: a heat camera: brightness becomes heat, black to white through red
    [
        [0.25, 0.00, 0.00, 0.0],
        [0.002, 0.000, 0.6, 0.0],
        [0.00, 2.0, 0.0, 0.0],
    ],
    // film: grain, a shaking gate, flicker, dust and scratches
    [
        [0.15, 0.00, 0.00, 0.0],
        [0.002, 0.000, 0.6, 0.0],
        [0.00, 3.0, 0.0, 0.0],
    ],
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    pub bg: usize,
    pub pattern: usize,
    pub palette: usize,
    pub post: usize,
    /// Index into OBJECTS; 0 is no object.
    pub object: usize,
    /// Index into MATERIALS.
    pub material: usize,
    /// Index into ATMOSPHERES; 0 is none.
    pub atmos: usize,
}

impl Look {
    /// Wraps every index into range, so any numbers make a valid look.
    pub fn wrapped(self) -> Self {
        Self {
            bg: self.bg % BACKGROUNDS.len(),
            pattern: self.pattern % PATTERNS.len(),
            palette: self.palette % PALETTES.len(),
            post: self.post % POST_PRESETS.len(),
            object: self.object % OBJECTS.len(),
            material: self.material % MATERIALS.len(),
            atmos: self.atmos % ATMOSPHERES.len(),
        }
    }
}

/// Hands out values like cards from a shuffled deck: every value comes up
/// about once per round, and none comes back until most of the others have
/// had their turn, even across the join between two decks.
struct Bag {
    n: usize,
    queue: Vec<usize>,
    /// The last few values dealt, newest last; none of them may come next.
    recent: Vec<usize>,
    /// For `deal_vibe`: deals so far, and the deal at which each value last came.
    dealt: u64,
    last: Vec<u64>,
}

impl Bag {
    fn new(n: usize) -> Self {
        Self {
            n,
            queue: Vec::new(),
            recent: Vec::new(),
            dealt: 0,
            last: vec![0; n],
        }
    }

    /// The next value that is neither `current` (the value on screen) nor
    /// one of the last n - 2 dealt. Skipped values stay in the deck.
    fn next(&mut self, current: usize, rng: &mut Rng) -> usize {
        assert!(self.n >= 2, "a deck needs at least two values to change");
        let allowed = |v: &usize, recent: &[usize]| *v != current && !recent.contains(v);
        loop {
            if let Some(i) = self.queue.iter().position(|v| allowed(v, &self.recent)) {
                let value = self.queue.remove(i);
                self.recent.push(value);
                if self.recent.len() > self.n - 2 {
                    self.recent.remove(0);
                }
                return value;
            }
            // Nothing usable left: shuffle a fresh deck in behind what is left.
            // A fresh deck holds all n values and at most n - 1 are barred, so
            // this always ends.
            let mut deck: Vec<usize> = (0..self.n).collect();
            for i in (1..self.n).rev() {
                let j = rng.below(i + 1);
                deck.swap(i, j); // Fisher-Yates: every order equally likely
            }
            self.queue.extend(deck);
        }
    }
}

impl Bag {
    /// A value from `allowed`, other than `current`, chosen by `weight` (how
    /// well it suits the vibe) but still like a deck: the `hold` most
    /// recently dealt choices sit out, and the longer a value has waited the
    /// stronger its claim, so even one far from the vibe comes round.
    fn deal_vibe(
        &mut self,
        current: usize,
        allowed: &[usize],
        hold: usize,
        weight: impl Fn(usize) -> f32,
        rng: &mut Rng,
    ) -> usize {
        let options: Vec<usize> = allowed
            .iter()
            .copied()
            .filter(|v| *v != current && *v < self.n)
            .collect();
        if options.is_empty() {
            return current;
        }
        let hold = hold.min(options.len() - 1);
        let held: Vec<usize> = self
            .recent
            .iter()
            .rev()
            .filter(|v| options.contains(v))
            .take(hold)
            .copied()
            .collect();
        let open: Vec<usize> = options
            .iter()
            .copied()
            .filter(|v| !held.contains(v))
            .collect();
        self.dealt += 1;
        // A wait as long as two rounds of all the choices doubles a claim.
        let (dealt, last, waits) = (self.dealt, &self.last, 2.0 * options.len() as f32);
        let pick = rng.weighted(&open, |v| {
            weight(v) * (1.0 + (dealt - last[v]) as f32 / waits)
        });
        self.last[pick] = self.dealt;
        self.recent.push(pick);
        if self.recent.len() > self.n {
            self.recent.remove(0);
        }
        pick
    }
}

/// What to draw this frame: usually one look, or two while cross-fading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectorOut {
    pub from: Look,
    pub to: Look,
    /// 0 shows `from`, 1 shows `to`.
    pub mix: f32,
    /// Flash the visuals would like, 0..1. Goes through the strobe limiter.
    pub flash: f32,
    /// How far the picture has closed in during a build-up, 0..1. Snaps back
    /// to 0 on the drop.
    pub tension: f32,
    /// Index into THEMES.
    pub theme: usize,
    /// The look for Complement surfaces (a projector with several), cross-
    /// fading with the same `mix`.
    pub complement_from: Look,
    pub complement_to: Look,
}

/// Objects whose compositions are walls: a Complement surface shows the
/// same wall, so it carries on beside the main one. (Speakers, skulls, eyes.)
pub const WALL_OBJECTS: [usize; 3] = [9, 2, 6];

/// A set of choices that belong together, like a VJ's scene for one part
/// of the night: a colour, a few backgrounds and patterns, an effect style,
/// a few objects. Every list holds indices into the tables above.
pub struct Theme {
    pub name: &'static str,
    pub palettes: &'static [usize],
    pub backgrounds: &'static [usize],
    pub patterns: &'static [usize],
    pub posts: &'static [usize],
    /// Never 0: "no object" is always allowed and needs no listing.
    pub objects: &'static [usize],
    /// Atmospheres, 0 (none) included where the theme may go without.
    pub atmos: &'static [usize],
}

// Index reminders:
//   backgrounds  0 tunnel  1 grid  2 liquid  3 shapes  4 void
//   patterns     0 rings  1 scope  2 kaleido  3 bars  4 stripes  5 frames  6 radar  7 spiral  8 halftone
//   palettes     0 neon  1 acid  2 fire  3 ice  4 ultraviolet  5 red  6 white  7 terminal  8 violet  9 amber
//   effects      0 clean  1 trails  2 zoom  3 kaleido  (4 drift)  5 crt  6 prism  7 floor  8 vhs  9 thermal  10 film
//   atmos        0 none  1 storm  2 smoke  3 fire  4 beams  5 aurora
//   objects      1 mirror ball  2 skull  3 prism  4 gem  5 knot  6 eye  7 monolith  8 chain
//                9 speakers  10 crystal  11 planet  12 corridor
pub const THEMES: [Theme; 6] = [
    Theme {
        name: "red room",
        palettes: &[5],
        backgrounds: &[4, 0],
        patterns: &[4, 5, 3, 7],
        posts: &[0, 1, 6, 7, 8],
        objects: &[1, 2, 7, 8, 9, 12],
        atmos: &[0, 1, 3, 4],
    },
    Theme {
        name: "white corridor",
        palettes: &[6],
        backgrounds: &[4, 3],
        patterns: &[5, 0, 4, 7, 2],
        posts: &[0, 1, 3, 7, 10],
        objects: &[1, 4, 3, 5, 10, 12],
        atmos: &[0, 4, 2, 5],
    },
    Theme {
        name: "terminal",
        palettes: &[7],
        backgrounds: &[4, 1, 2],
        patterns: &[1, 4, 5, 6],
        posts: &[5, 8],
        objects: &[6, 2, 1, 9, 12],
        atmos: &[0, 2, 1],
    },
    Theme {
        name: "ultraviolet",
        palettes: &[8, 4],
        backgrounds: &[3, 2, 4],
        patterns: &[2, 0, 5, 8, 7],
        posts: &[6, 3, 1, 9],
        objects: &[4, 5, 6, 3, 10, 11],
        atmos: &[0, 5, 4, 2],
    },
    Theme {
        name: "amber",
        palettes: &[9, 2],
        backgrounds: &[0, 1, 4],
        patterns: &[4, 3, 1, 8, 6],
        posts: &[0, 2, 1, 10],
        objects: &[7, 8, 5, 9, 11],
        atmos: &[0, 3, 2],
    },
    Theme {
        name: "rainbow",
        palettes: &[0, 1, 2, 3, 4],
        backgrounds: &[0, 1, 2, 3],
        patterns: &[0, 1, 2, 3, 6, 7, 8],
        posts: &[1, 2, 3, 6, 8, 9, 10],
        objects: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        atmos: &[0, 1, 2, 3, 4, 5],
    },
];

/// Where each thing sits on the vibe map (see src/analysis/vibe.rs):
/// (brightness, intensity), each -1..1. Dark and dramatic things sit to the
/// left, bright and colourful ones to the right; calm ones low, intense ones
/// high. The director prefers what sits near the music's vibe.
type Spot = (f32, f32);

/// red room, white corridor, terminal, ultraviolet, amber, rainbow
pub const THEME_VIBE: [Spot; 6] = [
    (-0.8, 0.8),
    (0.6, -0.4),
    (-0.6, 0.2),
    (-0.4, -0.6),
    (0.0, 0.1),
    (0.8, 0.8),
];
/// neon, acid, fire, ice, ultraviolet, red, white, terminal, violet, amber
const PALETTE_VIBE: [Spot; 10] = [
    (0.6, 0.5),
    (0.7, 0.8),
    (0.0, 0.7),
    (0.5, -0.5),
    (-0.3, -0.3),
    (-0.8, 0.7),
    (0.8, 0.0),
    (-0.5, 0.0),
    (-0.5, -0.5),
    (0.0, -0.2),
];
/// tunnel, grid, liquid, shapes, void
const BG_VIBE: [Spot; 5] = [
    (0.0, 0.6),
    (0.3, 0.4),
    (0.3, -0.6),
    (0.2, -0.2),
    (-0.7, 0.0),
];
/// rings, scope, kaleido, bars, stripes, frames, radar, spiral, halftone
const PATTERN_VIBE: [Spot; 9] = [
    (0.3, -0.4),
    (0.0, 0.0),
    (0.7, 0.3),
    (0.3, 0.6),
    (-0.2, 0.9),
    (-0.4, 0.5),
    (-0.6, -0.2),
    (0.2, 0.3),
    (0.6, -0.2),
];
/// clean, trails, zoom, kaleido, drift, crt, prism, floor, vhs, thermal, film
const POST_VIBE: [Spot; 11] = [
    (0.0, -0.3),
    (0.2, -0.3),
    (0.3, 0.8),
    (0.6, 0.4),
    (0.0, -1.0),
    (-0.6, 0.2),
    (0.8, 0.3),
    (0.4, -0.6),
    (-0.3, 0.4),
    (-0.5, 0.5),
    (0.0, -0.5),
];
/// none, mirror ball, skull, prism, gem, knot, eye, monolith, chain,
/// speakers, crystal, planet, corridor. "None" sits with calm music.
const OBJECT_VIBE: [Spot; 13] = [
    (0.0, -0.9),
    (0.9, 0.4),
    (-0.8, 0.6),
    (0.7, 0.0),
    (0.8, -0.4),
    (0.2, -0.6),
    (-0.5, -0.5),
    (-0.7, 0.3),
    (-0.6, 0.8),
    (0.0, 0.9),
    (0.6, -0.3),
    (0.0, -0.8),
    (-0.2, 0.6),
];
/// none, storm, smoke, fire, beams, aurora
const ATMOS_VIBE: [Spot; 6] = [
    (0.0, -0.9),
    (-0.7, 0.6),
    (-0.6, -0.5),
    (-0.3, 0.9),
    (0.4, 0.8),
    (0.6, -0.6),
];

/// The least a thing far from the vibe still weighs against one right on it:
/// the unexpected keeps happening and nothing disappears for a night.
const VIBE_FLOOR: f32 = 0.15;
/// How far on the map a thing can be and still count as near.
const VIBE_SPREAD: f32 = 0.7;
/// How much an object prefers to make way for "no object" in calm music,
/// and to stay in intense music: the chance an object is followed by none
/// is 0.5 minus this times the intensity.
const FILL_BY_INTENSITY: f32 = 0.25;

/// The vibe as the director weighs it: where the music is, and how much a
/// thing far from it still counts (the "variety" setting).
#[derive(Clone, Copy, Debug)]
struct Taste {
    vibe: Vibe,
    floor: f32,
}

impl From<&Vibe> for Taste {
    fn from(vibe: &Vibe) -> Self {
        Taste {
            vibe: *vibe,
            floor: VIBE_FLOOR,
        }
    }
}

impl From<&Taste> for Taste {
    fn from(t: &Taste) -> Self {
        *t
    }
}

/// How much the vibe likes a thing at `at`: 1 right on it, down to the
/// floor (VIBE_FLOOR unless set) far away. Before the vibe is known,
/// everything weighs 1.
fn vibe_weight(taste: impl Into<Taste>, at: Spot) -> f32 {
    let Taste { vibe, floor } = taste.into();
    let vibe = &vibe;
    let d2 = (vibe.brightness - at.0).powi(2) + (vibe.intensity - at.1).powi(2);
    let fit = floor + (1.0 - floor) * (-d2 / (VIBE_SPREAD * VIBE_SPREAD)).exp();
    let sure = if vibe.sure.is_finite() {
        vibe.sure.clamp(0.0, 1.0)
    } else {
        0.0
    };
    1.0 + (fit - 1.0) * sure
}

/// A phrase: the unit techno is built in. Something changes at the end of each.
const PHRASE_BEATS: u64 = 16;
const FADE_BEATS: f32 = 2.0;
/// A drop cuts instead of fading.
const DROP_FADE_SECS: f32 = 0.5;
const FLASH_SECS: f32 = 0.12;
/// After a drop the first kicks flash too, at this strength.
const DROP_FLASH_BEATS: u64 = 8;
const DROP_KICK_FLASH: f32 = 0.6;
/// ...and never for longer than this, in case no tempo is known and the
/// beats are not being counted at all.
const DROP_FLASH_SECS: f32 = 5.0;

/// Without a trustworthy tempo for this long, change on a timer instead.
/// "Trustworthy": either the tracker is sure, or it has a tempo and kicks
/// are still landing. Through a room microphone the bass line makes the
/// tracker unsure of a tempo that is right; the kicks show that it is.
const TIMER_CONFIDENCE: f32 = 0.3;
const TIMER_AFTER_SECS: f32 = 4.0;
const KICKS_KEEP_THE_BEAT_SECS: f32 = 4.0;
const TIMER_CHANGE_SECS: f32 = 20.0;

/// The effect reserved for breakdowns, and the energetic ones a drop prefers.
const DRIFT: usize = 4;
const DROP_POSTS: [usize; 4] = [1, 2, 3, 6];
/// A new track: the vibe has moved at least this far on the map from where
/// it was when the theme began...
const NEW_TRACK_DISTANCE: f32 = 0.7;
/// ...the theme no longer suits it (its weight under this)...
const NEW_TRACK_MISFIT: f32 = 0.6;
/// ...and it has stayed moved this long. Then the theme changes at the next
/// phrase, the gentle way, over this many beats.
const NEW_TRACK_SECS: f32 = 20.0;
const NEW_TRACK_FADE_BEATS: f32 = 8.0;
/// A theme gets at least this many phrases before a new track may end it.
const MIN_THEME_PHRASES: u32 = 4;

/// A theme lasts this many phrases (two minutes at 128 BPM), or until a drop.
const THEME_PHRASES: u32 = 16;

/// A build-up closes the picture in over this long...
const TENSION_RISE_SECS: f32 = 8.0;
/// ...up to this far...
const MAX_TENSION: f32 = 0.85;
/// ...and if it ends without a drop, opens it again over this long.
const TENSION_FALL_SECS: f32 = 0.4;

/// What the menu lets one choose about the director.
#[derive(Clone, Debug, PartialEq)]
pub struct DirectorOptions {
    /// One per theme: in rotation or not. None in rotation means all.
    pub themes: Vec<bool>,
    /// Off: the vibe is ignored and the decks deal as they did before it.
    pub follow_vibe: bool,
    /// How much a thing far from the vibe still counts (0.05..0.5).
    pub variety: f32,
}

impl Default for DirectorOptions {
    fn default() -> Self {
        Self {
            themes: vec![true; THEMES.len()],
            follow_vibe: true,
            variety: VIBE_FLOOR,
        }
    }
}

/// Decides what is on screen and when it changes, like a VJ would:
///
/// * it works within a theme (a colour, a few backgrounds, patterns, effects
///   and objects) for about two minutes, then moves to the next;
/// * every 16 beats one thing changes, and every 32 it is the background;
/// * a breakdown slows everything into long drifting echoes;
/// * a drop starts a new theme and replaces everything at once, with a flash, and restarts the count
///   so that later changes line up with the music's own phrases;
/// * with no beat to follow, it changes something every 20 seconds.
///
/// It only looks at `Features`, never at audio or the GPU, so a whole night
/// can be simulated in a test in milliseconds.
pub struct Director {
    rng: Rng,
    /// Its own random numbers, so the complements do not change the main show.
    complement_rng: Rng,
    complement_from: Look,
    complement_to: Look,
    theme_bag: Bag,
    pattern_bag: Bag,
    object_bag: Bag,
    theme: usize,
    /// Changes made in the current theme, counted in phrases.
    phrases_in_theme: u32,
    tension: f32,
    from: Look,
    to: Look,
    /// 0..1 through the current cross-fade; 1 when settled.
    progress: f32,
    fade_secs: f32,
    beat_secs: f32,
    locked: bool,
    prev_mood: Mood,
    /// The effect to return to when a breakdown ends without a drop.
    saved_post: Option<usize>,
    /// The beat phrases are counted from.
    phrase_origin: u64,
    last_phrase: u64,
    drop_beat: Option<u64>,
    since_drop_secs: f32,
    drop_flash: f32,
    low_confidence_for: f32,
    since_timer_change: f32,
    since_kick: f32,
    /// The music's vibe, as last heard.
    vibe: Vibe,
    /// The vibe when the current theme began (followed until it is sure).
    theme_vibe: Vibe,
    /// How long the vibe has been far from `theme_vibe`.
    moved_for: f32,
    options: DirectorOptions,
}

impl Director {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut theme_bag = Bag::new(THEMES.len());
        let theme = theme_bag.next(usize::MAX, &mut rng);
        let t = &THEMES[theme];
        let mut pick = |list: &[usize]| list[rng.below(list.len())];
        let look = Look {
            bg: pick(t.backgrounds),
            pattern: pick(t.patterns),
            palette: pick(t.palettes),
            post: pick(t.posts),
            object: 0,
            material: 0,
            atmos: pick(t.atmos),
        };
        Self {
            rng,
            complement_rng: Rng::new(seed ^ 0x00c0_ffee),
            complement_from: look,
            complement_to: look,
            theme_bag,
            pattern_bag: Bag::new(PATTERNS.len()),
            object_bag: Bag::new(OBJECTS.len()),
            theme,
            phrases_in_theme: 0,
            tension: 0.0,
            from: look,
            to: look,
            progress: 1.0,
            fade_secs: 1.0,
            beat_secs: 0.5,
            locked: false,
            prev_mood: Mood::Silence,
            saved_post: None,
            phrase_origin: 0,
            last_phrase: 0,
            drop_beat: None,
            since_drop_secs: f32::INFINITY,
            drop_flash: 0.0,
            low_confidence_for: 0.0,
            since_timer_change: 0.0,
            since_kick: f32::INFINITY,
            vibe: Vibe::default(),
            theme_vibe: Vibe::default(),
            moved_for: 0.0,
            options: DirectorOptions::default(),
        }
    }

    /// Starts from the given look instead of a random one.
    pub fn with_look(mut self, look: Look) -> Self {
        self.from = look.wrapped();
        self.to = self.from;
        self.complement_to = self.complement_of(self.to);
        self.complement_from = self.complement_to;
        self
    }

    pub fn update(&mut self, f: &Features, dt: f32) -> DirectorOut {
        self.vibe = if self.options.follow_vibe {
            f.vibe
        } else {
            Vibe::default()
        };
        self.listen_for_a_new_track(dt);
        if self.progress < 1.0 {
            self.progress = (self.progress + dt / self.fade_secs).min(1.0);
            if self.progress >= 1.0 {
                self.from = self.to;
            }
        }
        if f.bpm > 0.0 {
            self.beat_secs = 60.0 / f.bpm;
        }
        let fade = FADE_BEATS * self.beat_secs;

        if f.drop {
            // The drop is beat one of a new phrase.
            self.phrase_origin = f.beat_count;
            self.last_phrase = 0;
            self.drop_beat = Some(f.beat_count);
            self.since_drop_secs = 0.0;
            self.drop_flash = 1.0;
            self.saved_post = None;
            if !self.locked {
                self.new_theme();
                let look = self.all_new(true);
                self.go(look, DROP_FADE_SECS);
            }
        }

        let lows_gone = matches!(f.mood, Mood::Breakdown | Mood::Build);
        let lows_were_gone = matches!(self.prev_mood, Mood::Breakdown | Mood::Build);
        self.prev_mood = f.mood;
        if !self.locked && !f.drop {
            if lows_gone && !lows_were_gone {
                self.saved_post = Some(self.to.post);
                self.go(
                    Look {
                        post: DRIFT,
                        ..self.to
                    },
                    fade,
                );
            } else if !lows_gone
                && lows_were_gone
                && let Some(post) = self.saved_post.take()
            {
                self.go(Look { post, ..self.to }, fade);
            }
        }

        let phrase = f.beat_count.saturating_sub(self.phrase_origin) / PHRASE_BEATS;
        let phrase_ended = phrase != self.last_phrase;
        self.last_phrase = phrase;

        self.since_kick = if f.kick { 0.0 } else { self.since_kick + dt };
        let kicks_landing = f.bpm > 0.0 && self.since_kick < KICKS_KEEP_THE_BEAT_SECS;
        let low = f.confidence < TIMER_CONFIDENCE && !kicks_landing;
        self.low_confidence_for = if low {
            self.low_confidence_for + dt
        } else {
            0.0
        };
        if self.low_confidence_for >= TIMER_AFTER_SECS {
            self.since_timer_change += dt;
            if self.since_timer_change >= TIMER_CHANGE_SECS {
                self.since_timer_change = 0.0;
                if !self.locked {
                    self.phrases_in_theme += 2; // a timer change is about two phrases long
                    let look = if self.phrases_in_theme >= THEME_PHRASES {
                        self.theme_change()
                    } else {
                        self.one_new(true, !lows_gone)
                    };
                    self.go(look, fade);
                }
            }
        } else {
            self.since_timer_change = 0.0;
            if phrase_ended && !self.locked && !f.drop {
                self.phrases_in_theme += 1;
                let new_track = self.new_track_heard();
                let look = if new_track {
                    self.theme_change()
                } else if !phrase.is_multiple_of(2) {
                    self.one_new(false, !lows_gone)
                } else if self.phrases_in_theme >= THEME_PHRASES {
                    self.theme_change()
                } else {
                    let t = &THEMES[self.theme];
                    Look {
                        bg: self.rng.other_by_vibe(
                            self.to.bg,
                            t.backgrounds,
                            &BG_VIBE,
                            &self.taste(),
                        ),
                        ..self.to
                    }
                };
                // A new track is eased into slowly: no sudden turn of mood.
                let fade = if new_track {
                    NEW_TRACK_FADE_BEATS * self.beat_secs
                } else {
                    fade
                };
                self.go(look, fade);
            }
        }

        let after_drop = self.drop_beat.map(|b| f.beat_count.saturating_sub(b));
        let kick_flash = match after_drop {
            // Short and sharp (pulse squared), and weaker with every beat, so
            // the drop lands as one impact that settles instead of eight white-outs.
            Some(beats) if beats < DROP_FLASH_BEATS && self.since_drop_secs < DROP_FLASH_SECS => {
                let by_beats = 1.0 - beats as f32 / DROP_FLASH_BEATS as f32;
                let by_time = 1.0 - self.since_drop_secs / DROP_FLASH_SECS;
                DROP_KICK_FLASH * f.pulse * f.pulse * by_beats.min(by_time)
            }
            _ => 0.0,
        };
        self.since_drop_secs += dt;
        if f.drop {
            self.tension = 0.0;
        } else if f.mood == Mood::Build {
            self.tension = (self.tension + MAX_TENSION * dt / TENSION_RISE_SECS).min(MAX_TENSION);
        } else {
            self.tension = (self.tension - MAX_TENSION * dt / TENSION_FALL_SECS).max(0.0);
        }
        let flash = self.drop_flash.max(kick_flash).clamp(0.0, 1.0);
        self.drop_flash *= (-dt / FLASH_SECS).exp();
        if self.drop_flash < 1e-3 {
            self.drop_flash = 0.0; // an exponential fade never reaches zero by itself
        }

        // Ease in and out rather than fading at a constant rate.
        let t = self.progress;
        DirectorOut {
            from: self.from,
            to: self.to,
            mix: t * t * (3.0 - 2.0 * t),
            flash,
            tension: self.tension,
            theme: self.theme,
            complement_from: self.complement_from,
            complement_to: self.complement_to,
        }
    }

    pub fn set_options(&mut self, options: DirectorOptions) {
        let variety = if options.variety.is_finite() {
            options.variety.clamp(0.05, 0.5)
        } else {
            VIBE_FLOOR
        };
        self.options = DirectorOptions { variety, ..options };
    }

    fn taste(&self) -> Taste {
        Taste {
            vibe: self.vibe,
            floor: self.options.variety,
        }
    }

    /// The themes in rotation (all of them if none is).
    fn enabled_themes(&self) -> Vec<usize> {
        let on: Vec<usize> = (0..THEMES.len())
            .filter(|t| self.options.themes.get(*t).copied().unwrap_or(true))
            .collect();
        if on.is_empty() {
            (0..THEMES.len()).collect()
        } else {
            on
        }
    }

    /// Changes everything now, into a new theme.
    pub fn next(&mut self) {
        self.new_theme();
        let look = self.all_new(false);
        self.go(look, FADE_BEATS * self.beat_secs);
    }

    pub fn theme(&self) -> usize {
        self.theme
    }

    fn new_theme(&mut self) {
        let (v, all) = (self.taste(), self.enabled_themes());
        // With the vibe known, only the theme on screen sits out, so dark
        // music may go back and forth between the two dark themes. Without it,
        // the themes take turns like a deck, every one before any comes back.
        let sure = if v.vibe.sure.is_finite() {
            v.vibe.sure.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let hold = ((1.0 - sure) * all.len().saturating_sub(2) as f32).round() as usize;
        self.theme = self.theme_bag.deal_vibe(
            self.theme,
            &all,
            hold,
            // Squared: the theme sets the whole tone, so it follows the vibe
            // more firmly than the single parts inside it do.
            |t| vibe_weight(v, THEME_VIBE[t]).powi(2),
            &mut self.rng,
        );
        self.phrases_in_theme = 0;
        self.theme_vibe = self.vibe;
        self.moved_for = 0.0;
    }

    /// Keeps count of how long the music has sounded unlike it did when the
    /// theme began, and unlike the theme itself: the sign of a new track.
    fn listen_for_a_new_track(&mut self, dt: f32) {
        let v = self.vibe;
        if !(v.sure.is_finite() && v.brightness.is_finite() && v.intensity.is_finite()) {
            return;
        }
        // Until the vibe is sure, the theme's vibe follows it.
        if self.theme_vibe.sure < 0.9 {
            self.theme_vibe = v;
            self.moved_for = 0.0;
            return;
        }
        let distance = ((v.brightness - self.theme_vibe.brightness).powi(2)
            + (v.intensity - self.theme_vibe.intensity).powi(2))
        .sqrt();
        let misfit = vibe_weight(&v, THEME_VIBE[self.theme]) < NEW_TRACK_MISFIT;
        self.moved_for = if distance > NEW_TRACK_DISTANCE && misfit && v.sure > 0.9 {
            self.moved_for + dt
        } else {
            0.0
        };
    }

    fn new_track_heard(&self) -> bool {
        self.moved_for >= NEW_TRACK_SECS && self.phrases_in_theme >= MIN_THEME_PHRASES
    }

    /// The gentle way into a new theme, at a phrase: only the background and
    /// the colour change now; the rest follows over the next phrases.
    fn theme_change(&mut self) -> Look {
        self.new_theme();
        let t = &THEMES[self.theme];
        Look {
            bg: self
                .rng
                .other_by_vibe(self.to.bg, t.backgrounds, &BG_VIBE, &self.taste()),
            palette: self.rng.other_by_vibe(
                self.to.palette,
                t.palettes,
                &PALETTE_VIBE,
                &self.taste(),
            ),
            ..self.to
        }
    }

    pub fn next_palette(&mut self) {
        self.edit(|l| l.palette += 1);
    }

    /// Changes the look by hand, with a normal cross-fade.
    pub fn edit(&mut self, change: impl FnOnce(&mut Look)) {
        let mut look = self.to;
        change(&mut look);
        self.go(look, FADE_BEATS * self.beat_secs);
    }

    /// While locked the look never changes on its own.
    pub fn toggle_lock(&mut self) -> bool {
        self.locked = !self.locked;
        self.locked
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    /// The look being shown, or faded to.
    pub fn look(&self) -> Look {
        self.to
    }

    /// Starts a cross-fade to `look`.
    fn go(&mut self, look: Look, fade_secs: f32) {
        let look = look.wrapped();
        if look == self.to {
            return;
        }
        // Only two scenes can be blended, so a fade that is interrupted jumps to its end.
        self.from = self.to;
        self.to = look;
        self.complement_from = self.complement_to;
        self.complement_to = self.complement_of(look);
        self.progress = 0.0;
        self.fade_secs = fade_secs.max(0.05);
    }

    /// The look for Complement surfaces that goes with `main`: the same
    /// colour, effect, atmosphere and material; a pattern and a background of
    /// the theme's other than Main's (kept from before while they still
    /// are); the same object only if it is a wall.
    fn complement_of(&mut self, main: Look) -> Look {
        let t = &THEMES[self.theme];
        let taste = self.taste();
        let before = self.complement_to;
        let mut c = Look {
            palette: main.palette,
            post: main.post,
            atmos: main.atmos,
            material: main.material,
            object: if WALL_OBJECTS.contains(&main.object) {
                main.object
            } else {
                0
            },
            ..before
        };
        if c.pattern == main.pattern || !t.patterns.contains(&c.pattern) {
            c.pattern =
                self.complement_rng
                    .other_by_vibe(main.pattern, t.patterns, &PATTERN_VIBE, &taste);
        }
        if c.bg == main.bg || !t.backgrounds.contains(&c.bg) {
            c.bg = self
                .complement_rng
                .other_by_vibe(main.bg, t.backgrounds, &BG_VIBE, &taste);
        }
        c.wrapped()
    }

    /// A look from the current theme that differs from the one on screen in
    /// every part it can. A drop prefers the theme's more energetic effects.
    fn all_new(&mut self, drop: bool) -> Look {
        let t = &THEMES[self.theme];
        let v = self.taste();
        let energetic: Vec<usize> = t
            .posts
            .iter()
            .copied()
            .filter(|p| DROP_POSTS.contains(p))
            .collect();
        let posts = if drop && !energetic.is_empty() {
            energetic
        } else {
            t.posts.to_vec()
        };
        Look {
            bg: self
                .rng
                .other_by_vibe(self.to.bg, t.backgrounds, &BG_VIBE, &v),
            pattern: self.pattern_bag.deal_vibe(
                self.to.pattern,
                t.patterns,
                t.patterns.len() / 3,
                |p| vibe_weight(v, PATTERN_VIBE[p]),
                &mut self.rng,
            ),
            palette: self
                .rng
                .other_by_vibe(self.to.palette, t.palettes, &PALETTE_VIBE, &v),
            post: self.rng.other_by_vibe(self.to.post, &posts, &POST_VIBE, &v),
            atmos: self
                .rng
                .other_by_vibe(self.to.atmos, t.atmos, &ATMOS_VIBE, &v),
            ..self.new_object(self.to)
        }
    }

    /// `look` with a different object, or with none. An object is followed
    /// by no object half the time, so objects come and go rather than
    /// being on screen all night; no object is always followed by one.
    fn new_object(&mut self, mut look: Look) -> Look {
        let objects = THEMES[self.theme].objects;
        let stays = look.object != 0 && objects.contains(&look.object);
        // Calm music leaves the screen emptier more often, intense music fuller.
        // Which object comes is weighed squared, like the theme: the objects
        // are the centrepiece, and the mirror ball belongs to bright music.
        let (v, taste) = (self.vibe, self.taste());
        let sure = if v.sure.is_finite() {
            v.sure.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let none_chance = 0.5 - FILL_BY_INTENSITY * v.intensity.clamp(-1.0, 1.0) * sure;
        if look.object != 0 && (self.rng.unit() < none_chance || objects.len() < 2 && stays) {
            look.object = 0;
            return look;
        }
        look.object = self.object_bag.deal_vibe(
            look.object,
            objects,
            objects.len() / 3,
            |o| vibe_weight(taste, OBJECT_VIBE[o]).powi(2),
            &mut self.rng,
        );
        look.material = self.rng.below(MATERIALS.len());
        look
    }

    /// A look that differs from the current one in exactly one part. Parts
    /// still left over from the previous theme go first.
    fn one_new(&mut self, may_change_bg: bool, may_change_post: bool) -> Look {
        let t = &THEMES[self.theme];
        let mut look = self.to;
        // (part, the theme's choices for it, the value on screen)
        let mut parts = vec![(1, t.patterns, look.pattern), (2, t.palettes, look.palette)];
        if may_change_bg {
            parts.push((0, t.backgrounds, look.bg));
        }
        if may_change_post && look.post != DRIFT {
            parts.push((3, t.posts, look.post));
        }
        parts.push((5, t.atmos, look.atmos));
        let left_over: Vec<usize> = parts
            .iter()
            .filter(|(_, list, now)| !list.contains(now))
            .map(|(part, ..)| *part)
            .collect();
        let object_left_over = look.object != 0 && !t.objects.contains(&look.object);
        let choice = if object_left_over {
            4
        } else if !left_over.is_empty() {
            left_over[self.rng.below(left_over.len())]
        } else {
            // Only parts that have something else to change to, plus the
            // object, which counts twice: half its changes take it away, and
            // the 3D objects are the show's centrepiece, so they need the
            // extra turns to all come round within a quarter of an hour.
            let mut open: Vec<usize> = parts
                .iter()
                .filter(|(_, list, _)| list.len() > 1)
                .map(|(p, ..)| *p)
                .collect();
            open.extend([4, 4]);
            open[self.rng.below(open.len())]
        };
        let v = self.taste();
        match choice {
            0 => look.bg = self.rng.other_by_vibe(look.bg, t.backgrounds, &BG_VIBE, &v),
            1 => {
                look.pattern = self.pattern_bag.deal_vibe(
                    look.pattern,
                    t.patterns,
                    t.patterns.len() / 3,
                    |p| vibe_weight(v, PATTERN_VIBE[p]),
                    &mut self.rng,
                )
            }
            2 => {
                look.palette = self
                    .rng
                    .other_by_vibe(look.palette, t.palettes, &PALETTE_VIBE, &v)
            }
            3 => look.post = self.rng.other_by_vibe(look.post, t.posts, &POST_VIBE, &v),
            5 => look.atmos = self.rng.other_by_vibe(look.atmos, t.atmos, &ATMOS_VIBE, &v),
            _ => look = self.new_object(look),
        }
        look
    }
}

/// xorshift64: a tiny random number generator. The same seed always gives
/// the same show, which is what makes the director testable.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Scramble the seed so that neighbouring seeds give unrelated shows;
        // the state must never be zero.
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 32) as usize) % n
    }

    /// 0..1.
    fn unit(&mut self) -> f32 {
        self.below(1 << 24) as f32 / (1 << 24) as f32
    }

    /// One of `options`, each as likely as its weight (all equal if the
    /// weights are unusable).
    fn weighted(&mut self, options: &[usize], weight: impl Fn(usize) -> f32) -> usize {
        let weights: Vec<f32> = options
            .iter()
            .map(|o| weight(*o).max(0.0))
            .map(|w| if w.is_finite() { w } else { 0.0 })
            .collect();
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return options[self.below(options.len())];
        }
        let mut at = self.unit() * total;
        for (o, w) in options.iter().zip(&weights) {
            if at < *w {
                return *o;
            }
            at -= w;
        }
        *options.last().unwrap()
    }

    /// A member of `options` other than `current`, by how well each suits
    /// the vibe; `current` itself if it is the only option.
    fn other_by_vibe(
        &mut self,
        current: usize,
        options: &[usize],
        spots: &[Spot],
        vibe: &Taste,
    ) -> usize {
        let rest: Vec<usize> = options.iter().copied().filter(|o| *o != current).collect();
        if rest.is_empty() {
            current
        } else {
            self.weighted(&rest, |o| {
                spots.get(o).map_or(1.0, |at| vibe_weight(vibe, *at))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DT: f32 = 1.0 / 60.0;
    /// Frames per beat at 128 BPM and 60 fps.
    const FRAMES_PER_BEAT: usize = 28;
    fn groove(beat: u64) -> Features {
        Features {
            beat_count: beat,
            bpm: 128.0,
            confidence: 0.9,
            mood: Mood::Groove,
            ..Default::default()
        }
    }

    /// Plays `beats` beats of steady groove and returns (beat, output) for every frame.
    fn play(d: &mut Director, from_beat: u64, beats: u64) -> Vec<(u64, DirectorOut)> {
        let mut out = Vec::new();
        for beat in from_beat..from_beat + beats {
            for _ in 0..FRAMES_PER_BEAT {
                out.push((beat, d.update(&groove(beat), DT)));
            }
        }
        out
    }

    /// The beats at which the target look changed, with the look before and after.
    fn changes(frames: &[(u64, DirectorOut)]) -> Vec<(u64, Look, Look)> {
        frames
            .windows(2)
            .filter(|w| w[0].1.to != w[1].1.to)
            .map(|w| (w[1].0, w[0].1.to, w[1].1.to))
            .collect()
    }

    /// How many things changed. An object and the material it is made of
    /// count as one thing.
    fn fields_changed(a: Look, b: Look) -> usize {
        let object = a.object != b.object || a.material != b.material;
        [
            a.bg != b.bg,
            a.pattern != b.pattern,
            a.palette != b.palette,
            a.post != b.post,
            object,
            a.atmos != b.atmos,
        ]
        .iter()
        .filter(|c| **c)
        .count()
    }

    #[test]
    fn changes_only_on_phrase_boundaries() {
        let mut d = Director::new(1);
        let all = changes(&play(&mut d, 0, 128));
        assert_eq!(all.len(), 7, "{all:?}"); // beats 16, 32, ... 112
        for (beat, _, _) in &all {
            assert_eq!(beat % 16, 0, "changed at beat {beat}");
        }
    }

    #[test]
    fn background_changes_every_32() {
        let mut d = Director::new(2);
        let all = changes(&play(&mut d, 0, 128));
        let bg_beats: Vec<u64> = all
            .iter()
            .filter(|(_, a, b)| a.bg != b.bg)
            .map(|c| c.0)
            .collect();
        assert_eq!(bg_beats, vec![32, 64, 96]);
    }

    #[test]
    fn between_backgrounds_exactly_one_other_thing_changes() {
        // ...except when a new theme starts: then the background and the
        // colour change together.
        let mut d = Director::new(3);
        let frames = play(&mut d, 0, 1024);
        for w in frames.windows(2) {
            let ((_, before), (beat, after)) = (w[0], w[1]);
            if before.to == after.to {
                continue;
            }
            let expected = if before.theme != after.theme { 2 } else { 1 };
            assert_eq!(
                fields_changed(before.to, after.to),
                expected,
                "beat {beat}: {:?} -> {:?}",
                before.to,
                after.to
            );
            if beat % 32 != 0 {
                assert_eq!(before.to.bg, after.to.bg, "beat {beat}");
            }
        }
    }

    #[test]
    fn themes_are_well_formed() {
        assert!(THEMES.len() >= 4);
        for t in &THEMES {
            let lists: [(&[usize], usize); 6] = [
                (t.palettes, PALETTES.len()),
                (t.backgrounds, BACKGROUNDS.len()),
                (t.patterns, PATTERNS.len()),
                (t.posts, POST_PRESETS.len()),
                (t.objects, OBJECTS.len()),
                (t.atmos, ATMOSPHERES.len()),
            ];
            for (list, len) in lists {
                assert!(
                    !list.is_empty() && list.iter().all(|i| *i < len),
                    "{}: {list:?}",
                    t.name
                );
            }
            assert!(
                !t.posts.contains(&DRIFT),
                "{}: the breakdown effect is not for grooves",
                t.name
            );
            assert!(
                !t.objects.contains(&0),
                "{}: no object is always allowed",
                t.name
            );
            assert!(
                t.backgrounds.len() >= 2 && t.patterns.len() >= 2,
                "{}: too little to vary",
                t.name
            );
            assert!(
                t.atmos.len() >= 2,
                "{}: a drop must be able to change the atmosphere",
                t.name
            );
        }
        // Between them the themes use everything there is.
        for (len, pick) in [
            (
                PALETTES.len(),
                (|t: &Theme| t.palettes) as fn(&Theme) -> &'static [usize],
            ),
            (BACKGROUNDS.len(), |t| t.backgrounds),
            (PATTERNS.len(), |t| t.patterns),
            (OBJECTS.len() - 1, |t| t.objects),
            (ATMOSPHERES.len(), |t| t.atmos),
        ] {
            let mut used: Vec<usize> = THEMES
                .iter()
                .flat_map(|t| pick(t).iter().copied())
                .collect();
            used.sort();
            used.dedup();
            assert_eq!(used.len(), len, "unused: {used:?}");
        }
    }

    #[test]
    fn a_theme_holds_for_about_two_minutes() {
        let mut d = Director::new(19);
        let frames = play(&mut d, 0, 2048); // 16 minutes at 128 BPM
        let switches = frames
            .windows(2)
            .filter(|w| w[0].1.theme != w[1].1.theme)
            .count();
        assert!(
            (7..=8).contains(&switches),
            "{switches} theme changes in 2048 beats"
        );
        let mut themes: Vec<usize> = frames.iter().map(|(_, o)| o.theme).collect();
        themes.sort();
        themes.dedup();
        assert_eq!(themes.len(), THEMES.len(), "themes shown: {themes:?}");
    }

    #[test]
    fn colour_and_background_always_belong_to_the_theme() {
        let mut d = Director::new(20);
        for (beat, o) in play(&mut d, 0, 2048) {
            let t = &THEMES[o.theme];
            assert!(
                t.palettes.contains(&o.to.palette),
                "beat {beat}: palette {} in {}",
                o.to.palette,
                t.name
            );
            assert!(
                t.backgrounds.contains(&o.to.bg),
                "beat {beat}: background {} in {}",
                o.to.bg,
                t.name
            );
        }
    }

    #[test]
    fn the_rest_of_the_look_joins_the_theme_soon() {
        // Pattern, effect and object follow the theme within its first half.
        let mut d = Director::new(21);
        let frames = play(&mut d, 0, 2048);
        let mut in_theme = 0;
        for w in frames.windows(2) {
            let (beat, o) = w[1];
            in_theme = if w[0].1.theme == o.theme {
                in_theme + 1
            } else {
                0
            };
            if in_theme < 8 * 16 * FRAMES_PER_BEAT {
                continue;
            }
            let t = &THEMES[o.theme];
            assert!(
                t.patterns.contains(&o.to.pattern),
                "beat {beat}: pattern {} in {}",
                o.to.pattern,
                t.name
            );
            assert!(
                t.posts.contains(&o.to.post),
                "beat {beat}: effect {} in {}",
                o.to.post,
                t.name
            );
            assert!(
                o.to.object == 0 || t.objects.contains(&o.to.object),
                "beat {beat}: object {} in {}",
                o.to.object,
                t.name
            );
        }
    }

    #[test]
    fn every_drop_brings_a_new_theme() {
        let mut d = Director::new(22);
        let mut theme = play(&mut d, 0, 4).last().unwrap().1.theme;
        for k in 0..6 {
            let beat = 4 + k * 20;
            let hit = d.update(
                &Features {
                    drop: true,
                    mood: Mood::Drop,
                    ..groove(beat)
                },
                DT,
            );
            assert_ne!(hit.theme, theme, "drop {k}");
            let t = &THEMES[hit.theme];
            assert!(
                t.palettes.contains(&hit.to.palette) && t.patterns.contains(&hit.to.pattern),
                "drop {k}"
            );
            theme = hit.theme;
            play(&mut d, beat + 1, 18);
        }
    }

    #[test]
    fn steady_groove_never_uses_the_breakdown_effect() {
        let mut d = Director::new(4);
        assert!(play(&mut d, 0, 512).iter().all(|(_, o)| o.to.post != DRIFT));
    }

    #[test]
    fn drop_changes_everything() {
        let mut d = Director::new(5);
        let before = play(&mut d, 0, 20).last().unwrap().1.to;
        let hit = d.update(
            &Features {
                drop: true,
                mood: Mood::Drop,
                ..groove(20)
            },
            DT,
        );
        assert_eq!(
            fields_changed(before, hit.to),
            6,
            "{before:?} -> {:?}",
            hit.to
        );
        assert_eq!(hit.flash, 1.0);
        // A drop cuts fast: half a second later the new look is fully in.
        let mut last = hit;
        for _ in 0..31 {
            last = d.update(
                &Features {
                    mood: Mood::Drop,
                    ..groove(20)
                },
                DT,
            );
        }
        assert_eq!(last.mix, 1.0);
        assert!(last.flash < 0.3, "flash still {}", last.flash);
    }

    #[test]
    fn kicks_after_a_drop_flash_less_each_beat_then_stop() {
        let mut d = Director::new(13);
        play(&mut d, 0, 4);
        d.update(
            &Features {
                drop: true,
                mood: Mood::Drop,
                ..groove(4)
            },
            DT,
        );
        // Let the drop's own flash die away, then measure a kick on each later beat.
        let kick_flash = |d: &mut Director, beat: u64| {
            for _ in 0..FRAMES_PER_BEAT {
                d.update(&groove(beat), DT);
            }
            d.update(
                &Features {
                    pulse: 1.0,
                    ..groove(beat)
                },
                DT,
            )
            .flash
        };
        let flashes: Vec<f32> = (5..14).map(|beat| kick_flash(&mut d, beat)).collect();
        assert!(flashes[0] > 0.3 && flashes[0] <= 0.6, "{flashes:?}");
        assert!(flashes.windows(2).all(|w| w[1] <= w[0]), "{flashes:?}");
        assert_eq!(flashes[8], 0.0, "{flashes:?}"); // beat 13 is nine beats after the drop
    }

    #[test]
    fn kick_flashes_stop_even_if_the_beat_never_counts() {
        // A drop before any tempo was found: the beat counter stands still,
        // so "the first eight beats" would otherwise last forever.
        let mut d = Director::new(14);
        let stuck = Features {
            confidence: 0.0,
            mood: Mood::Groove,
            ..Default::default()
        };
        d.update(
            &Features {
                drop: true,
                ..stuck
            },
            DT,
        );
        let mut last = 1.0;
        for frame in 0..60 * 30 {
            last = d
                .update(
                    &Features {
                        pulse: 1.0,
                        ..stuck
                    },
                    DT,
                )
                .flash;
            if frame == 60 * 6 {
                assert_eq!(last, 0.0, "still flashing six seconds after the drop");
            }
        }
        assert_eq!(last, 0.0);
    }

    #[test]
    fn phrase_restarts_at_the_drop() {
        let mut d = Director::new(6);
        play(&mut d, 0, 40);
        d.update(
            &Features {
                drop: true,
                mood: Mood::Drop,
                ..groove(40)
            },
            DT,
        );
        let beats: Vec<u64> = changes(&play(&mut d, 40, 40)).iter().map(|c| c.0).collect();
        assert_eq!(beats, vec![56, 72]);
    }

    #[test]
    fn lock_freezes_look() {
        let mut d = Director::new(7);
        let start = d.update(&groove(0), DT).to;
        assert!(d.toggle_lock());
        assert!(d.locked());
        let frames = play(&mut d, 0, 128);
        let hit = d.update(
            &Features {
                drop: true,
                mood: Mood::Drop,
                ..groove(128)
            },
            DT,
        );
        assert!(frames.iter().all(|(_, o)| o.to == start));
        assert_eq!(hit.to, start);
        // Unlocking brings the changes back.
        assert!(!d.toggle_lock());
        assert!(!changes(&play(&mut d, 129, 64)).is_empty());
    }

    #[test]
    fn a_locked_tempo_with_shaky_confidence_stays_on_the_phrases() {
        // Through a room microphone the bass line makes the tempo look
        // unsure, though it is right and the kicks keep landing. The
        // changes must stay on the phrases, not move to a 20 second timer.
        let mut d = Director::new(23);
        let mut frames = Vec::new();
        for beat in 0..256u64 {
            for i in 0..FRAMES_PER_BEAT {
                let f = Features {
                    confidence: 0.1,
                    kick: i == 0,
                    pulse: if i == 0 { 1.0 } else { 0.0 },
                    ..groove(beat)
                };
                frames.push((beat, d.update(&f, DT)));
            }
        }
        let all = changes(&frames);
        assert!(all.len() >= 12, "only {} changes in 256 beats", all.len());
        for (beat, _, _) in &all {
            assert_eq!(beat % 16, 0, "changed at beat {beat}");
        }
    }

    #[test]
    fn no_beat_falls_back_to_timer() {
        let mut d = Director::new(8);
        let silent = Features::default(); // confidence 0, no beats
        let frames: Vec<(u64, DirectorOut)> =
            (0..60 * 60).map(|_| (0, d.update(&silent, DT))).collect();
        let n = changes(&frames).len();
        assert!((2..=4).contains(&n), "{n} changes in 60 s without a beat");
    }

    #[test]
    fn same_seed_same_show() {
        let show = |seed| {
            let mut d = Director::new(seed);
            play(&mut d, 0, 256)
                .iter()
                .map(|(_, o)| o.to)
                .collect::<Vec<_>>()
        };
        assert_eq!(show(42), show(42));
        assert_ne!(show(42), show(43));
    }

    #[test]
    fn breakdown_brings_drift_and_groove_brings_the_old_effect_back() {
        let mut d = Director::new(9).with_look(Look {
            post: 2,
            ..Look::default()
        });
        play(&mut d, 0, 4);
        let breakdown = Features {
            mood: Mood::Breakdown,
            ..groove(4)
        };
        assert_eq!(d.update(&breakdown, DT).to.post, DRIFT);
        let build = Features {
            mood: Mood::Build,
            ..groove(5)
        };
        assert_eq!(d.update(&build, DT).to.post, DRIFT);
        assert_eq!(d.update(&groove(6), DT).to.post, 2);
    }

    #[test]
    fn a_change_fades_in_over_two_beats() {
        let mut d = Director::new(10);
        let frames = play(&mut d, 0, 20);
        let at = frames.iter().position(|(beat, _)| *beat == 16).unwrap();
        let (first, later) = (frames[at].1, frames[at + FRAMES_PER_BEAT].1);
        assert!(first.mix < 0.1, "mix starts at {}", first.mix);
        assert!(
            later.mix > 0.3 && later.mix < 0.7,
            "mix after one beat {}",
            later.mix
        );
        let settled = frames[at + 2 * FRAMES_PER_BEAT + 2].1;
        assert_eq!(settled.mix, 1.0);
        assert_eq!(settled.from, settled.to);
        // Mix never runs backwards during the fade.
        let fade = &frames[at..at + 2 * FRAMES_PER_BEAT];
        assert!(fade.windows(2).all(|w| w[1].1.mix >= w[0].1.mix));
    }

    #[test]
    fn the_bag_deals_every_value_before_repeating_one() {
        for n in [2, 4, 5, 10] {
            let (mut bag, mut rng) = (Bag::new(n), Rng::new(n as u64));
            let mut current = 0;
            let mut dealt = Vec::new();
            for _ in 0..n * 20 {
                current = bag.next(current, &mut rng);
                dealt.push(current);
            }
            // Never the same twice in a row.
            assert!(dealt.windows(2).all(|w| w[0] != w[1]), "n {n}: {dealt:?}");
            // Any n - 1 in a row are all different (the value on screen is
            // skipped once, so a full set of n can be one short).
            for w in dealt.windows(n - 1) {
                let mut seen = w.to_vec();
                seen.sort();
                seen.dedup();
                assert_eq!(seen.len(), n - 1, "n {n}: {w:?} in {dealt:?}");
            }
        }
    }

    #[test]
    fn the_bag_is_not_a_fixed_cycle() {
        // In a fixed cycle every value is always followed by the same one.
        // A shuffled deck follows each value with many different ones.
        let (mut bag, mut rng) = (Bag::new(6), Rng::new(3));
        let mut current = 0;
        let mut followers = vec![Vec::new(); 6];
        for _ in 0..300 {
            let next = bag.next(current, &mut rng);
            followers[current].push(next);
            current = next;
        }
        for (value, after) in followers.iter_mut().enumerate() {
            after.sort();
            after.dedup();
            assert!(
                after.len() >= 3,
                "{value} is always followed by one of {after:?}"
            );
        }
    }

    #[test]
    fn every_pattern_gets_its_turn() {
        // The first real-music run showed only two of the four patterns in
        // three minutes. With a deck, a long groove shows them all: within
        // an hour (8192 beats at 128 BPM), for all but the odd unlucky seed.
        let mut unlucky = Vec::new();
        for seed in 0..20 {
            let frames = play(&mut Director::new(seed), 0, 8192);
            let mut seen: Vec<usize> = frames.iter().map(|(_, o)| o.to.pattern).collect();
            seen.sort();
            seen.dedup();
            if seen.len() < PATTERNS.len() {
                unlucky.push(seed);
            }
        }
        assert!(
            unlucky.len() <= 1,
            "seeds that missed a pattern: {unlucky:?}"
        );
        let mut d = Director::new(15);
        let frames = play(&mut d, 0, 4096);
        let mut bgs: Vec<usize> = frames.iter().map(|(_, o)| o.to.bg).collect();
        bgs.sort();
        bgs.dedup();
        assert_eq!(bgs.len(), BACKGROUNDS.len(), "backgrounds shown: {bgs:?}");
    }

    #[test]
    fn objects_come_and_go() {
        // Within an hour every object appears, for all but the odd unlucky
        // seed (each stays a few minutes, and there are twelve), in both
        // materials, and there are stretches with no object at all.
        let mut unlucky = Vec::new();
        for seed in 0..20 {
            let frames = play(&mut Director::new(seed), 0, 8192);
            let mut objects: Vec<usize> = frames.iter().map(|(_, o)| o.to.object).collect();
            objects.sort();
            objects.dedup();
            if objects.len() < OBJECTS.len() {
                unlucky.push(seed);
            }
        }
        assert!(
            unlucky.len() <= 1,
            "seeds that missed an object: {unlucky:?}"
        );
        let mut d = Director::new(18);
        let frames = play(&mut d, 0, 2048);
        let with: Vec<&DirectorOut> = frames
            .iter()
            .map(|(_, o)| o)
            .filter(|o| o.to.object != 0)
            .collect();
        let share = with.len() as f32 / frames.len() as f32;
        assert!(
            (0.4..0.85).contains(&share),
            "an object is on screen {:.0}% of the time",
            share * 100.0
        );
        assert!(with.iter().any(|o| o.to.material == 0) && with.iter().any(|o| o.to.material == 1));
    }

    #[test]
    fn atmospheres_come_and_go_with_the_themes() {
        // Half an hour at 128 BPM.
        let mut d = Director::new(24);
        let frames = play(&mut d, 0, 4096);
        let mut seen: Vec<usize> = frames.iter().map(|(_, o)| o.to.atmos).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), ATMOSPHERES.len(), "atmospheres shown: {seen:?}");
        let mut in_theme = 0;
        for w in frames.windows(2) {
            let (beat, o) = w[1];
            in_theme = if w[0].1.theme == o.theme {
                in_theme + 1
            } else {
                0
            };
            if in_theme > 8 * 16 * FRAMES_PER_BEAT {
                let t = &THEMES[o.theme];
                assert!(
                    t.atmos.contains(&o.to.atmos),
                    "beat {beat}: atmosphere {} in {}",
                    o.to.atmos,
                    t.name
                );
            }
        }
    }

    #[test]
    fn a_build_up_closes_in_and_the_drop_opens_it() {
        let mut d = Director::new(16);
        let calm = play(&mut d, 0, 8);
        assert!(calm.iter().all(|(_, o)| o.tension == 0.0));

        let build = Features {
            mood: Mood::Build,
            ..groove(8)
        };
        let mut tension = Vec::new();
        for _ in 0..60 * 8 {
            tension.push(d.update(&build, DT).tension);
        }
        assert!(
            tension.windows(2).all(|w| w[1] >= w[0]),
            "tension must only grow"
        );
        assert!(tension[60] < 0.2, "after 1 s: {}", tension[60]);
        let peak = *tension.last().unwrap();
        assert!((0.8..=0.9).contains(&peak), "after 8 s: {peak}");

        let hit = d.update(
            &Features {
                drop: true,
                mood: Mood::Drop,
                ..groove(9)
            },
            DT,
        );
        assert_eq!(hit.tension, 0.0);
    }

    #[test]
    fn a_build_up_that_fizzles_out_eases_back() {
        let mut d = Director::new(17);
        let build = Features {
            mood: Mood::Build,
            ..groove(0)
        };
        for _ in 0..60 * 6 {
            d.update(&build, DT);
        }
        let mut last = 1.0;
        let mut prev = d.update(&groove(1), DT).tension;
        assert!(prev > 0.3, "the build should have closed in: {prev}");
        for _ in 0..30 {
            last = d.update(&groove(1), DT).tension;
            // Easing, not snapping: no single frame drops more than a tenth.
            assert!(
                prev - last < 0.1,
                "dropped from {prev} to {last} in one frame"
            );
            prev = last;
        }
        assert_eq!(last, 0.0, "half a second later");
    }

    #[test]
    fn manual_controls() {
        let mut d = Director::new(11);
        let start = d.update(&groove(0), DT).to;
        d.next_palette();
        let a = d.update(&groove(0), DT).to;
        assert_eq!(fields_changed(start, a), 1);
        assert_ne!(start.palette, a.palette);
        d.next();
        let b = d.update(&groove(0), DT).to;
        assert_eq!(fields_changed(a, b), 6);
        d.edit(|l| l.bg = 99);
        let c = d.update(&groove(0), DT).to;
        assert_eq!(c.bg, 99 % BACKGROUNDS.len());
    }

    #[test]
    fn looks_are_always_in_range() {
        let mut d = Director::new(12);
        let mut frames = play(&mut d, 0, 600);
        frames.push((
            600,
            d.update(
                &Features {
                    drop: true,
                    ..groove(600)
                },
                DT,
            ),
        ));
        for (_, o) in frames {
            assert_eq!(o.to, o.to.wrapped());
            assert_eq!(o.from, o.from.wrapped());
            assert!((0.0..=1.0).contains(&o.mix) && (0.0..=1.0).contains(&o.flash));
        }
    }

    /// Plays `beats` beats of steady groove whose vibe is `vibe`.
    fn play_vibe(seed: u64, vibe: Vibe, beats: u64) -> Vec<DirectorOut> {
        let mut d = Director::new(seed);
        let mut out = Vec::new();
        for beat in 0..beats {
            for _ in 0..FRAMES_PER_BEAT {
                out.push(d.update(
                    &Features {
                        vibe,
                        ..groove(beat)
                    },
                    DT,
                ));
            }
        }
        out
    }

    fn vibe(brightness: f32, intensity: f32) -> Vibe {
        Vibe {
            brightness,
            intensity,
            texture: 0.0,
            sure: 1.0,
        }
    }

    /// Share of frames, over several seeds, for which `test` holds.
    fn share(v: Vibe, beats: u64, test: impl Fn(&DirectorOut) -> bool) -> f32 {
        let (mut hit, mut all) = (0, 0);
        for seed in 0..8 {
            let frames = play_vibe(seed, v, beats);
            hit += frames.iter().filter(|o| test(o)).count();
            all += frames.len();
        }
        hit as f32 / all as f32
    }

    #[test]
    fn dark_music_gets_the_dark_themes_and_bright_music_the_bright_ones() {
        // red room 0, terminal 2 are dark; white corridor 1, rainbow 5 bright.
        let dark = |o: &DirectorOut| o.theme == 0 || o.theme == 2;
        let bright = |o: &DirectorOut| o.theme == 1 || o.theme == 5;
        let in_dark = share(vibe(-0.8, 0.6), 4096, dark);
        let in_bright = share(vibe(0.8, 0.6), 4096, bright);
        println!(
            "dark themes in dark music {in_dark:.2}, bright themes in bright music {in_bright:.2}"
        );
        assert!(in_dark > 0.5, "{in_dark}");
        assert!(in_bright > 0.5, "{in_bright}");
        // And the other way round: the dark themes come far less in bright music.
        let dark_in_bright = share(vibe(0.8, 0.6), 4096, dark);
        assert!(dark_in_bright < in_dark / 2.0, "{dark_in_bright}");
    }

    #[test]
    fn the_mirror_ball_comes_more_in_bright_music() {
        let ball = |o: &DirectorOut| o.to.object == 1;
        let (bright, dark) = (
            share(vibe(0.8, 0.4), 4096, ball),
            share(vibe(-0.8, 0.4), 4096, ball),
        );
        println!("mirror ball on screen: bright {bright:.3}, dark {dark:.3}");
        assert!(bright > 2.0 * dark, "bright {bright} dark {dark}");
    }

    #[test]
    fn calm_music_leaves_the_screen_emptier() {
        let object = |o: &DirectorOut| o.to.object != 0;
        let atmos = |o: &DirectorOut| o.to.atmos != 0;
        let (calm, intense) = (vibe(0.0, -0.9), vibe(0.0, 0.9));
        let (oc, oi) = (share(calm, 4096, object), share(intense, 4096, object));
        let (ac, ai) = (share(calm, 4096, atmos), share(intense, 4096, atmos));
        println!(
            "object on screen calm {oc:.2} intense {oi:.2}; atmosphere calm {ac:.2} intense {ai:.2}"
        );
        assert!(oi > oc + 0.1, "objects: calm {oc} intense {oi}");
        assert!(ai > ac + 0.1, "atmosphere: calm {ac} intense {ai}");
    }

    #[test]
    fn nothing_disappears_even_when_the_vibe_never_changes() {
        // Two hours of the same dark, calm music: every object and pattern
        // still comes up, for all but the odd unlucky seed.
        let mut unlucky = Vec::new();
        for seed in 0..10 {
            let frames = play_vibe(seed, vibe(-0.8, -0.8), 16384);
            let mut objects: Vec<usize> = frames.iter().map(|o| o.to.object).collect();
            let mut patterns: Vec<usize> = frames.iter().map(|o| o.to.pattern).collect();
            for list in [&mut objects, &mut patterns] {
                list.sort();
                list.dedup();
            }
            if objects.len() < OBJECTS.len() || patterns.len() < PATTERNS.len() {
                unlucky.push((seed, objects.len(), patterns.len()));
            }
        }
        assert!(unlucky.len() <= 1, "{unlucky:?}");
    }

    #[test]
    fn an_unknown_vibe_prefers_nothing() {
        let unsure = Vibe {
            brightness: -1.0,
            intensity: 1.0,
            texture: 0.0,
            sure: 0.0,
        };
        for at in THEME_VIBE.iter().chain(&OBJECT_VIBE) {
            assert_eq!(vibe_weight(&unsure, *at), 1.0);
        }
        // And a known one never rules anything out.
        let sure = vibe(1.0, -1.0);
        assert!(
            OBJECT_VIBE
                .iter()
                .all(|at| vibe_weight(&sure, *at) >= VIBE_FLOOR)
        );
    }

    #[test]
    fn every_element_has_a_place_on_the_map() {
        assert_eq!(THEME_VIBE.len(), THEMES.len());
        assert_eq!(PALETTE_VIBE.len(), PALETTES.len());
        assert_eq!(BG_VIBE.len(), BACKGROUNDS.len());
        assert_eq!(PATTERN_VIBE.len(), PATTERNS.len());
        assert_eq!(POST_VIBE.len(), POST_PRESETS.len());
        assert_eq!(OBJECT_VIBE.len(), OBJECTS.len());
        assert_eq!(ATMOS_VIBE.len(), ATMOSPHERES.len());
        let all = THEME_VIBE
            .iter()
            .chain(&PALETTE_VIBE)
            .chain(&BG_VIBE)
            .chain(&PATTERN_VIBE)
            .chain(&POST_VIBE)
            .chain(&OBJECT_VIBE)
            .chain(&ATMOS_VIBE);
        for (b, i) in all {
            assert!((-1.0..=1.0).contains(b) && (-1.0..=1.0).contains(i));
        }
    }

    /// Plays a groove whose vibe is `first` for `switch` beats, then `then`.
    fn play_switch(
        seed: u64,
        first: Vibe,
        then: Vibe,
        switch: u64,
        beats: u64,
    ) -> Vec<(u64, DirectorOut)> {
        let mut d = Director::new(seed);
        let mut out = Vec::new();
        for beat in 0..beats {
            let vibe = if beat < switch { first } else { then };
            for _ in 0..FRAMES_PER_BEAT {
                out.push((
                    beat,
                    d.update(
                        &Features {
                            vibe,
                            ..groove(beat)
                        },
                        DT,
                    ),
                ));
            }
        }
        out
    }

    /// The beats at which the theme changed.
    fn theme_changes(frames: &[(u64, DirectorOut)]) -> Vec<u64> {
        frames
            .windows(2)
            .filter(|w| w[0].1.theme != w[1].1.theme)
            .map(|w| w[1].0)
            .collect()
    }

    #[test]
    fn a_new_track_brings_a_new_theme_at_the_next_phrase() {
        // 128 BPM: 20 s is about 43 beats, then up to a phrase (16) to wait.
        let (dark, bright) = (vibe(-0.8, 0.6), vibe(0.8, 0.6));
        let mut checked = 0;
        for seed in 0..8 {
            let frames = play_switch(seed, dark, bright, 96, 176);
            // A theme that happens to suit the new vibe too may stay.
            let theme_then = frames.iter().find(|(b, _)| *b == 96).unwrap().1.theme;
            if vibe_weight(&bright, THEME_VIBE[theme_then]) >= NEW_TRACK_MISFIT {
                continue;
            }
            checked += 1;
            let after: Vec<u64> = theme_changes(&frames)
                .into_iter()
                .filter(|b| *b > 96)
                .collect();
            assert!(
                !after.is_empty(),
                "seed {seed}: no new theme after the vibe changed"
            );
            let first = after[0];
            assert!(
                first <= 96 + 43 + 16,
                "seed {seed}: new theme only at beat {first}"
            );
            assert!(
                first >= 96 + 40,
                "seed {seed}: new theme at beat {first}, before 20 s had passed"
            );
            assert_eq!(first % 16, 0, "seed {seed}: not on a phrase");
            // The gentle way: only background and colour move at first, slowly.
            let at = frames
                .iter()
                .position(|(b, o)| *b == first && o.from != o.to)
                .unwrap();
            let (before, o) = (frames[at - 1].1.to, frames[at].1);
            assert_eq!(
                fields_changed(before, o.to),
                2,
                "seed {seed}: {before:?} -> {:?}",
                o.to
            );
            let fading = frames[at..].iter().take_while(|(_, o)| o.mix < 1.0).count();
            assert!(
                fading as f32 >= 0.9 * NEW_TRACK_FADE_BEATS * FRAMES_PER_BEAT as f32,
                "seed {seed}: faded over {fading} frames"
            );
        }
        assert!(
            checked >= 4,
            "only {checked} seeds began in a theme unlike the new vibe"
        );
    }

    #[test]
    fn a_short_change_of_vibe_is_not_a_new_track() {
        // Ten seconds of something else (a long fill, a tool) and back.
        let (dark, bright) = (vibe(-0.8, 0.6), vibe(0.8, 0.6));
        for seed in 0..8 {
            let mut d = Director::new(seed);
            let mut changes = Vec::new();
            let mut theme = None;
            for beat in 0..200u64 {
                let vibe = if (96..117).contains(&beat) {
                    bright
                } else {
                    dark
                };
                for _ in 0..FRAMES_PER_BEAT {
                    let o = d.update(
                        &Features {
                            vibe,
                            ..groove(beat)
                        },
                        DT,
                    );
                    if theme.is_some_and(|t| t != o.theme) {
                        changes.push(beat);
                    }
                    theme = Some(o.theme);
                }
            }
            // Only the scheduled change (every 16 phrases = 256 beats) could
            // come, and that is past the end.
            assert!(
                changes.iter().all(|b| *b < 96),
                "seed {seed}: theme changed at {changes:?}"
            );
        }
    }

    #[test]
    fn a_new_track_that_suits_the_theme_changes_nothing() {
        // From one bright, intense vibe to another close by: the theme fits both.
        let mut quiet = 0;
        for seed in 0..8 {
            let frames = play_switch(seed, vibe(0.8, 0.8), vibe(0.3, 0.9), 96, 200);
            quiet += theme_changes(&frames).iter().all(|b| *b < 96) as usize;
        }
        assert!(quiet >= 6, "only {quiet} of 8 kept their theme");
    }

    /// Like `share`, with options set on every director.
    fn share_with(
        o: &DirectorOptions,
        v: Vibe,
        beats: u64,
        test: impl Fn(&DirectorOut) -> bool,
    ) -> f32 {
        let (mut hit, mut all) = (0, 0);
        for seed in 0..8 {
            let mut d = Director::new(seed);
            d.set_options(o.clone());
            for beat in 0..beats {
                for _ in 0..FRAMES_PER_BEAT {
                    let out = d.update(
                        &Features {
                            vibe: v,
                            ..groove(beat)
                        },
                        DT,
                    );
                    hit += test(&out) as usize;
                    all += 1;
                }
            }
        }
        hit as f32 / all as f32
    }

    #[test]
    fn disabled_themes_never_come() {
        let mut themes = vec![true; THEMES.len()];
        themes[0] = false; // red room
        themes[5] = false; // rainbow
        let o = DirectorOptions {
            themes,
            ..DirectorOptions::default()
        };
        // The first theme is dealt before the options; skip the first two minutes.
        let mut d = Director::new(4);
        d.set_options(o.clone());
        let mut seen = Vec::new();
        for beat in 0..4096 {
            for _ in 0..FRAMES_PER_BEAT {
                let out = d.update(
                    &Features {
                        vibe: vibe(-0.8, 0.8),
                        ..groove(beat)
                    },
                    DT,
                );
                if beat > 300 {
                    seen.push(out.theme);
                }
            }
        }
        assert!(
            seen.iter().all(|t| *t != 0 && *t != 5),
            "a disabled theme came"
        );
    }

    #[test]
    fn all_disabled_means_all_allowed() {
        let o = DirectorOptions {
            themes: vec![false; THEMES.len()],
            ..DirectorOptions::default()
        };
        let mut d = Director::new(5);
        d.set_options(o);
        let mut themes: Vec<usize> = (0..4096u64)
            .flat_map(|beat| (0..FRAMES_PER_BEAT).map(move |_| beat))
            .map(|beat| d.update(&groove(beat), DT).theme)
            .collect();
        themes.sort();
        themes.dedup();
        assert!(themes.len() >= 4, "{themes:?}");
    }

    #[test]
    fn not_following_the_vibe_is_the_old_deck() {
        let dark = |o: &DirectorOut| o.theme == 0 || o.theme == 2;
        let off = DirectorOptions {
            follow_vibe: false,
            ..DirectorOptions::default()
        };
        let ignoring = share_with(&off, vibe(-0.8, 0.6), 4096, dark);
        let unknown = share_with(&off, Vibe::default(), 4096, dark);
        assert!(
            (ignoring - unknown).abs() < 0.1,
            "ignoring {ignoring} unknown {unknown}"
        );
        assert!(ignoring < 0.5, "{ignoring}");
    }

    #[test]
    fn more_variety_spreads_the_picks() {
        let dark = |o: &DirectorOut| o.theme == 0 || o.theme == 2;
        let led = DirectorOptions {
            variety: 0.05,
            ..DirectorOptions::default()
        };
        let varied = DirectorOptions {
            variety: 0.5,
            ..DirectorOptions::default()
        };
        let (a, b) = (
            share_with(&led, vibe(-0.8, 0.6), 4096, dark),
            share_with(&varied, vibe(-0.8, 0.6), 4096, dark),
        );
        println!("dark themes in dark music: vibe-led {a:.2}, varied {b:.2}");
        assert!(a > b + 0.1, "vibe-led {a} varied {b}");
    }

    #[test]
    fn complement_shares_colour_and_atmosphere() {
        let mut d = Director::new(7);
        for (_, o) in play(&mut d, 0, 2048) {
            let (m, c) = (o.to, o.complement_to);
            assert_eq!(
                (m.palette, m.post, m.atmos),
                (c.palette, c.post, c.atmos),
                "{m:?} vs {c:?}"
            );
        }
    }

    #[test]
    fn complement_differs_in_pattern_or_background() {
        let mut d = Director::new(8);
        let frames = play(&mut d, 0, 2048);
        let differ = frames
            .iter()
            .filter(|(_, o)| {
                o.to.pattern != o.complement_to.pattern || o.to.bg != o.complement_to.bg
            })
            .count();
        assert!(
            differ as f32 / frames.len() as f32 > 0.95,
            "only {differ} of {} frames differ",
            frames.len()
        );
    }

    #[test]
    fn complement_keeps_walls_only() {
        let mut d = Director::new(9);
        for (_, o) in play(&mut d, 0, 4096) {
            let c = o.complement_to.object;
            assert!(
                c == 0 || (c == o.to.object && WALL_OBJECTS.contains(&c)),
                "{:?} with main {:?}",
                c,
                o.to.object
            );
        }
    }

    #[test]
    fn a_drop_cuts_both() {
        let mut d = Director::new(10);
        let mut drop_at = None;
        for beat in 0..200u64 {
            for frame in 0..FRAMES_PER_BEAT {
                let drop = beat == 100 && frame == 0;
                let o = d.update(
                    &Features {
                        drop,
                        ..groove(beat)
                    },
                    DT,
                );
                if drop {
                    drop_at = Some(o);
                }
            }
        }
        let o = drop_at.unwrap();
        // One mix drives both, so the complement cuts with the main look.
        assert_ne!(o.complement_from, o.complement_to);
        assert_eq!(o.complement_to.palette, o.to.palette);
    }
}
