//! The vibe of what is playing: how bright or dark it sounds, how calm or
//! intense, how smooth or raw. Measured over the last 10 seconds, and
//! compared with the rest of the night, so the same room and microphone
//! read the same way all evening.
//!
//! * **Brightness**: where the sound's weight sits (the spectral centroid).
//!   A sub-heavy, muffled track is dark; open hats and high chords are bright.
//! * **Intensity**: how busy it is (kicks, hats and claps per second), how
//!   fast, and how loud compared with the rest of the night.
//! * **Texture**: how noise-like the mids and highs are (spectral flatness):
//!   distortion and noise are raw, clean tones and chords smooth.
//!
//! Each is -1..1. It moves slowly and smoothly, the way a set moves, and it
//! only listens while there is a groove: a breakdown or a silence leaves it
//! where it was, since a breakdown is part of a track, not a new vibe.

use std::collections::VecDeque;

use super::Mood;
use super::spectrum::SpectrumFrame;

/// The vibe is the average of this many seconds.
const WINDOW_SECS: usize = 10;
/// The night it is compared with: the last ten minutes.
const HISTORY_SECS: usize = 600;
/// After this much music the night's own range has most of the say.
const HISTORY_TRUSTED_SECS: f32 = 120.0;
/// How much the night's range counts, at most, against fixed anchors. The
/// anchors keep a night of only dark techno reading as dark.
const RELATIVE_WEIGHT: f32 = 0.7;

/// Fixed anchors: (value that reads as -1, value that reads as +1).
/// Spectral centroid, as log2 of Hz.
/// Set from recordings of real sets through a laptop microphone (2026-11-21),
/// where dark tracks sat near 900 Hz and bright ones near 2.5 kHz.
const BRIGHT_ANCHOR: (f32, f32) = (9.6, 11.4); // about 780 Hz .. 2.7 kHz
/// Onsets (kicks, hats, claps) per second.
/// Set from the same recordings, where sparse stretches had about 2 a second
/// and busy ones about 8.
const BUSY_ANCHOR: (f32, f32) = (2.0, 8.5);
/// Tempo.
const TEMPO_ANCHOR: (f32, f32) = (118.0, 145.0);
/// Spectral flatness of the mids and highs.
const RAW_ANCHOR: (f32, f32) = (0.04, 0.35);

/// The shown vibe follows the measured one with this time constant...
const SMOOTH_SECS: f32 = 6.0;
/// ...and never faster than this much per second, so a change of track
/// turns the picture gradually.
const MAX_SPEED: f32 = 0.08;

/// Intensity reacts faster than brightness: it is measured over the last few
/// seconds only, and follows more quickly (tried in a room: measured like
/// brightness, it took too long to notice the intensity). Still a glide, not a jump.
const INTENSITY_SECS: usize = 4;
const INTENSITY_SMOOTH_SECS: f32 = 2.5;
const INTENSITY_MAX_SPEED: f32 = 0.25;
/// A drop is intense by definition: while the mood says Drop, the intensity
/// is lifted this much straight away, before the averages catch up.
const DROP_LIFT: f32 = 0.35;

/// Onset detection on the highs: a rise this much above their running
/// average, and not within this long of the last one.
const ONSET_RATIO: f32 = 1.5;
const ONSET_FLOOR: f32 = 0.04;
const ONSET_GAP_SECS: f32 = 0.07;

/// A second is only counted once the groove has gone on this many seconds
/// past it: the mood detector notices a breakdown a moment late, and the
/// start of a breakdown is not the vibe of the track.
const CONFIRM_SECS: usize = 2;

/// Frames quieter than this (RMS) are not measured: the centroid of room
/// hiss means nothing.
const GATE_RMS: f32 = 1e-3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vibe {
    /// -1 dark .. 1 bright.
    pub brightness: f32,
    /// -1 calm .. 1 intense.
    pub intensity: f32,
    /// -1 smooth, melodic .. 1 raw, noisy.
    pub texture: f32,
    /// 0 before any music, 1 once a whole window has been heard.
    pub sure: f32,
}

impl Vibe {
    /// A short name for the overlay: DARK INTENSE, BRIGHT CALM, ...
    pub fn name(&self) -> &'static str {
        if self.sure < 0.3 {
            return "LISTENING";
        }
        match (self.brightness >= 0.0, self.intensity >= 0.0) {
            (false, true) => "DARK INTENSE",
            (false, false) => "DARK CALM",
            (true, true) => "BRIGHT INTENSE",
            (true, false) => "BRIGHT CALM",
        }
    }
}

/// One second's worth of measures.
#[derive(Clone, Copy, Debug, Default)]
pub struct Second {
    /// log2 of the spectral centroid in Hz.
    pub centroid: f32,
    pub flatness: f32,
    /// Onsets (kicks plus hats and claps) per second.
    pub onsets: f32,
    pub bpm: f32,
    /// Loudness, dB.
    pub loudness: f32,
}

#[derive(Default)]
struct Accumulator {
    frames: u32,
    measured: u32,
    centroid: f32,
    flatness: f32,
    onsets: u32,
    loudness: f32,
    bpm: f32,
}

pub struct VibeMeter {
    hop_secs: f32,
    /// Time into the current second.
    clock: f32,
    acc: Accumulator,
    /// Seconds measured but not yet confirmed (see CONFIRM_SECS).
    pending: VecDeque<Second>,
    window: VecDeque<Second>,
    history: VecDeque<Second>,
    /// The high-flux running average and time since the last onset.
    flux_avg: f32,
    since_onset: f32,
    above: bool,
    target: Vibe,
    shown: Vibe,
}

impl VibeMeter {
    pub fn new(hop_secs: f32) -> Self {
        Self {
            hop_secs,
            clock: 0.0,
            acc: Accumulator::default(),
            pending: VecDeque::with_capacity(CONFIRM_SECS + 1),
            window: VecDeque::with_capacity(WINDOW_SECS + 1),
            history: VecDeque::with_capacity(HISTORY_SECS + 1),
            flux_avg: 0.0,
            since_onset: 1.0,
            above: false,
            target: Vibe::default(),
            shown: Vibe::default(),
        }
    }

    /// One analysis frame. `kick` and `bpm` come from the beat tracker,
    /// `mood` from the mood detector.
    pub fn push(&mut self, frame: &SpectrumFrame, kick: bool, bpm: f32, mood: Mood) -> Vibe {
        let dt = self.hop_secs;
        // Onsets in the highs: crossing well above their running average.
        let flux = if frame.flux_high.is_finite() {
            frame.flux_high
        } else {
            0.0
        };
        self.since_onset += dt;
        let threshold = self.flux_avg * ONSET_RATIO + ONSET_FLOOR;
        let onset = flux > threshold && !self.above && self.since_onset > ONSET_GAP_SECS;
        self.above = flux > threshold;
        if onset {
            self.since_onset = 0.0;
        }
        self.flux_avg += (flux - self.flux_avg) * (dt / 1.0).min(1.0);

        // Only music with a groove is measured.
        let listening = matches!(mood, Mood::Groove | Mood::Drop);
        if !listening {
            // A breakdown or silence: the last few seconds were its start.
            self.pending.clear();
        }
        self.acc.frames += 1;
        if listening && frame.rms > GATE_RMS && frame.centroid_hz > 20.0 {
            self.acc.measured += 1;
            self.acc.centroid += frame.centroid_hz.log2();
            self.acc.flatness += frame.flatness;
            self.acc.loudness += 20.0 * frame.rms.log10();
            self.acc.onsets += (kick || onset) as u32;
            self.acc.bpm = bpm;
        }

        self.clock += dt;
        if self.clock >= 1.0 {
            self.clock -= 1.0;
            self.end_second();
        }

        // The shown vibe eases toward the measured one, never too fast.
        let follow = |shown: f32, target: f32, secs: f32, speed: f32| {
            let ease = 1.0 - (-dt / secs).exp();
            let step = speed * dt;
            shown + ((target - shown) * ease).clamp(-step, step)
        };
        let lift = if mood == Mood::Drop && self.target.sure > 0.0 {
            DROP_LIFT
        } else {
            0.0
        };
        let fast = |shown: f32, target: f32| {
            follow(shown, target, INTENSITY_SMOOTH_SECS, INTENSITY_MAX_SPEED)
        };
        let follow = |shown: f32, target: f32| follow(shown, target, SMOOTH_SECS, MAX_SPEED);
        self.shown = Vibe {
            brightness: follow(self.shown.brightness, self.target.brightness),
            intensity: fast(
                self.shown.intensity,
                (self.target.intensity + lift).min(1.0),
            ),
            texture: follow(self.shown.texture, self.target.texture),
            sure: follow(self.shown.sure, self.target.sure),
        };
        self.shown
    }

    /// The measures of the last whole window (for the analyse example).
    pub fn window_average(&self) -> Option<Second> {
        average(&self.window)
    }

    fn end_second(&mut self) {
        let acc = std::mem::take(&mut self.acc);
        // A second counts if most of it was music.
        if acc.measured * 2 < acc.frames {
            self.pending.clear();
            return;
        }
        let n = acc.measured as f32;
        let second = Second {
            centroid: acc.centroid / n,
            flatness: acc.flatness / n,
            onsets: acc.onsets as f32 * acc.frames as f32 / n,
            bpm: acc.bpm,
            loudness: acc.loudness / n,
        };
        self.pending.push_back(second);
        if self.pending.len() <= CONFIRM_SECS {
            return;
        }
        let Some(second) = self.pending.pop_front() else {
            return;
        };
        if self.window.len() == WINDOW_SECS {
            self.window.pop_front();
        }
        self.window.push_back(second);
        let Some(now) = average(&self.window) else {
            return;
        };
        if self.history.len() == HISTORY_SECS {
            self.history.pop_front();
        }
        self.history.push_back(now);

        // The night's own range counts more as more of it has been heard.
        let trust = RELATIVE_WEIGHT * (self.history.len() as f32 / HISTORY_TRUSTED_SECS).min(1.0);
        let rank =
            |value: f32, of: fn(&Second) -> f32| percentile(&self.history, value, of) * 2.0 - 1.0;
        let blend =
            |fixed: f32, relative: f32| (fixed * (1.0 - trust) + relative * trust).clamp(-1.0, 1.0);

        let brightness = blend(
            scale(now.centroid, BRIGHT_ANCHOR),
            rank(now.centroid, |s| s.centroid),
        );
        // Intensity from the last few seconds only.
        let recent = {
            let from = self.window.len().saturating_sub(INTENSITY_SECS);
            average(&self.window.iter().skip(from).copied().collect()).unwrap_or(now)
        };
        let busy = scale(recent.onsets, BUSY_ANCHOR);
        let tempo = if now.bpm > 0.0 {
            scale(now.bpm, TEMPO_ANCHOR)
        } else {
            0.0
        };
        let intensity = blend(
            0.7 * busy + 0.3 * tempo,
            0.4 * rank(recent.onsets, |s| s.onsets)
                + 0.4 * rank(recent.loudness, |s| s.loudness)
                + 0.2 * tempo,
        );
        let texture = blend(
            scale(now.flatness, RAW_ANCHOR),
            rank(now.flatness, |s| s.flatness),
        );
        self.target = Vibe {
            brightness,
            intensity,
            texture,
            sure: self.window.len() as f32 / WINDOW_SECS as f32,
        };
    }
}

/// Maps `value` so that anchor.0 is -1 and anchor.1 is +1, limited to that.
fn scale(value: f32, anchor: (f32, f32)) -> f32 {
    ((value - anchor.0) / (anchor.1 - anchor.0) * 2.0 - 1.0).clamp(-1.0, 1.0)
}

/// Where `value` falls among the history, 0..1 (ties count half).
fn percentile(history: &VecDeque<Second>, value: f32, of: fn(&Second) -> f32) -> f32 {
    if history.is_empty() {
        return 0.5;
    }
    let (below, equal) = history.iter().map(of).fold((0, 0), |(b, e), v| {
        if v < value {
            (b + 1, e)
        } else if v == value {
            (b, e + 1)
        } else {
            (b, e)
        }
    });
    (below as f32 + 0.5 * equal as f32) / history.len() as f32
}

fn average(seconds: &VecDeque<Second>) -> Option<Second> {
    if seconds.is_empty() {
        return None;
    }
    let n = seconds.len() as f32;
    let sum = seconds.iter().fold(Second::default(), |a, s| Second {
        centroid: a.centroid + s.centroid,
        flatness: a.flatness + s.flatness,
        onsets: a.onsets + s.onsets,
        bpm: s.bpm,
        loudness: a.loudness + s.loudness,
    });
    Some(Second {
        centroid: sum.centroid / n,
        flatness: sum.flatness / n,
        onsets: sum.onsets / n,
        bpm: sum.bpm,
        loudness: sum.loudness / n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Analyzer;
    use crate::synth::styled_track;

    /// Plays a signal 1/60 s at a time, like the app, and returns the vibe
    /// after every frame.
    fn vibes(signal: &[f32], sr: u32) -> Vec<Vibe> {
        let mut a = Analyzer::new(sr);
        signal
            .chunks(sr as usize / 60)
            .map(|chunk| {
                a.push(chunk);
                a.features().vibe
            })
            .collect()
    }

    fn settled(dark: bool, intense: bool, gain: f32) -> Vibe {
        let sr = 48000;
        let track: Vec<f32> = styled_track(sr, 50.0, dark, intense)
            .iter()
            .map(|s| s * gain)
            .collect();
        *vibes(&track, sr).last().unwrap()
    }

    #[test]
    fn each_style_lands_in_its_corner() {
        for dark in [true, false] {
            for intense in [true, false] {
                let v = settled(dark, intense, 1.0);
                assert_eq!(
                    v.brightness < 0.0,
                    dark,
                    "dark {dark} intense {intense}: {v:?}"
                );
                assert_eq!(
                    v.intensity > 0.0,
                    intense,
                    "dark {dark} intense {intense}: {v:?}"
                );
                assert!(v.sure > 0.9);
            }
        }
    }

    #[test]
    fn a_quieter_microphone_hears_the_same_vibe() {
        for (dark, intense) in [(true, true), (false, false)] {
            let loud = settled(dark, intense, 1.0);
            let quiet = settled(dark, intense, 0.05);
            assert!(
                (loud.brightness - quiet.brightness).abs() < 0.2,
                "{loud:?} {quiet:?}"
            );
            assert!(
                (loud.intensity - quiet.intensity).abs() < 0.25,
                "{loud:?} {quiet:?}"
            );
        }
    }

    #[test]
    fn a_new_track_turns_the_vibe_gradually() {
        // A dark, intense track mixed into a bright, calm one.
        let sr = 48000;
        let mut signal = styled_track(sr, 50.0, true, true);
        signal.extend(styled_track(sr, 60.0, false, false));
        let v = vibes(&signal, sr);
        // Never a jump: each axis moves no faster than its own top speed.
        let per_frame = |speed: f32| speed / 60.0 * 1.6; // the analysis steps a little unevenly
        for pair in v.windows(2) {
            let (a, b) = (pair[0].brightness, pair[1].brightness);
            assert!(
                (b - a).abs() <= per_frame(MAX_SPEED),
                "brightness jumped from {a} to {b}"
            );
            let (a, b) = (pair[0].intensity, pair[1].intensity);
            assert!(
                (b - a).abs() <= per_frame(INTENSITY_MAX_SPEED),
                "intensity jumped from {a} to {b}"
            );
        }
        // But it gets there: bright within half a minute of the change.
        let at = |secs: f32| v[(secs * 60.0) as usize];
        assert!(at(49.0).brightness < 0.0, "{:?}", at(49.0));
        assert!(at(80.0).brightness > 0.0, "{:?}", at(80.0));
        assert!(at(109.0).intensity < at(49.0).intensity);
    }

    #[test]
    fn intensity_notices_a_change_within_seconds() {
        // A calm dark track, then an intense dark one: within about ten
        // seconds of the change the intensity has crossed over.
        let sr = 48000;
        let mut signal = styled_track(sr, 40.0, true, false);
        signal.extend(styled_track(sr, 20.0, true, true));
        let v = vibes(&signal, sr);
        let at = |secs: f32| v[(secs * 60.0) as usize - 1];
        assert!(at(40.0).intensity < 0.0, "{:?}", at(40.0));
        let crossed = (40..60).find(|s| at(*s as f32).intensity > 0.0);
        println!("intense from {crossed:?} s (the change was at 40 s)");
        assert!(crossed.is_some_and(|s| s <= 50), "crossed at {crossed:?}");
    }

    #[test]
    fn a_breakdown_does_not_change_the_vibe() {
        // The test track's breakdown and build (7.5 s .. 15 s) follow 40 s
        // of a dark, intense groove; the mood detector calls the breakdown
        // at about 41.4 s. From then on the vibe stays where it was.
        let sr = 48000;
        let mut signal = styled_track(sr, 40.0, true, true);
        let track = crate::synth::test_track(sr);
        signal.extend(&track[(7.5 * sr as f32) as usize..(15.0 * sr as f32) as usize]);
        let v = vibes(&signal, sr);
        let before = v[(41.0 * 60.0) as usize];
        let after = *v.last().unwrap();
        assert!(
            (before.brightness - after.brightness).abs() < 0.15,
            "{before:?} {after:?}"
        );
        assert!(
            (before.intensity - after.intensity).abs() < 0.15,
            "{before:?} {after:?}"
        );
    }

    #[test]
    fn silence_gives_a_neutral_unsure_vibe() {
        let v = vibes(&vec![0.0; 48000 * 20], 48000);
        assert_eq!(*v.last().unwrap(), Vibe::default());
    }

    #[test]
    fn the_names_follow_the_map() {
        assert_eq!(Vibe::default().name(), "LISTENING");
        let v = |b, i| {
            Vibe {
                brightness: b,
                intensity: i,
                texture: 0.0,
                sure: 1.0,
            }
            .name()
        };
        assert_eq!(v(-0.5, 0.5), "DARK INTENSE");
        assert_eq!(v(0.5, -0.5), "BRIGHT CALM");
    }
}
