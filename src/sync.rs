//! Lines the visuals up with the sound.
//!
//! Everything the program does takes time: the sound reaches the microphone,
//! waits in the audio buffer, fills an FFT window, is recognised as a kick,
//! is drawn into a frame, and the screen or projector shows that frame. By
//! then the kick was heard a few dozen milliseconds ago, and a flash that
//! comes late looks wrong.
//!
//! Once the tempo is known, though, the next kick can be predicted. So the
//! beat clock runs *ahead* of what was heard by exactly that delay, and
//! the beat-locked visuals (the pulse on every kick, the steps of the
//! patterns, phrase changes) land with the sound instead of after it:
//!
//! * It keeps its own beat position, advancing smoothly at the tempo, and
//!   eases toward the beat tracker's (plus the lead) so the small jolts of the
//!   tracker's corrections never show. A big disagreement (a new track, a
//!   changed tempo) makes it jump straight to the new grid.
//! * The pulse fires when the clock crosses a beat, but only while kicks are
//!   really landing: a breakdown without a kick gets no pulse.
//! * Without a trustworthy tempo it simply passes the analysis through.
//!
//! The lead is the delay inside the program (measured, see the tests) plus
//! an offset set by hand for the room, the screen or the projector, with
//! `[` and `]` or in the menu, kept between runs in the settings.

use crate::analysis::Features;

/// The analysis is late by this much on its own: half the FFT window, the
/// hop, and the kick detector. Measured on kicks with known times
/// (examples/latency_probe.rs): 12 to 29 ms, depending on the music.
pub const ANALYSIS_MS: f32 = 20.0;
/// The audio buffer, and waiting for the next frame to read it.
pub const INPUT_MS: f32 = 15.0;
/// From reading the features to the picture on a screen: drawing the frame
/// and presenting it at the next refresh, about one and a half frames at 60.
pub const OUTPUT_MS: f32 = 25.0;
/// The hand offset is kept within this.
pub const MAX_OFFSET_MS: f32 = 300.0;
/// One press of `[` or `]`.
pub const OFFSET_STEP_MS: f32 = 10.0;

/// Below this confidence the tempo is not used to predict.
const LOCK_CONFIDENCE: f32 = 0.4;
/// Further than this (in beats) from the tracker's grid, the clock jumps.
const SNAP_BEATS: f64 = 0.25;
/// Closer than that, it eases over to the tracker's grid in about this long.
const CORRECT_SECS: f64 = 1.0;
/// The predicted pulse only while a real kick came within this many beats.
const KICKS_LANDING_BEATS: f32 = 1.5;
/// The pulse dies away like the analysis's own does.
const PULSE_RELEASE_SECS: f32 = 0.15;

pub struct BeatClock {
    /// Beats, ahead of what was heard by the lead.
    pos: f64,
    running: bool,
    pulse: f32,
    since_kick: f32,
    /// The hand offset, ms; positive makes the visuals earlier.
    offset_ms: f32,
}

impl Default for BeatClock {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl BeatClock {
    pub fn new(offset_ms: f32) -> Self {
        Self {
            pos: 0.0,
            running: false,
            pulse: 0.0,
            since_kick: f32::INFINITY,
            offset_ms: clamp_offset(offset_ms),
        }
    }

    pub fn offset_ms(&self) -> f32 {
        self.offset_ms
    }

    pub fn set_offset(&mut self, ms: f32) {
        self.offset_ms = clamp_offset(ms);
    }

    /// Moves the hand offset by `steps` presses; returns the new offset.
    pub fn nudge(&mut self, steps: i32) -> f32 {
        self.offset_ms = clamp_offset(self.offset_ms + steps as f32 * OFFSET_STEP_MS);
        self.offset_ms
    }

    /// How far ahead of the sound the clock runs, in seconds.
    pub fn lead_secs(&self) -> f32 {
        (ANALYSIS_MS + INPUT_MS + OUTPUT_MS + self.offset_ms) / 1000.0
    }

    /// The features with the beat moved ahead by the lead: beat count, phase
    /// and pulse. Everything else passes through as it is.
    pub fn update(&mut self, f: &Features, dt: f32) -> Features {
        self.since_kick = if f.kick { 0.0 } else { self.since_kick + dt };
        let locked = f.bpm > 0.0 && f.confidence >= LOCK_CONFIDENCE && dt.is_finite();
        if !locked {
            self.running = false;
            self.pulse = f.pulse;
            return *f;
        }
        let beats_per_sec = f.bpm as f64 / 60.0;
        let target =
            f.beat_count as f64 + f.beat_phase as f64 + self.lead_secs() as f64 * beats_per_sec;
        let before = self.pos;
        if !self.running || (target - self.pos).abs() > SNAP_BEATS {
            self.pos = target;
            self.running = true;
        } else {
            self.pos += dt as f64 * beats_per_sec;
            self.pos += (target - self.pos) * (1.0 - (-(dt as f64) / CORRECT_SECS).exp());
        }
        // (Easing never moves it backwards: the most it corrects in a frame,
        // a quarter beat times 1/60 of a second's worth, is far less than a
        // frame's advance. Only a jump to a new grid can go back.)

        let crossed = self.pos.floor() > before.floor() && before > 0.0;
        let kicks_landing = self.since_kick * (f.bpm / 60.0) < KICKS_LANDING_BEATS;
        self.pulse = if crossed && kicks_landing {
            1.0
        } else {
            self.pulse * (-dt / PULSE_RELEASE_SECS).exp()
        };

        let mut out = *f;
        out.beat_count = self.pos.max(0.0).floor() as u64;
        out.beat_phase = self.pos.max(0.0).fract() as f32;
        out.pulse = self.pulse;
        out
    }
}

fn clamp_offset(ms: f32) -> f32 {
    if ms.is_finite() {
        ms.clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{Analyzer, Mood};

    const DT: f32 = 1.0 / 60.0;

    /// What the tracker says at time t for a steady 128 BPM, its grid late by `late` s.
    fn tracked(t: f32, late: f32, kick: bool) -> Features {
        let pos = ((t - late) * 128.0 / 60.0).max(0.0);
        Features {
            bpm: 128.0,
            confidence: 0.9,
            beat_count: pos.floor() as u64,
            beat_phase: pos.fract(),
            kick,
            pulse: if kick { 1.0 } else { 0.0 },
            mood: Mood::Groove,
            ..Features::default()
        }
    }

    #[test]
    fn the_clock_runs_ahead_by_the_lead() {
        let mut c = BeatClock::new(0.0);
        let mut out = Features::default();
        for i in 0..600 {
            out = c.update(&tracked(i as f32 * DT, 0.0, false), DT);
        }
        let t = 599.0 * DT;
        let shown = out.beat_count as f32 + out.beat_phase;
        let heard = t * 128.0 / 60.0;
        let lead_ms = (shown - heard) / (128.0 / 60.0) * 1000.0;
        let expected = 1000.0 * c.lead_secs();
        assert!(
            (lead_ms - expected).abs() < 2.0,
            "lead {lead_ms} ms, expected {expected}"
        );
    }

    #[test]
    fn small_corrections_glide_and_never_step_back() {
        // The tracker's grid wobbles by ±10 ms; the clock moves on evenly.
        let mut c = BeatClock::new(0.0);
        let mut last = f64::MIN;
        let mut steps = Vec::new();
        for i in 0..1200 {
            let t = i as f32 * DT;
            let wobble = 0.01 * (t * 1.7).sin();
            let out = c.update(&tracked(t, wobble, false), DT);
            let pos = out.beat_count as f64 + out.beat_phase as f64;
            if i > 60 {
                assert!(pos >= last, "stepped back from {last} to {pos}");
                steps.push(pos - last);
            }
            last = pos;
        }
        let per_frame = 128.0 / 60.0 * DT as f64;
        assert!(
            steps
                .iter()
                .all(|s| (s - per_frame).abs() < 0.3 * per_frame),
            "uneven steps"
        );
    }

    #[test]
    fn a_new_grid_is_jumped_to() {
        let mut c = BeatClock::new(0.0);
        for i in 0..300 {
            c.update(&tracked(i as f32 * DT, 0.0, false), DT);
        }
        // The DJ brings in a track half a beat off.
        let half_beat = 0.5 * 60.0 / 128.0;
        let out = c.update(&tracked(300.0 * DT, half_beat, false), DT);
        let expected = tracked(300.0 * DT, half_beat - c.lead_secs(), false);
        let (a, b) = (
            out.beat_count as f32 + out.beat_phase,
            expected.beat_count as f32 + expected.beat_phase,
        );
        assert!((a - b).abs() < 0.02, "{a} vs {b}");
    }

    #[test]
    fn no_pulse_where_no_kick_lands() {
        let mut c = BeatClock::new(0.0);
        let mut pulses = 0;
        for i in 0..600 {
            let t = i as f32 * DT;
            // Kicks for five seconds, then a breakdown: the grid goes on.
            let kick = t < 5.0 && (t * 128.0 / 60.0).fract() < (128.0 / 60.0 * DT);
            let out = c.update(&tracked(t, 0.0, kick), DT);
            if out.pulse > 0.99 {
                assert!(
                    t < 5.0 + 1.5 * 60.0 / 128.0,
                    "pulse at {t} s in the breakdown"
                );
                pulses += 1;
            }
        }
        assert!(pulses >= 8, "only {pulses} pulses while the kicks played");
    }

    #[test]
    fn without_a_tempo_it_passes_the_analysis_through() {
        let mut c = BeatClock::new(0.0);
        let f = Features {
            pulse: 0.7,
            beat_phase: 0.3,
            ..Features::default()
        };
        assert_eq!(c.update(&f, DT).pulse, 0.7);
        assert_eq!(c.update(&f, DT).beat_phase, 0.3);
    }

    #[test]
    fn the_hand_offset_moves_in_steps_and_stays_in_range() {
        let mut c = BeatClock::new(0.0);
        assert_eq!(c.nudge(3), 30.0);
        assert_eq!(c.nudge(-5), -20.0);
        assert_eq!(c.nudge(1000), MAX_OFFSET_MS);
        assert_eq!(BeatClock::new(f32::NAN).offset_ms(), 0.0);
    }

    /// The whole chain on real audio: the predicted pulse lands on the
    /// kicks themselves, not after them, once the delay inside the program
    /// is taken away (the screen's share, OUTPUT_MS, is about the screen).
    #[test]
    fn predicted_pulses_land_on_the_kicks() {
        let sr = 48000;
        let bpm = 128.0f64;
        let signal = crate::synth::kick_train(bpm as f32, 40.0, sr);
        let mut a = Analyzer::new(sr);
        // No screen here, and the samples are read the moment they "arrive".
        let mut c = BeatClock::new(-(INPUT_MS + OUTPUT_MS));
        let mut errors = Vec::new();
        for (i, chunk) in signal.chunks(sr as usize / 60).enumerate() {
            a.push(chunk);
            let f = a.features();
            let t = (i + 1) as f64 / 60.0;
            let out = c.update(&f, DT);
            if t > 15.0 && out.pulse > 0.99 {
                let beat = 60.0 / bpm;
                // The frame shows what is due by its end: when the beat fell inside it.
                let due = (t / beat).floor() * beat;
                errors.push((t - due) * 1000.0);
            }
        }
        let mean = errors.iter().sum::<f64>() / errors.len() as f64;
        println!(
            "pulse after the kick: {mean:.1} ms on average over {} kicks",
            errors.len()
        );
        assert!(errors.len() > 40);
        // Within one frame of the true kick: the frame that contains it.
        assert!(
            errors
                .iter()
                .all(|e| (0.0..=1000.0 / 60.0 + 2.0).contains(e)),
            "{errors:?}"
        );
    }
}
