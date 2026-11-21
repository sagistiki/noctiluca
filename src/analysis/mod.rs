//! Turns raw audio into the handful of numbers the visuals react to.
//!
//! Nothing in here knows about windows, GPUs or audio devices: samples go in,
//! `Features` come out. That keeps it testable with synthesized signals.

pub mod beat;
pub mod mood;
pub mod spectrum;
pub mod vibe;

use beat::BeatTracker;
use mood::MoodDetector;
use spectrum::{BANDS, FFT_SIZE, HOP, SPECTRUM_BINS, Spectrum};
pub use vibe::Vibe;
use vibe::VibeMeter;

pub const WAVE_SAMPLES: usize = 256;

/// How quickly a value falls back after a peak, in seconds.
const BAND_RELEASE_SECS: f32 = 0.12;
const SPECTRUM_RELEASE_SECS: f32 = 0.08;
const PULSE_RELEASE_SECS: f32 = 0.15;
/// Full scale is 1.0; anything far beyond it is a fault, not music.
const MAX_SAMPLE: f32 = 4.0;
/// Longer than this without any samples counts as silence.
const STARVE_GRACE_SECS: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Silence,
    Groove,
    Build,
    Drop,
    Breakdown,
}

/// Everything the visuals know about the music at one moment.
#[derive(Clone, Copy, Debug)]
pub struct Features {
    /// Sub, bass, mid, high: 0..1, smoothed for display.
    pub bands: [f32; BANDS],
    /// Overall loudness, 0..1.
    pub level: f32,
    /// Log-spaced spectrum, 0..1.
    pub spectrum: [f32; SPECTRUM_BINS],
    /// The most recent slice of the waveform, -1..1.
    pub wave: [f32; WAVE_SAMPLES],
    /// True for the one frame in which a kick was detected.
    pub kick: bool,
    /// 1.0 on a kick, then fading out.
    pub pulse: f32,
    /// Tempo, or 0 when unknown.
    pub bpm: f32,
    /// How sure the tempo is, 0..1.
    pub confidence: f32,
    /// Position inside the current beat, 0..1.
    pub beat_phase: f32,
    /// Beats counted since the start.
    pub beat_count: u64,
    pub mood: Mood,
    /// True for the one frame in which a drop was detected.
    pub drop: bool,
    /// How the music feels over the last few seconds (see vibe.rs).
    pub vibe: Vibe,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            bands: [0.0; BANDS],
            level: 0.0,
            spectrum: [0.0; SPECTRUM_BINS],
            wave: [0.0; WAVE_SAMPLES],
            kick: false,
            pulse: 0.0,
            bpm: 0.0,
            confidence: 0.0,
            beat_phase: 0.0,
            beat_count: 0,
            mood: Mood::Silence,
            drop: false,
            vibe: Vibe::default(),
        }
    }
}

pub struct Analyzer {
    spectrum: Spectrum,
    beat: BeatTracker,
    mood: MoodDetector,
    vibe: VibeMeter,
    features: Features,
    sample_rate: u32,
    hop_secs: f32,
    starved_secs: f32,
    /// Reused buffer for the sanitised input.
    clean: Vec<f32>,
    /// The newest `FFT_SIZE` samples, for the waveform display.
    recent: Vec<f32>,
    band_release: f32,
    spectrum_release: f32,
    pulse_release: f32,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let hop_secs = HOP as f32 / sample_rate as f32;
        let spectrum = Spectrum::new(sample_rate);
        Self {
            beat: BeatTracker::new(spectrum.hop_rate()),
            mood: MoodDetector::new(),
            vibe: VibeMeter::new(hop_secs),
            spectrum,
            features: Features::default(),
            sample_rate,
            hop_secs,
            starved_secs: 0.0,
            clean: Vec::new(),
            pulse_release: (-hop_secs / PULSE_RELEASE_SECS).exp(),
            recent: vec![0.0; FFT_SIZE],
            band_release: (-hop_secs / BAND_RELEASE_SECS).exp(),
            spectrum_release: (-hop_secs / SPECTRUM_RELEASE_SECS).exp(),
        }
    }

    pub fn push(&mut self, mono: &[f32]) {
        if mono.is_empty() {
            return;
        }
        self.starved_secs = 0.0;
        self.feed(mono);
    }

    fn feed(&mut self, raw: &[f32]) {
        // One infinite sample would turn every running peak infinite, and
        // they never come back down. Nothing downstream should have to care,
        // so bad samples are replaced here, once. (`take` lends the buffer
        // out of `self` so the rest of `self` can still be borrowed.)
        let mut clean = std::mem::take(&mut self.clean);
        clean.clear();
        clean.extend(raw.iter().map(|s| {
            if s.is_finite() {
                s.clamp(-MAX_SAMPLE, MAX_SAMPLE)
            } else {
                0.0
            }
        }));
        self.analyse(&clean);
        self.clean = clean;
    }

    fn analyse(&mut self, mono: &[f32]) {
        // Keep only the newest samples for the waveform.
        let keep = self.recent.len();
        if mono.len() >= keep {
            self.recent.copy_from_slice(&mono[mono.len() - keep..]);
        } else {
            self.recent.rotate_left(mono.len());
            self.recent[keep - mono.len()..].copy_from_slice(mono);
        }

        // Borrow the fields one by one: the closure below needs several of
        // them mutably while `spectrum` is itself borrowed mutably.
        let Self {
            spectrum,
            beat,
            mood,
            vibe,
            features: f,
            ..
        } = self;
        let (band_release, spectrum_release, pulse_release, hop_secs) = (
            self.band_release,
            self.spectrum_release,
            self.pulse_release,
            self.hop_secs,
        );
        spectrum.push(mono, |frame| {
            // Rise at once, fall slowly: a hit reads as a hit, not a flicker.
            for (shown, new) in f.bands.iter_mut().zip(frame.bands) {
                *shown = new.max(*shown * band_release);
            }
            for (shown, new) in f.spectrum.iter_mut().zip(frame.spectrum) {
                *shown = new.max(*shown * spectrum_release);
            }
            f.level = frame.level.max(f.level * band_release);

            let b = beat.push(frame.flux_low);
            f.kick |= b.kick;
            f.pulse = if b.kick { 1.0 } else { f.pulse * pulse_release };
            f.bpm = b.bpm;
            f.confidence = b.confidence;
            f.beat_phase = b.beat_phase;
            f.beat_count = b.beat_count;

            let (now, dropped) = mood.push(frame, &b, hop_secs);
            f.mood = now;
            f.drop |= dropped;
            f.vibe = vibe.push(frame, b.kick, b.bpm, now);
        });
    }

    /// Call instead of `push` on a frame where no audio arrived.
    ///
    /// Audio comes in bursts, so a frame or two with nothing is normal and
    /// ignored. But if nothing arrives for longer (a cable pulled, a device
    /// that went away) that is silence, and it is analysed as silence, so
    /// that everything fades out instead of freezing at its last value.
    pub fn idle(&mut self, dt: f32) {
        self.starved_secs += dt;
        if self.starved_secs > STARVE_GRACE_SECS {
            const ZEROS: [f32; 4096] = [0.0; 4096];
            let n = ((dt * self.sample_rate as f32) as usize).min(ZEROS.len());
            self.feed(&ZEROS[..n]);
        }
    }

    /// The raw measures behind the vibe: averages over its window.
    pub fn vibe_window(&self) -> Option<vibe::Second> {
        self.vibe.window_average()
    }

    /// How long it has been since any audio arrived.
    pub fn starved_secs(&self) -> f32 {
        self.starved_secs
    }

    /// The current state. One-frame events (`kick`, `drop`) are cleared after
    /// they have been returned once.
    pub fn features(&mut self) -> Features {
        let peak = self.recent.iter().fold(0.01f32, |m, s| m.max(s.abs()));
        let stride = self.recent.len() / WAVE_SAMPLES;
        for (out, chunk) in self
            .features
            .wave
            .iter_mut()
            .zip(self.recent.chunks(stride))
        {
            *out = chunk[0] / peak;
        }
        let out = self.features;
        self.features.kick = false;
        self.features.drop = false;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::sine;

    #[test]
    fn bands_fall_back_after_the_sound_stops() {
        let mut a = Analyzer::new(48000);
        a.push(&sine(100.0, 1.0, 48000, 0.8));
        let during = a.features().bands[1];
        a.push(&vec![0.0; 48000]);
        let after = a.features().bands[1];
        assert!(during > 0.7, "during {during}");
        assert!(after < 0.05, "after {after}");
    }

    #[test]
    fn wave_is_normalised_and_finite() {
        let mut a = Analyzer::new(48000);
        a.push(&sine(440.0, 0.2, 48000, 0.05));
        let wave = a.features().wave;
        let peak = wave.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.8 && peak <= 1.0, "peak {peak}");
    }

    /// Feeds a signal one video frame (1/60 s) at a time, like the app does,
    /// and returns (seconds, features) for each.
    fn play(signal: &[f32], sr: u32) -> Vec<(f32, Features)> {
        let mut a = Analyzer::new(sr);
        let step = sr as usize / 60;
        signal
            .chunks(step)
            .enumerate()
            .map(|(i, chunk)| {
                a.push(chunk);
                ((i + 1) as f32 / 60.0, a.features())
            })
            .collect()
    }

    #[test]
    fn test_track_mood_sequence() {
        let frames = play(&crate::synth::test_track(48000), 48000);
        let mut seen = Vec::new();
        for (_, f) in &frames {
            if seen.last() != Some(&f.mood) {
                seen.push(f.mood);
            }
        }
        assert_eq!(
            seen,
            vec![
                Mood::Silence,
                Mood::Groove,
                Mood::Breakdown,
                Mood::Build,
                Mood::Drop,
                Mood::Groove
            ]
        );
        // Beat 32 at 128 BPM is 15.0 s.
        let drops: Vec<f32> = frames
            .iter()
            .filter(|(_, f)| f.drop)
            .map(|(t, _)| *t)
            .collect();
        assert_eq!(drops.len(), 1, "drops at {drops:?}");
        assert!((15.0..=15.3).contains(&drops[0]), "drop at {}", drops[0]);
    }

    #[test]
    fn test_track_moods_are_detected_promptly() {
        // The track changes at 7.5 s (breakdown), 11.25 s (build), 15.0 s (drop).
        let frames = play(&crate::synth::test_track(48000), 48000);
        let first = |mood: Mood| frames.iter().find(|(_, f)| f.mood == mood).unwrap().0;
        let (breakdown, build, drop) = (
            first(Mood::Breakdown),
            first(Mood::Build),
            first(Mood::Drop),
        );
        println!("breakdown {breakdown:.2}  build {build:.2}  drop {drop:.2}");
        assert!((7.5..9.5).contains(&breakdown), "breakdown at {breakdown}");
        assert!((11.25..14.0).contains(&build), "build at {build}");
        assert!((15.0..15.3).contains(&drop), "drop at {drop}");
    }

    fn scaled(signal: &[f32], gain: f32) -> Vec<f32> {
        signal.iter().map(|s| s * gain).collect()
    }

    /// The first 16 beats of the test track, repeated.
    fn groove_loop(sr: u32, times: usize) -> Vec<f32> {
        let track = crate::synth::test_track(sr);
        let groove = &track[..(7.5 * sr as f32) as usize];
        (0..times).flat_map(|_| groove.iter().copied()).collect()
    }

    fn lows_gone(mood: Mood) -> bool {
        matches!(mood, Mood::Breakdown | Mood::Build)
    }

    #[test]
    fn mood_does_not_depend_on_input_level() {
        // A hot line input and a quiet microphone must tell the same story.
        for gain in [1.0, 0.1, 0.02, 0.005] {
            let frames = play(&scaled(&crate::synth::test_track(48000), gain), 48000);
            let mut seen = Vec::new();
            for (_, f) in &frames {
                if seen.last() != Some(&f.mood) {
                    seen.push(f.mood);
                }
            }
            assert_eq!(
                seen,
                vec![
                    Mood::Silence,
                    Mood::Groove,
                    Mood::Breakdown,
                    Mood::Build,
                    Mood::Drop,
                    Mood::Groove
                ],
                "at gain {gain}"
            );
            assert_eq!(
                frames.iter().filter(|(_, f)| f.drop).count(),
                1,
                "at gain {gain}"
            );
        }
    }

    #[test]
    fn a_quieter_return_still_ends_the_breakdown() {
        // Groove, breakdown, build, and then the kick comes back at a third
        // of its old level (the next track is mastered quieter, say).
        let sr = 48000;
        let track = crate::synth::test_track(sr);
        let mut signal = track[..15 * sr as usize].to_vec();
        signal.extend(scaled(&groove_loop(sr, 3), 0.3));
        let frames = play(&signal, sr);
        let stuck: Vec<f32> = frames
            .iter()
            .filter(|(t, f)| *t > 18.0 && lows_gone(f.mood))
            .map(|(t, _)| *t)
            .collect();
        assert!(
            stuck.is_empty(),
            "still in a breakdown at {:?}..{:?}",
            stuck.first(),
            stuck.last()
        );
        // It was a drop, even if recognised a moment late.
        let drops: Vec<f32> = frames
            .iter()
            .filter(|(_, f)| f.drop)
            .map(|(t, _)| *t)
            .collect();
        assert_eq!(drops.len(), 1, "{drops:?}");
        assert!((15.0..18.0).contains(&drops[0]), "drop at {}", drops[0]);
    }

    #[test]
    fn turning_the_volume_down_is_not_a_breakdown() {
        let sr = 48000;
        let mut signal = groove_loop(sr, 2);
        signal.extend(scaled(&groove_loop(sr, 4), 0.25));
        let frames = play(&signal, sr);
        assert!(frames.iter().all(|(_, f)| !lows_gone(f.mood)));
        assert_eq!(frames.iter().filter(|(_, f)| f.drop).count(), 0);
    }

    #[test]
    fn a_breakdown_cannot_last_forever() {
        // Kick, then a pad that never ends. Whatever this is, after a minute
        // and a half it is the new normal, not a breakdown.
        let sr = 48000;
        let mut signal = groove_loop(sr, 2);
        let pad: Vec<f32> = crate::synth::sine(300.0, 120.0, sr, 0.2);
        signal.extend(pad);
        let frames = play(&signal, sr);
        assert!(frames.iter().any(|(_, f)| f.mood == Mood::Breakdown));
        assert_eq!(frames.last().unwrap().1.mood, Mood::Groove);
    }

    #[test]
    fn hiss_after_music_is_silence() {
        let sr = 48000;
        let mut signal = crate::synth::kick_train(128.0, 6.0, sr);
        signal.extend(crate::synth::noise(10.0, sr, 0.0005, 9));
        let frames = play(&signal, sr);
        assert_eq!(frames.last().unwrap().1.mood, Mood::Silence);
    }

    #[test]
    fn when_audio_stops_arriving_everything_settles() {
        // The cable is pulled: no samples at all, not even zeros.
        let sr = 48000;
        let mut a = Analyzer::new(sr);
        for chunk in groove_loop(sr, 2).chunks(800) {
            a.push(chunk);
            a.features();
        }
        let before = a.features();
        assert!(before.confidence > 0.5 && before.mood == Mood::Groove);
        let mut after = before;
        for _ in 0..60 * 30 {
            a.idle(1.0 / 60.0);
            after = a.features();
        }
        assert_eq!(after.mood, Mood::Silence);
        assert!(
            after.pulse < 0.01 && after.level < 0.01,
            "pulse {} level {}",
            after.pulse,
            after.level
        );
        assert!(after.bands.iter().all(|b| *b < 0.01), "{:?}", after.bands);
        assert!(after.confidence < 0.1, "confidence {}", after.confidence);
        assert!(a.starved_secs() > 29.0);
        // And it wakes up again when audio returns.
        for chunk in groove_loop(sr, 2).chunks(800) {
            a.push(chunk);
            a.features();
        }
        assert_eq!(a.starved_secs(), 0.0);
        assert_eq!(a.features().mood, Mood::Groove);
    }

    #[test]
    fn a_brief_gap_in_delivery_is_ignored() {
        // Audio arrives in bursts; a frame or two without samples is normal.
        let sr = 48000;
        let mut whole = Analyzer::new(sr);
        let mut gappy = Analyzer::new(sr);
        for chunk in groove_loop(sr, 1).chunks(1600) {
            whole.push(chunk);
            gappy.idle(1.0 / 60.0);
            gappy.push(chunk);
        }
        assert_eq!(whole.features().bands, gappy.features().bands);
        assert_eq!(whole.features().beat_count, gappy.features().beat_count);
    }

    #[test]
    fn the_second_drop_is_caught_as_fast_as_the_first() {
        // The track played twice: drops at 15.0 s and 45.0 s.
        let track = crate::synth::test_track(48000);
        let twice: Vec<f32> = track.iter().chain(&track).copied().collect();
        let frames = play(&twice, 48000);
        let drops: Vec<f32> = frames
            .iter()
            .filter(|(_, f)| f.drop)
            .map(|(t, _)| *t)
            .collect();
        println!("drops at {drops:?}");
        assert_eq!(drops.len(), 2, "{drops:?}");
        assert!(
            (15.0..15.3).contains(&drops[0]),
            "first drop at {}",
            drops[0]
        );
        assert!(
            (45.0..45.3).contains(&drops[1]),
            "second drop at {}",
            drops[1]
        );
    }

    #[test]
    fn broken_samples_do_not_kill_the_analysis() {
        // One infinite or not-a-number sample (a faulty driver, a corrupt
        // file) must cost a moment, not the rest of the night.
        for bad in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 1.0e30] {
            let sr = 48000;
            let mut signal = groove_loop(sr, 4);
            signal[10 * sr as usize] = bad;
            let frames = play(&signal, sr);
            let (_, last) = frames.last().unwrap();
            assert!(
                (127.0..=129.0).contains(&last.bpm),
                "after {bad}: bpm {}",
                last.bpm
            );
            assert!(
                last.confidence.is_finite() && last.confidence > 0.5,
                "after {bad}"
            );
            let late_kicks = frames.iter().filter(|(t, f)| f.kick && *t > 20.0).count();
            assert!(
                late_kicks >= 18,
                "after {bad}: {late_kicks} kicks in the last 10 s"
            );
            for (_, f) in &frames {
                let all = f
                    .bands
                    .iter()
                    .chain(&f.spectrum)
                    .chain(&f.wave)
                    .chain([&f.level, &f.pulse]);
                assert!(
                    all.into_iter().all(|v| v.is_finite()),
                    "after {bad}: a value went non-finite"
                );
            }
        }
    }

    /// What a laptop microphone hears from small speakers: almost nothing
    /// below `cutoff` Hz (two high-pass stages), plus room hiss.
    fn through_a_small_speaker(signal: &[f32], sr: u32, cutoff: f32) -> Vec<f32> {
        let a = 1.0 / (1.0 + std::f32::consts::TAU * cutoff / sr as f32);
        let hiss = crate::synth::noise(signal.len() as f32 / sr as f32 + 1.0, sr, 0.003, 21);
        let (mut x1, mut y1, mut x2, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        signal
            .iter()
            .zip(&hiss)
            .map(|(x, n)| {
                y1 = a * (y1 + x - x1);
                x1 = *x;
                y2 = a * (y2 + y1 - x2);
                x2 = y1;
                0.3 * y2 + n
            })
            .collect()
    }

    #[test]
    fn mood_survives_a_small_speaker_and_room_hiss() {
        // Ruled out as the reason real music showed no breakdown: even with
        // everything under 300 Hz mostly gone, the story is still told.
        for cutoff in [120.0, 200.0, 300.0] {
            let heard = through_a_small_speaker(&crate::synth::test_track(48000), 48000, cutoff);
            let frames = play(&heard, 48000);
            let mut seen = Vec::new();
            for (_, f) in &frames {
                if seen.last() != Some(&f.mood) {
                    seen.push(f.mood);
                }
            }
            assert_eq!(
                seen,
                vec![
                    Mood::Silence,
                    Mood::Groove,
                    Mood::Breakdown,
                    Mood::Build,
                    Mood::Drop,
                    Mood::Groove
                ],
                "with a {cutoff} Hz high-pass"
            );
            assert!((127.0..=129.0).contains(&frames.last().unwrap().1.bpm));
        }
    }

    /// The test track cut into its parts: groove, breakdown, build, drop.
    fn parts(sr: u32) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        let t = crate::synth::test_track(sr);
        let at = |secs: f32| (secs * sr as f32) as usize;
        (
            t[..at(7.5)].to_vec(),
            t[at(7.5)..at(11.25)].to_vec(),
            t[at(11.25)..at(15.0)].to_vec(),
            t[at(15.0)..].to_vec(),
        )
    }

    fn drops(frames: &[(f32, Features)]) -> Vec<f32> {
        frames
            .iter()
            .filter(|(_, f)| f.drop)
            .map(|(t, _)| *t)
            .collect()
    }

    // The next four come from a real recording through room speakers
    // (2026-11-21), where real drops were missed or came early.

    #[test]
    fn a_near_silent_gap_inside_a_breakdown_keeps_the_breakdown() {
        // The room went almost silent for four seconds in the middle of a
        // breakdown. That was taken as "the music stopped", which wiped the
        // memory of the groove, so the drop after it was never seen.
        let sr = 48000;
        let (groove, breakdown, build, drop) = parts(sr);
        let mut signal = [groove.clone(), groove].concat();
        signal.extend(&breakdown);
        signal.extend(crate::synth::noise(4.0, sr, 0.0005, 13));
        signal.extend(&build);
        let drop_at = signal.len() as f32 / sr as f32;
        signal.extend(&drop);
        let frames = play(&signal, sr);
        let after_start: Vec<Mood> = frames
            .iter()
            .filter(|(t, _)| *t > 1.0)
            .map(|(_, f)| f.mood)
            .collect();
        assert!(
            !after_start.contains(&Mood::Silence),
            "a quiet moment was called silence"
        );
        let found = drops(&frames);
        assert_eq!(
            found.len(),
            1,
            "drops at {found:?}, expected one at {drop_at}"
        );
        assert!(
            (drop_at..drop_at + 0.5).contains(&found[0]),
            "drop at {}, expected {drop_at}",
            found[0]
        );
    }

    #[test]
    fn bass_coming_back_before_the_kick_is_still_one_drop() {
        // The bass returned three seconds before the kick. That was read as
        // "the low end crept back", not as a drop.
        let sr = 48000;
        let (groove, breakdown, build, _) = parts(sr);
        let mut signal = [groove.clone(), groove.clone(), breakdown, build].concat();
        let bass_at = signal.len() as f32 / sr as f32;
        signal.extend(crate::synth::bass_line(128.0, 3.0, sr));
        signal.extend([groove.clone(), groove].concat());
        let frames = play(&signal, sr);
        let found = drops(&frames);
        assert_eq!(found.len(), 1, "drops at {found:?}");
        assert!(
            (bass_at..bass_at + 3.5).contains(&found[0]),
            "drop at {}, the bass came at {bass_at}",
            found[0]
        );
    }

    #[test]
    fn a_short_dip_is_not_a_drop() {
        // Two seconds without the kick (a fill) and back: no drop.
        let sr = 48000;
        let (groove, breakdown, _, _) = parts(sr);
        let mut signal = [groove.clone(), groove.clone()].concat();
        signal.extend(&breakdown[..2 * sr as usize]);
        signal.extend([groove.clone(), groove].concat());
        let frames = play(&signal, sr);
        assert!(drops(&frames).is_empty(), "drops at {:?}", drops(&frames));
    }

    #[test]
    fn nothing_is_called_before_a_groove_has_locked() {
        // A quiet intro with a few stray thumps is not a breakdown, and the
        // next thump is not a drop.
        let sr = 48000;
        let mut signal = crate::synth::noise(18.0, sr, 0.002, 17);
        let kick = crate::synth::kick_train(60.0, 1.0, sr);
        for at in [2.0, 7.3, 9.1, 13.8, 14.4] {
            let start = (at * sr as f32) as usize;
            for (s, k) in signal[start..].iter_mut().zip(&kick) {
                *s += 0.3 * k;
            }
        }
        let frames = play(&signal, sr);
        assert!(
            frames
                .iter()
                .all(|(_, f)| !matches!(f.mood, Mood::Breakdown | Mood::Build | Mood::Drop))
        );
    }

    #[test]
    fn a_few_quiet_seconds_inside_music_are_not_silence() {
        let sr = 48000;
        let (groove, ..) = parts(sr);
        let mut signal = [groove.clone(), groove.clone()].concat();
        signal.extend(crate::synth::noise(5.0, sr, 0.0005, 19));
        signal.extend([groove.clone(), groove].concat());
        let frames = play(&signal, sr);
        assert!(
            frames
                .iter()
                .filter(|(t, _)| *t > 1.0)
                .all(|(_, f)| f.mood != Mood::Silence)
        );
    }

    #[test]
    fn steady_groove_never_drops() {
        let sr = 48000;
        let track = crate::synth::test_track(sr);
        let groove = &track[..(7.5 * sr as f32) as usize];
        let mut looped = Vec::new();
        for _ in 0..4 {
            looped.extend_from_slice(groove);
        }
        let frames = play(&looped, sr);
        assert_eq!(frames.iter().filter(|(_, f)| f.drop).count(), 0);
        assert!(
            frames
                .iter()
                .all(|(_, f)| matches!(f.mood, Mood::Silence | Mood::Groove))
        );
        assert_eq!(frames.last().unwrap().1.mood, Mood::Groove);
    }

    #[test]
    fn silence_after_music_is_silence() {
        let sr = 48000;
        let mut signal = crate::synth::kick_train(128.0, 6.0, sr);
        signal.extend(vec![0.0; 10 * sr as usize]);
        let frames = play(&signal, sr);
        assert_eq!(frames.last().unwrap().1.mood, Mood::Silence);
        assert_eq!(frames.iter().filter(|(_, f)| f.drop).count(), 0);
    }

    #[test]
    fn test_track_tempo_and_pulse() {
        let frames = play(&crate::synth::test_track(48000), 48000);
        let (_, at_7s) = frames[7 * 60];
        assert!((127.0..=129.0).contains(&at_7s.bpm), "bpm {}", at_7s.bpm);
        // Every kick reaches the app exactly once, and lifts the pulse.
        // Beats 8..16 (3.75 s .. 7.5 s), after the tempo has locked.
        let kicks = frames
            .iter()
            .filter(|(t, f)| f.kick && *t > 3.7 && *t < 7.45)
            .count();
        assert!((7..=9).contains(&kicks), "{kicks} kicks in beats 8..16");
        assert!(
            frames
                .iter()
                .filter(|(_, f)| f.kick)
                .all(|(_, f)| f.pulse > 0.8)
        );
    }

    #[test]
    fn short_pushes_equal_one_long_push() {
        let signal = sine(100.0, 1.0, 48000, 0.8);
        let mut whole = Analyzer::new(48000);
        whole.push(&signal);
        let mut pieces = Analyzer::new(48000);
        for chunk in signal.chunks(333) {
            pieces.push(chunk);
        }
        assert_eq!(whole.features().bands, pieces.features().bands);
    }
}
