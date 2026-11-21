//! Synthesized test signals. Nothing here is used by the live app; it exists
//! so the analysis can be tested against music whose tempo and structure we
//! know exactly.

use std::f32::consts::TAU;

pub fn sine(freq: f32, secs: f32, sr: u32, amp: f32) -> Vec<f32> {
    let n = (secs * sr as f32) as usize;
    (0..n)
        .map(|i| amp * (TAU * freq * i as f32 / sr as f32).sin())
        .collect()
}

/// White noise in -amp..amp. The same seed always gives the same samples.
pub fn noise(secs: f32, sr: u32, amp: f32, seed: u32) -> Vec<f32> {
    let n = (secs * sr as f32) as usize;
    let mut rng = Rng(seed.max(1));
    (0..n).map(|_| amp * rng.next()).collect()
}

/// A four-on-the-floor kick drum for `secs` seconds.
pub fn kick_train(bpm: f32, secs: f32, sr: u32) -> Vec<f32> {
    let mut out = vec![0.0; (secs * sr as f32) as usize];
    let hit = kick(sr);
    let beat = 60.0 / bpm * sr as f32;
    let mut b = 0;
    loop {
        let at = (b as f32 * beat).round() as usize;
        if at >= out.len() {
            break;
        }
        add(&mut out, at, &hit, 1.0);
        b += 1;
    }
    out
}

/// Kicks with a bass note between every two of them. `bass_gain` 1.0 makes
/// the bass hit as hard as the kick, which is the hard case for tempo.
pub fn kick_and_bass(bpm: f32, secs: f32, sr: u32, bass_gain: f32) -> Vec<f32> {
    let mut out = kick_train(bpm, secs, sr);
    let note = bass_note(55.0, 0.12, sr);
    let beat = 60.0 / bpm * sr as f32;
    let mut b = 0.5;
    while ((b * beat) as usize) < out.len() {
        add(&mut out, (b * beat).round() as usize, &note, bass_gain);
        b += 1.0;
    }
    out
}

/// A bass note on every off-beat and no kick: what often comes back first
/// at the end of a breakdown.
pub fn bass_line(bpm: f32, secs: f32, sr: u32) -> Vec<f32> {
    let mut out = vec![0.0; (secs * sr as f32) as usize];
    let note = bass_note(55.0, 0.2, sr);
    let beat = 60.0 / bpm * sr as f32;
    let mut b = 0.5;
    while ((b * beat) as usize) < out.len() {
        add(&mut out, (b * beat).round() as usize, &note, 0.5);
        b += 1.0;
    }
    out
}

/// A 30 second, 128 BPM track with a known structure:
/// 16 beats groove, 8 beats breakdown, 8 beats build, then 32 beats drop.
pub fn test_track(sr: u32) -> Vec<f32> {
    const BPM: f32 = 128.0;
    const BEATS: usize = 64;
    let beat = 60.0 / BPM * sr as f32;
    let at = |beats: f32| (beats * beat).round() as usize;
    let mut out = vec![0.0; at(BEATS as f32)];

    let kick = kick(sr);
    let bass = bass_note(55.0, 0.2, sr);
    let hat = hat(sr, 7);
    let snare = snare(sr, 11);

    for b in 0..BEATS {
        let t = b as f32;
        match b {
            // Groove and drop: kick on the beat, bass and hat on the off-beat.
            0..16 | 32.. => {
                add(&mut out, at(t), &kick, 0.9);
                add(&mut out, at(t + 0.5), &bass, 0.5);
                add(&mut out, at(t + 0.5), &hat, 0.25);
                if b >= 32 {
                    add(&mut out, at(t + 0.25), &hat, 0.15);
                    add(&mut out, at(t + 0.75), &hat, 0.15);
                }
            }
            // Breakdown: only off-beat hats over the pad.
            16..24 => add(&mut out, at(t + 0.5), &hat, 0.15),
            // Build: a snare roll that doubles in speed twice and gets louder.
            _ => {
                let hits = match b {
                    24..28 => 1,
                    28..30 => 2,
                    _ => 4,
                };
                let gain = 0.3 + 0.3 * (t - 24.0) / 8.0;
                for h in 0..hits {
                    add(&mut out, at(t + h as f32 / hits as f32), &snare, gain);
                }
            }
        }
    }

    // Pad through breakdown and build, noise riser through the build.
    let mut rng = Rng(3);
    let mut prev = 0.0;
    // The index is also the clock here, so a plain range reads best.
    #[allow(clippy::needless_range_loop)]
    for i in at(16.0)..at(32.0) {
        let secs = i as f32 / sr as f32;
        let swell = 0.75 + 0.25 * (TAU * 0.5 * secs).sin();
        let pad: f32 = [220.0, 277.18, 329.63]
            .iter()
            .map(|f| (TAU * f * secs).sin())
            .sum();
        out[i] += 0.12 * swell * pad;

        if i >= at(24.0) {
            let rise = (i - at(24.0)) as f32 / (at(32.0) - at(24.0)) as f32;
            let white = rng.next();
            out[i] += 0.3 * rise * rise * (white - prev); // high-passed
            prev = white;
        }
    }

    for v in &mut out {
        *v = v.tanh();
    }
    out
}

/// A loop in one of four styles, for testing the vibe meter: dark or
/// bright (where the sound sits above the kick), calm or intense (how busy
/// it is and how fast).
///
/// * dark: rolling sub bass, a low pad, a muffled rumble, hardly any highs.
/// * bright: high chords, open hats, a little bass.
/// * calm: kick, off-beat bass or hat only; intense: sixteenth hats and
///   claps on two and four, and faster.
pub fn styled_track(sr: u32, secs: f32, dark: bool, intense: bool) -> Vec<f32> {
    let bpm = if intense { 140.0 } else { 122.0 };
    let beat = 60.0 / bpm * sr as f32;
    let mut out = kick_train(bpm, secs, sr);
    let at = |beats: f32| (beats * beat).round() as usize;
    let beats = (secs * sr as f32 / beat) as usize;
    let sub = bass_note(45.0, 0.1, sr);
    let open_hat = burst(0.12, 0.05, sr, 23);
    let closed_hat = hat(sr, 29);
    let clap = snare(sr, 31);
    for b in 0..beats {
        let t = b as f32;
        if dark {
            // Sub bass rolling under the kick.
            let steps: &[f32] = if intense { &[0.25, 0.5, 0.75] } else { &[0.5] };
            for s in steps {
                add(&mut out, at(t + s), &sub, 0.6);
            }
            if intense {
                for s in [0.25, 0.5, 0.75] {
                    add(&mut out, at(t + s), &closed_hat, 0.07);
                }
            }
        } else {
            add(&mut out, at(t + 0.5), &open_hat, 0.45);
            add(&mut out, at(t + 0.5), &sub, 0.25);
            if intense {
                for s in [0.25, 0.75] {
                    add(&mut out, at(t + s), &closed_hat, 0.3);
                }
            }
        }
        if intense && b % 2 == 1 {
            add(&mut out, at(t), &clap, if dark { 0.12 } else { 0.4 });
        }
    }
    // The sustained part: a low pad and rumble, or high chords.
    let mut rng = Rng(41);
    // Three gentle low-pass stages: a rumble that stays down low.
    let mut low = [0.0f32; 3];
    for (i, v) in out.iter_mut().enumerate() {
        let t = i as f32 / sr as f32;
        if dark {
            let pad: f32 = [110.0, 130.8, 164.8]
                .iter()
                .map(|f| (TAU * f * t).sin())
                .sum();
            low[0] += 0.02 * (rng.next() - low[0]);
            low[1] += 0.02 * (low[0] - low[1]);
            low[2] += 0.02 * (low[1] - low[2]);
            *v += 0.1 * pad + 4.0 * low[2];
        } else {
            // Bright chords: each note with a few harmonics, like a saw.
            let chord: f32 = [659.3, 830.6, 987.8, 1318.5]
                .iter()
                .map(|f| {
                    (1..5)
                        .map(|h| (TAU * f * h as f32 * t).sin() / h as f32)
                        .sum::<f32>()
                })
                .sum();
            *v += 0.06 * chord;
        }
    }
    for v in &mut out {
        *v = v.tanh();
    }
    out
}

/// One kick: a sine that sweeps from 150 Hz down to 45 Hz and dies in 180 ms.
fn kick(sr: u32) -> Vec<f32> {
    let n = (0.18 * sr as f32) as usize;
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let freq = 45.0 + 105.0 * (-t / 0.03).exp();
            phase += TAU * freq / sr as f32;
            phase.sin() * (-t / 0.06).exp()
        })
        .collect()
}

fn bass_note(freq: f32, secs: f32, sr: u32) -> Vec<f32> {
    let n = (secs * sr as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let attack = (t / 0.005).min(1.0);
            let release = ((secs - t) / 0.03).clamp(0.0, 1.0);
            attack * release * (TAU * freq * t).sin()
        })
        .collect()
}

fn hat(sr: u32, seed: u32) -> Vec<f32> {
    burst(0.04, 0.010, sr, seed)
}

fn snare(sr: u32, seed: u32) -> Vec<f32> {
    burst(0.12, 0.035, sr, seed)
}

/// High-passed noise with an exponential decay.
fn burst(secs: f32, decay: f32, sr: u32, seed: u32) -> Vec<f32> {
    let n = (secs * sr as f32) as usize;
    let mut rng = Rng(seed);
    let mut prev = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let white = rng.next();
            let high = white - prev;
            prev = white;
            high * (-t / decay).exp()
        })
        .collect()
}

fn add(dst: &mut [f32], at: usize, src: &[f32], gain: f32) {
    for (d, s) in dst.iter_mut().skip(at).zip(src) {
        *d += gain * s;
    }
}

/// xorshift32: tiny, deterministic, good enough for noise.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample indices where the rectified signal first rises above `thresh`,
    /// ignoring re-triggers for `gap` samples.
    fn onsets(x: &[f32], thresh: f32, gap: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut next_allowed = 0;
        for (i, v) in x.iter().enumerate() {
            if i >= next_allowed && v.abs() > thresh {
                out.push(i);
                next_allowed = i + gap;
            }
        }
        out
    }

    #[test]
    fn kick_train_has_expected_kick_count() {
        let x = kick_train(120.0, 4.0, 48000);
        assert_eq!(x.len(), 4 * 48000);
        let hits = onsets(&x, 0.5, 12000);
        assert_eq!(hits.len(), 8, "onsets at {hits:?}");
        for pair in hits.windows(2) {
            let spacing = pair[1] as i64 - pair[0] as i64;
            assert!((spacing - 24000).abs() <= 50, "spacing {spacing}");
        }
    }

    #[test]
    fn test_track_is_30_seconds_and_not_clipped() {
        let x = test_track(48000);
        assert_eq!(x.len(), 30 * 48000);
        let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.5 && peak <= 1.0, "peak {peak}");
    }

    #[test]
    fn test_track_breakdown_has_no_low_end() {
        // Beats 16..24 are the breakdown: 7.5 s .. 11.25 s at 128 BPM.
        let sr = 48000;
        let x = test_track(sr);
        let low = |from: f32, to: f32| -> f32 {
            // crude low-pass: mean of |running average over 5 ms|
            let seg = &x[(from * sr as f32) as usize..(to * sr as f32) as usize];
            let win = 240;
            let mut acc = 0.0;
            let mut n = 0;
            for chunk in seg.chunks(win) {
                acc += (chunk.iter().sum::<f32>() / chunk.len() as f32).abs();
                n += 1;
            }
            acc / n as f32
        };
        let groove = low(1.0, 7.0);
        let breakdown = low(8.0, 11.0);
        assert!(
            breakdown < groove * 0.25,
            "groove {groove} breakdown {breakdown}"
        );
    }
}
