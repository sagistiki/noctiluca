//! Kick detection, tempo and beat phase.
//!
//! Input is one number per spectrum frame: how much the low end just grew
//! ("flux"). A kick drum shows up as a short burst of it.
//!
//! * A **kick** is a burst that stands clear of the recent background.
//! * The **tempo** is the repeat distance at which the last eight seconds of
//!   flux line up best with themselves (autocorrelation).
//! * The **phase** is where in the beat the strong bursts tend to fall.
//!
//! Once tempo and phase are known the beat keeps counting on its own, so a
//! breakdown without kicks does not lose the count.

use std::collections::VecDeque;
use std::f32::consts::TAU;

const HISTORY_SECS: f32 = 8.0;
const BACKGROUND_SECS: f32 = 1.5;
const REFRACTORY_SECS: f32 = 0.2;
const TEMPO_UPDATE_SECS: f32 = 0.25;
const PHASE_WINDOW_SECS: f32 = 6.0;

const MIN_BPM: f32 = 90.0;
const MAX_BPM: f32 = 180.0;
const BPM_STEP: f32 = 0.25;
/// The repeat is checked at 1, 2, 3 and 4 beats; the longer distances are
/// what give the tempo its precision.
const COMB_TEETH: usize = 4;
/// Where tempos are most likely, and how wide that belief is (in octaves).
const PRIOR_BPM: f32 = 130.0;
const PRIOR_OCTAVES: f32 = 0.6;
/// How much a repeat at half the beat counts toward a tempo.
const HALF_BEAT_BONUS: f32 = 0.5;

/// Below this the tempo is not trusted enough to act on.
const LOCK_CONFIDENCE: f32 = 0.25;
/// Once locked, only bursts this close to the beat count as kicks. That
/// keeps off-beat bass notes from flashing the screen.
const BEAT_WINDOW: f32 = 0.2;

#[derive(Clone, Copy, Debug, Default)]
pub struct BeatOut {
    /// A kick landed in this frame.
    pub kick: bool,
    /// Tempo, or 0 while unknown.
    pub bpm: f32,
    /// How periodic the low end is, 0..1.
    pub confidence: f32,
    /// Position inside the current beat, 0..1.
    pub beat_phase: f32,
    /// Beats counted since the tempo was first found.
    pub beat_count: u64,
}

pub struct BeatTracker {
    hop_rate: f32,
    /// Recent flux, newest at the back.
    flux: VecDeque<f32>,
    flux_peak: f32,
    peak_decay: f32,
    frames_since_kick: usize,
    frames_since_tempo: usize,
    bpm: f32,
    confidence: f32,
    phase: f32,
    beat_count: u64,
    /// Tempo estimates in a row that disagreed with the current tempo.
    disagreements: u32,
    scratch: Vec<f32>,
}

impl BeatTracker {
    pub fn new(hop_rate: f32) -> Self {
        Self {
            hop_rate,
            flux: VecDeque::with_capacity((HISTORY_SECS * hop_rate) as usize + 1),
            flux_peak: 0.0,
            peak_decay: 0.5f32.powf(1.0 / (5.0 * hop_rate)),
            frames_since_kick: usize::MAX / 2,
            frames_since_tempo: 0,
            bpm: 0.0,
            confidence: 0.0,
            phase: 0.0,
            beat_count: 0,
            disagreements: 0,
            scratch: Vec::new(),
        }
    }

    pub fn push(&mut self, flux_low: f32) -> BeatOut {
        let threshold = self.kick_threshold();
        self.flux_peak = (self.flux_peak * self.peak_decay).max(flux_low);
        self.flux.push_back(flux_low);
        if self.flux.len() > (HISTORY_SECS * self.hop_rate) as usize {
            self.flux.pop_front();
        }

        self.frames_since_tempo += 1;
        if self.frames_since_tempo as f32 >= TEMPO_UPDATE_SECS * self.hop_rate {
            self.frames_since_tempo = 0;
            self.update_tempo();
        }

        if self.bpm > 0.0 {
            self.phase += self.bpm / 60.0 / self.hop_rate;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                self.beat_count += 1;
            }
        }

        self.frames_since_kick += 1;
        let burst = flux_low > threshold
            && self.frames_since_kick as f32 >= REFRACTORY_SECS * self.hop_rate;
        let on_the_beat = self.phase < BEAT_WINDOW || self.phase > 1.0 - BEAT_WINDOW;
        let kick = burst && (self.confidence < LOCK_CONFIDENCE || on_the_beat);
        if burst {
            self.frames_since_kick = 0;
        }

        BeatOut {
            kick,
            bpm: self.bpm,
            confidence: self.confidence,
            beat_phase: self.phase,
            beat_count: self.beat_count,
        }
    }

    /// How large a burst must be to count. Built from the median and the
    /// typical deviation of the last second and a half, which a few kicks
    /// cannot inflate the way they would inflate an average.
    fn kick_threshold(&mut self) -> f32 {
        let n = ((BACKGROUND_SECS * self.hop_rate) as usize).min(self.flux.len());
        self.scratch.clear();
        self.scratch.extend(self.flux.iter().rev().take(n));
        let median = median_of(&mut self.scratch);
        for v in &mut self.scratch {
            *v = (*v - median).abs();
        }
        let deviation = median_of(&mut self.scratch);
        (median + 5.0 * deviation + 0.15 * self.flux_peak).max(1e-4)
    }

    fn update_tempo(&mut self) {
        let Some((bpm, confidence)) = self.estimate_tempo() else {
            self.confidence *= 0.5;
            return;
        };
        self.confidence += 0.5 * (confidence - self.confidence);
        if confidence < LOCK_CONFIDENCE {
            return; // keep counting at the tempo we had
        }

        let first_lock = self.bpm == 0.0;
        if first_lock {
            self.bpm = bpm;
        } else if (bpm - self.bpm).abs() < 2.0 {
            self.bpm += 0.3 * (bpm - self.bpm);
            self.disagreements = 0;
        } else {
            // One odd estimate is noise; a second of them is a new tempo.
            self.disagreements += 1;
            if self.disagreements >= 4 {
                self.bpm = bpm;
                self.disagreements = 0;
            }
        }

        if let Some(target) = self.estimate_phase() {
            if first_lock {
                self.phase = target;
            } else {
                // Shortest way round the circle from where we are to the target.
                let error = (target - self.phase + 0.5).rem_euclid(1.0) - 0.5;
                let corrected = self.phase + 0.35 * error;
                if corrected >= 1.0 {
                    self.beat_count += 1;
                }
                self.phase = corrected.max(0.0).rem_euclid(1.0);
            }
        }
    }

    /// Finds the tempo whose beat length best matches the repeats in the
    /// recent flux. Returns (bpm, confidence).
    fn estimate_tempo(&mut self) -> Option<(f32, f32)> {
        let n = self.flux.len();
        if (n as f32) < 2.0 * self.hop_rate {
            return None;
        }
        let mean = self.flux.iter().sum::<f32>() / n as f32;
        let x: Vec<f32> = self.flux.iter().map(|v| v - mean).collect();

        // A repeat distance must leave at least a second of overlap.
        let usable = n as f32 - self.hop_rate;
        let longest = (self.frames_per_beat(MIN_BPM) * COMB_TEETH as f32).min(usable);
        let lags = longest.ceil() as usize + 2;
        // ac[lag]: how well the flux matches itself shifted by `lag` frames.
        let ac: Vec<f32> = (0..lags.min(n))
            .map(|lag| {
                let sum: f32 = x[lag..].iter().zip(&x).map(|(a, b)| a * b).sum();
                sum / (n - lag) as f32
            })
            .collect();
        if ac[0] < 1e-12 {
            return None;
        }
        let at = |lag: f32| {
            let i = lag as usize;
            match (ac.get(i), ac.get(i + 1)) {
                (Some(a), Some(b)) => a + (b - a) * lag.fract(),
                _ => 0.0,
            }
        };

        let steps = ((MAX_BPM - MIN_BPM) / BPM_STEP) as usize + 1;
        let mut scores = Vec::with_capacity(steps);
        for i in 0..steps {
            let bpm = MIN_BPM + i as f32 * BPM_STEP;
            let beat = self.frames_per_beat(bpm);
            let teeth = ((usable / beat) as usize).min(COMB_TEETH);
            if teeth == 0 {
                scores.push((0.0, 0.0));
                continue;
            }
            let match_ = (1..=teeth).map(|k| at(k as f32 * beat)).sum::<f32>() / teeth as f32;
            // Dance music divides the beat in two. If things also repeat at
            // half this beat (an off-beat bass, a hi-hat) that speaks for this
            // tempo and against one a third slower or faster, where the half
            // beat would fall on nothing.
            let half_beat = at(beat * 0.5).max(0.0);
            let octaves = (bpm / PRIOR_BPM).log2() / PRIOR_OCTAVES;
            let prior = (-0.5 * octaves * octaves).exp();
            scores.push(((match_ + HALF_BEAT_BONUS * half_beat) * prior, match_));
        }
        let best = (0..steps).max_by(|a, b| scores[*a].0.total_cmp(&scores[*b].0))?;
        let (weighted, match_) = scores[best];
        if weighted <= 0.0 {
            return None;
        }

        // Fit a parabola through the best score and its neighbours to land
        // between grid steps.
        let mut bpm = MIN_BPM + best as f32 * BPM_STEP;
        if best > 0 && best + 1 < steps {
            let (l, c, r) = (scores[best - 1].0, weighted, scores[best + 1].0);
            let curve = l - 2.0 * c + r;
            if curve < 0.0 {
                bpm += 0.5 * (l - r) / curve * BPM_STEP;
            }
        }
        Some((bpm, (match_ / ac[0]).clamp(0.0, 1.0)))
    }

    /// Where in the beat we are right now, judged by where the strong bursts
    /// of the last few seconds fell.
    ///
    /// Each burst is placed on a circle by its position in the beat and
    /// weighted by its strength; the direction of the total is the phase of
    /// the kicks. Off-beat bass notes pull the other way but are weaker.
    fn estimate_phase(&self) -> Option<f32> {
        let beat = self.frames_per_beat(self.bpm);
        let window = (PHASE_WINDOW_SECS * self.hop_rate) as usize;
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (age, v) in self.flux.iter().rev().take(window).enumerate() {
            let angle = -TAU * age as f32 / beat;
            re += v * v * angle.cos();
            im += v * v * angle.sin();
        }
        if re * re + im * im < 1e-12 {
            return None;
        }
        let kicks_at = (im.atan2(re) / TAU).rem_euclid(1.0);
        Some((1.0 - kicks_at).rem_euclid(1.0))
    }

    fn frames_per_beat(&self, bpm: f32) -> f32 {
        self.hop_rate * 60.0 / bpm
    }
}

fn median_of(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mid = values.len() / 2;
    *values.select_nth_unstable_by(mid, f32::total_cmp).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::spectrum::Spectrum;
    use crate::synth::{kick_train, noise, test_track};

    /// Runs a signal through the spectrum and the tracker; returns every frame's output.
    fn track(signal: &[f32], sr: u32) -> Vec<BeatOut> {
        let mut spectrum = Spectrum::new(sr);
        let mut beat = BeatTracker::new(spectrum.hop_rate());
        let mut out = Vec::new();
        spectrum.push(signal, |f| out.push(beat.push(f.flux_low)));
        out
    }

    #[test]
    fn detects_128_bpm() {
        let out = track(&kick_train(128.0, 20.0, 48000), 48000);
        let last = out.last().unwrap();
        assert!((127.0..=129.0).contains(&last.bpm), "bpm {}", last.bpm);
        assert!(last.confidence > 0.5, "confidence {}", last.confidence);
    }

    #[test]
    fn detects_140_bpm_at_44100() {
        let out = track(&kick_train(140.0, 20.0, 44100), 44100);
        let last = out.last().unwrap();
        assert!((139.0..=141.0).contains(&last.bpm), "bpm {}", last.bpm);
    }

    #[test]
    fn detects_other_tempos() {
        for bpm in [100.0, 122.0, 133.0, 150.0, 174.0] {
            let out = track(&kick_train(bpm, 20.0, 48000), 48000);
            let got = out.last().unwrap().bpm;
            assert!((got - bpm).abs() <= 1.0, "wanted {bpm}, got {got}");
        }
    }

    #[test]
    fn kick_count_matches() {
        let out = track(&kick_train(128.0, 20.0, 48000), 48000);
        let kicks = out.iter().filter(|o| o.kick).count();
        assert!((40..=44).contains(&kicks), "{kicks} kicks");
    }

    #[test]
    fn phase_locks_to_kicks() {
        let sr = 48000;
        let out = track(&kick_train(128.0, 20.0, sr), sr);
        let hop_rate = sr as f32 / 512.0;
        let late: Vec<f32> = out
            .iter()
            .enumerate()
            .filter(|(i, o)| o.kick && *i as f32 / hop_rate > 10.0)
            .map(|(_, o)| o.beat_phase)
            .collect();
        assert!(late.len() > 15);
        for phase in late {
            assert!(!(0.12..=0.88).contains(&phase), "kick at phase {phase}");
        }
    }

    #[test]
    fn beat_count_follows_the_tempo() {
        let out = track(&kick_train(128.0, 20.0, 48000), 48000);
        // 20 s at 128 BPM is 42.7 beats; the first few pass before the lock.
        let beats = out.last().unwrap().beat_count;
        assert!((34..=43).contains(&beats), "{beats} beats");
    }

    #[test]
    fn silence_has_no_beat() {
        let out = track(&vec![0.0; 10 * 48000], 48000);
        assert_eq!(out.iter().filter(|o| o.kick).count(), 0);
        let last = out.last().unwrap();
        assert!(last.confidence < 0.1, "confidence {}", last.confidence);
        assert_eq!(last.bpm, 0.0);
        assert_eq!(last.beat_count, 0);
    }

    #[test]
    fn white_noise_low_confidence() {
        let out = track(&noise(10.0, 48000, 0.5, 5), 48000);
        let last = out.last().unwrap();
        // Noise must never be trusted as a tempo: stay under the lock level
        // the whole time, not merely under some other number at the end.
        let peak = out.iter().map(|o| o.confidence).fold(0.0, f32::max);
        assert!(peak < LOCK_CONFIDENCE, "confidence reached {peak}");
        assert_eq!(last.bpm, 0.0);
        let kicks = out.iter().filter(|o| o.kick).count();
        assert!(kicks < 10, "{kicks} kicks in noise");
    }

    #[test]
    fn a_loud_off_beat_bass_does_not_pull_fast_tempos_down() {
        // With a bass as loud as the kick, something lands every half beat.
        // At 150 to 180 BPM that also fits two thirds of the tempo, which sits
        // nearer to where tempos usually are; it must not win.
        for bpm in [128.0, 140.0, 150.0, 160.0, 170.0, 174.0] {
            for bass in [0.5, 1.0] {
                let out = track(&crate::synth::kick_and_bass(bpm, 20.0, 48000, bass), 48000);
                let got = out.last().unwrap().bpm;
                assert!(
                    (got - bpm).abs() <= 1.0,
                    "{bpm} BPM, bass {bass}: got {got}"
                );
            }
        }
    }

    #[test]
    fn follows_a_tempo_change() {
        let sr = 48000;
        let mut signal = kick_train(128.0, 12.0, sr);
        signal.extend(kick_train(140.0, 12.0, sr));
        let out = track(&signal, sr);
        let hop_rate = sr as f32 / 512.0;
        let at = |secs: f32| out[(secs * hop_rate) as usize].bpm;
        assert!((127.0..=129.0).contains(&at(11.5)), "before: {}", at(11.5));
        assert!(
            (139.0..=141.0).contains(&at(21.0)),
            "9 s after the change: {}",
            at(21.0)
        );
    }

    #[test]
    fn phase_recovers_when_the_beat_shifts() {
        // Exactly 24 beats of kicks, half a beat of nothing, then the same
        // kicks again (as after a mix or a skipped beat). The tempo has not
        // changed, so only the phase correction can bring the count back
        // onto the kick; without it every kick would land at phase 0.5.
        let sr = 48000;
        let beat_samples = 22_500; // 128 BPM at 48 kHz
        let first_part = 24 * beat_samples;
        let mut signal = kick_train(128.0, first_part as f32 / sr as f32, sr);
        assert_eq!(signal.len(), first_part);
        signal.extend(vec![0.0; beat_samples / 2]);
        signal.extend(kick_train(128.0, 14.0, sr));
        let out = track(&signal, sr);

        let kick_frame = |k: usize| (first_part + beat_samples / 2 + k * beat_samples) / 512 + 1;
        let off_beat = |k: usize| {
            let phase = out[kick_frame(k)].beat_phase;
            phase.min(1.0 - phase)
        };
        // Right after the shift the count is still on the old grid...
        assert!(
            off_beat(1) > 0.3,
            "the shift did not shift anything: {}",
            off_beat(1)
        );
        // ...and ten seconds later it has moved onto the new one.
        let worst = (20..28).map(off_beat).fold(0.0, f32::max);
        assert!(
            worst < 0.15,
            "ten seconds after the shift a kick still lands {worst} of a beat off"
        );
    }

    #[test]
    fn off_beat_bass_does_not_double_the_tempo_or_the_kicks() {
        // The groove of the test track has a bass note between every two kicks.
        let sr = 48000;
        let track_samples = test_track(sr);
        let groove = &track_samples[..(7.5 * sr as f32) as usize];
        let mut looped = Vec::new();
        for _ in 0..3 {
            looped.extend_from_slice(groove);
        }
        let out = track(&looped, sr);
        let last = out.last().unwrap();
        assert!((127.0..=129.0).contains(&last.bpm), "bpm {}", last.bpm);
        let hop_rate = sr as f32 / 512.0;
        let late_kicks = out
            .iter()
            .enumerate()
            .filter(|(i, o)| o.kick && *i as f32 / hop_rate > 12.5)
            .count();
        // 10 s at 128 BPM is 21.3 beats.
        assert!((19..=23).contains(&late_kicks), "{late_kicks} kicks");
    }

    #[test]
    fn tempo_survives_a_breakdown() {
        // 10 s of kicks, 4 s without, then kicks again: the count keeps going.
        let sr = 48000;
        let mut signal = kick_train(128.0, 10.0, sr);
        let at_gap = signal.len();
        signal.extend(vec![0.0; 4 * sr as usize]);
        let out = track(&signal, sr);
        let before = out[at_gap / 512 - 1];
        let after = out.last().unwrap();
        assert!((127.0..=129.0).contains(&after.bpm), "bpm {}", after.bpm);
        let counted = after.beat_count - before.beat_count;
        assert!((8..=9).contains(&counted), "{counted} beats in 4 s");
    }
}
