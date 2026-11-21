//! Where in the track are we: groove, breakdown, build or drop?
//!
//! Techno is built on the kick. Take the low end away and the room waits
//! (breakdown); add rising noise and rolls on top and it winds up (build);
//! bring the kick back and it lands (drop). So the whole detector watches
//! two things: how much low end there is compared to a moment ago, and
//! whether the highs are climbing while the lows are gone.

use super::Mood;
use super::beat::BeatOut;
use super::spectrum::SpectrumFrame;

/// Nothing is playing when the signal stays, for a second, this far under
/// the loudest it has recently been (about -34 dB). Measuring against the
/// music's own level keeps a quiet line input from reading as silence.
const SILENCE_RATIO: f32 = 0.02;
/// Whatever the history, anything under this (about -66 dB) is silence.
const SILENCE_FLOOR_RMS: f32 = 0.0005;
const SILENCE_HOLD_SECS: f32 = 1.0;
/// Once music has been playing, it takes this long to decide it stopped. A
/// breakdown can go almost silent for a few seconds, and a room microphone
/// cannot tell that from the music being switched off.
const SILENCE_AFTER_MUSIC_SECS: f32 = 8.0;
/// How slowly "the loudest it has recently been" is forgotten.
const LOUD_HALF_LIFE_SECS: f32 = 60.0;

const LOW_FAST_SECS: f32 = 0.4;
const LOW_SLOW_SECS: f32 = 8.0;
/// Music must have been playing this long before a breakdown can be called;
/// before that there is no "normal" to compare with.
const WARMUP_SECS: f32 = 4.0;

/// Breakdown: the low end stays under this share of its usual level...
const BREAKDOWN_RATIO: f32 = 0.35;
const BREAKDOWN_HOLD_SECS: f32 = 1.0;
/// ...and ends quietly if it creeps back over this share without a hit.
const RECOVER_RATIO: f32 = 0.6;
const RECOVER_HOLD_SECS: f32 = 0.5;

const HIGH_FAST_SECS: f32 = 1.0;
/// Build: the highs climb this far above their lowest point in the breakdown.
const BUILD_RATIO: f32 = 1.3;
const BUILD_MARGIN: f32 = 0.05;
const BUILD_HOLD_SECS: f32 = 1.0;

/// When the low end returns after being gone at least this long, it is a
/// drop, however it returns (a hit, a run of kicks, or the bass creeping
/// back in). Shorter gaps are fills, and simply end the breakdown.
const DROP_AFTER_GONE_SECS: f32 = 3.0;
/// ...a low hit this strong, relative to the hits before the breakdown...
const DROP_RATIO: f32 = 0.5;
/// ...lasting this many frames in a row.
const DROP_FRAMES: u32 = 2;
const DROP_SECS: f32 = 4.0;

/// Kicks this close together count as a run...
const KICK_RUN_GAP_SECS: f32 = 1.0;
/// ...and a run this long, with at least this share of the usual low end,
/// means the beat is back even if quieter than before. Not kicks alone: a
/// build-up's kick roll is not the drop.
const KICK_RUN: u32 = 4;
const KICK_RUN_MIN_LOWS: f32 = 0.2;
/// No breakdown is called until the tempo has been this sure at least once:
/// without a locked groove there is nothing to break down from.
const GROOVE_CONFIDENCE: f32 = 0.5;
/// A kick this recently means the low end has not really gone.
const KICK_RECENT_SECS: f32 = 1.0;
/// A "breakdown" longer than this is simply how the music is now.
const MAX_LOWS_GONE_SECS: f32 = 90.0;

pub struct MoodDetector {
    mood: Mood,
    /// Seconds spent in the current mood.
    mood_for: f32,
    quiet_for: f32,
    /// Seconds since the music last started.
    awake_for: f32,
    low_fast: f32,
    /// The usual amount of low end. Frozen during a breakdown so it keeps
    /// describing the music from before.
    low_slow: f32,
    /// The usual size of a low hit; frozen like `low_slow`.
    low_peak: f32,
    high_fast: f32,
    /// Lowest `high_fast` seen in the current breakdown.
    high_floor: f32,
    low_missing_for: f32,
    low_back_for: f32,
    rising_for: f32,
    hits: u32,
    /// The loudest recent frame, slowly forgotten.
    loud_peak: f32,
    since_kick: f32,
    /// Kicks in a row, each within `KICK_RUN_GAP_SECS` of the one before.
    kick_run: u32,
    /// A groove with a sure tempo has been heard since the music started.
    grooved: bool,
    /// Music has been heard since the last real silence.
    heard_music: bool,
}

/// The numbers the detector is deciding on, for diagnosing real recordings.
#[derive(Clone, Copy, Debug)]
pub struct MoodLevels {
    /// Low end now (smoothed over 0.4 s).
    pub low_fast: f32,
    /// Low end as it usually is.
    pub low_slow: f32,
    /// Typical size of a low hit.
    pub low_peak: f32,
    /// Highs now (smoothed over 1 s).
    pub high_fast: f32,
    pub secs_since_kick: f32,
    /// The loudest recent frame (RMS); silence is judged against it.
    pub loud_peak: f32,
}

impl Default for MoodDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl MoodDetector {
    pub fn new() -> Self {
        Self {
            mood: Mood::Silence,
            mood_for: 0.0,
            quiet_for: SILENCE_HOLD_SECS,
            awake_for: 0.0,
            low_fast: 0.0,
            low_slow: 0.0,
            low_peak: 0.0,
            high_fast: 0.0,
            high_floor: 0.0,
            low_missing_for: 0.0,
            low_back_for: 0.0,
            rising_for: 0.0,
            hits: 0,
            loud_peak: 0.0,
            since_kick: f32::INFINITY,
            kick_run: 0,
            grooved: false,
            heard_music: false,
        }
    }

    /// Returns the mood and whether a drop landed in this frame. `kick` is
    /// the beat tracker's verdict for the same frame.
    ///
    /// The level comparisons below judge the low end against how it was
    /// before; they are quick but assume the volume has not changed. The
    /// kick tells the same story independently of level, so it is used as
    /// the second opinion: kicks still landing means no breakdown, and a run
    /// of kicks ends one however quiet they are.
    pub fn push(&mut self, frame: &SpectrumFrame, beat: &BeatOut, dt: f32) -> (Mood, bool) {
        let kick = beat.kick;
        self.mood_for += dt;
        let high = frame.bands[3];

        self.since_kick += dt;
        if kick {
            let in_a_run = self.since_kick <= KICK_RUN_GAP_SECS;
            self.kick_run = if in_a_run { self.kick_run + 1 } else { 1 };
            self.since_kick = 0.0;
        } else if self.since_kick > KICK_RUN_GAP_SECS {
            self.kick_run = 0;
        }

        // Silence overrides everything.
        self.loud_peak = (self.loud_peak * 0.5f32.powf(dt / LOUD_HALF_LIFE_SECS)).max(frame.rms);
        let quiet = frame.rms < (SILENCE_RATIO * self.loud_peak).max(SILENCE_FLOOR_RMS);
        self.quiet_for = if quiet { self.quiet_for + dt } else { 0.0 };
        let hold = if self.heard_music {
            SILENCE_AFTER_MUSIC_SECS
        } else {
            SILENCE_HOLD_SECS
        };
        if self.quiet_for >= hold {
            self.set(Mood::Silence);
            self.awake_for = 0.0;
            self.grooved = false;
            self.heard_music = false;
            return (self.mood, false);
        }
        if self.mood == Mood::Silence {
            if quiet {
                return (self.mood, false); // a gap between sounds, not yet music
            }
            self.set(Mood::Groove);
            self.low_fast = frame.low;
            self.low_slow = frame.low;
            self.low_peak = frame.low;
            self.high_fast = high;
        }
        self.awake_for += dt;
        self.heard_music |= self.awake_for > SILENCE_HOLD_SECS;
        if self.mood == Mood::Groove && beat.confidence >= GROOVE_CONFIDENCE {
            self.grooved = true;
        }

        let lows_gone = matches!(self.mood, Mood::Breakdown | Mood::Build);
        self.low_fast += ease(dt, LOW_FAST_SECS) * (frame.low - self.low_fast);
        self.high_fast += ease(dt, HIGH_FAST_SECS) * (high - self.high_fast);
        if !lows_gone {
            // During warm-up follow quickly so "usual" is meaningful soon.
            let secs = if self.awake_for < WARMUP_SECS {
                1.0
            } else {
                LOW_SLOW_SECS
            };
            self.low_slow += ease(dt, secs) * (self.low_fast - self.low_slow);
            self.low_peak = (self.low_peak * 0.5f32.powf(dt / LOW_SLOW_SECS)).max(frame.low);
        }

        let mut drop = false;
        match self.mood {
            Mood::Groove => {
                let missing = self.low_fast < BREAKDOWN_RATIO * self.low_slow;
                self.low_missing_for = if missing {
                    self.low_missing_for + dt
                } else {
                    0.0
                };
                let kick_recently = self.since_kick < KICK_RECENT_SECS;
                if self.awake_for > WARMUP_SECS
                    && self.grooved
                    && self.low_missing_for >= BREAKDOWN_HOLD_SECS
                    && !kick_recently
                {
                    self.set(Mood::Breakdown);
                    self.high_floor = self.high_fast;
                }
            }
            Mood::Breakdown | Mood::Build => {
                self.hits = if frame.low > DROP_RATIO * self.low_peak {
                    self.hits + 1
                } else {
                    0
                };
                let back = self.low_fast > RECOVER_RATIO * self.low_slow;
                self.low_back_for = if back { self.low_back_for + dt } else { 0.0 };

                // However the low end comes back, how long it was gone
                // decides whether that is a drop or the end of a fill.
                let hit = self.hits >= DROP_FRAMES;
                let beat_back =
                    self.kick_run >= KICK_RUN && self.low_fast > KICK_RUN_MIN_LOWS * self.low_slow;
                let crept_back = self.low_back_for >= RECOVER_HOLD_SECS;
                if hit || beat_back || crept_back {
                    let gone_for = self.mood_for + BREAKDOWN_HOLD_SECS;
                    drop = gone_for >= DROP_AFTER_GONE_SECS;
                    self.set(if drop { Mood::Drop } else { Mood::Groove });
                    if beat_back && !hit && !crept_back {
                        // It came back quieter than it left: take the new
                        // level as normal from here on.
                        self.low_slow = self.low_fast;
                        self.low_peak = frame.low;
                    }
                } else if self.mood_for >= MAX_LOWS_GONE_SECS {
                    self.set(Mood::Groove);
                    self.low_slow = self.low_fast;
                    self.low_peak = frame.low;
                } else if self.mood == Mood::Breakdown {
                    self.high_floor = self.high_floor.min(self.high_fast);
                    let rising = self.high_fast > BUILD_RATIO * self.high_floor + BUILD_MARGIN;
                    self.rising_for = if rising { self.rising_for + dt } else { 0.0 };
                    if self.rising_for >= BUILD_HOLD_SECS {
                        // `set` restarts the mood clock; keep it running so the
                        // drop is judged by how long the lows have really been gone.
                        let since_lows_went = self.mood_for;
                        self.set(Mood::Build);
                        self.mood_for = since_lows_went;
                    }
                }
            }
            Mood::Drop => {
                if self.mood_for >= DROP_SECS {
                    self.set(Mood::Groove);
                }
            }
            Mood::Silence => {}
        }
        (self.mood, drop)
    }

    pub fn levels(&self) -> MoodLevels {
        MoodLevels {
            low_fast: self.low_fast,
            low_slow: self.low_slow,
            low_peak: self.low_peak,
            high_fast: self.high_fast,
            secs_since_kick: self.since_kick,
            loud_peak: self.loud_peak,
        }
    }

    fn set(&mut self, mood: Mood) {
        if self.mood != mood {
            self.mood = mood;
            self.mood_for = 0.0;
            self.low_missing_for = 0.0;
            self.low_back_for = 0.0;
            self.rising_for = 0.0;
            self.hits = 0;
        }
    }
}

/// How far to move toward a new value in one step of `dt` so that the value
/// settles in about `secs` seconds (an exponential moving average).
fn ease(dt: f32, secs: f32) -> f32 {
    1.0 - (-dt / secs).exp()
}
