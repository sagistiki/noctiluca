//! When lightning strikes in the storm atmosphere.
//!
//! On the fourth beat of every second bar, now and then on another beat,
//! and always on a drop. The light a strike throws over the clouds goes
//! through its own flash limiter, so lightning can never flash faster than
//! is safe, and the strobe switch (S) turns it off with the rest.

use crate::analysis::Features;
use crate::strobe::StrobeLimiter;

/// How long a strike's light takes to die away.
const FADE_SECS: f32 = 0.18;
/// The brightest a strike may light the clouds (the limiter's cap).
const MAX_LIGHT: f32 = 0.8;
/// Share of other beats that strike too, at random.
const EXTRA_CHANCE: f32 = 0.08;

pub struct Lightning {
    last_beat: Option<u64>,
    brightness: f32,
    seed: f32,
    strikes: u32,
    limiter: StrobeLimiter,
}

impl Default for Lightning {
    fn default() -> Self {
        Self::new()
    }
}

impl Lightning {
    pub fn new() -> Self {
        Self {
            last_beat: None,
            brightness: 0.0,
            seed: 0.0,
            strikes: 0,
            limiter: StrobeLimiter::new(3.0, MAX_LIGHT),
        }
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.limiter.set_enabled(on);
    }

    /// Returns how bright the current strike is (0..1, already limited) and
    /// a number that picks the bolt's shape, new for every strike.
    /// `active` says whether a storm is on screen at all.
    pub fn update(&mut self, f: &Features, active: bool, dt: f32) -> (f32, f32) {
        let new_beat = self.last_beat != Some(f.beat_count);
        self.last_beat = Some(f.beat_count);
        let planned = new_beat && f.beat_count % 8 == 3;
        let chance = new_beat && random(f.beat_count as f32) < EXTRA_CHANCE;
        if active && (f.drop || planned || chance) {
            self.brightness = 1.0;
            self.strikes += 1;
            self.seed = random(self.strikes as f32 + 0.5) * 1000.0;
        }
        self.brightness *= (-dt / FADE_SECS).exp();
        if self.brightness < 0.01 {
            self.brightness = 0.0;
        }
        (self.limiter.update(self.brightness, dt), self.seed)
    }
}

/// A repeatable random number in 0..1.
fn random(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43758.547).fract().abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// Plays `beats` beats at `bpm`; returns the beats on which a strike
    /// began, and each strike's shape number.
    fn strikes(
        l: &mut Lightning,
        bpm: f32,
        beats: u64,
        active: bool,
        drop_at: Option<u64>,
    ) -> Vec<(u64, f32)> {
        let frames_per_beat = (60.0 / bpm / DT).round() as u64;
        let (mut out, mut lit) = (Vec::new(), false);
        for beat in 0..beats {
            for i in 0..frames_per_beat {
                let f = Features {
                    beat_count: beat,
                    bpm,
                    drop: drop_at == Some(beat) && i == 0,
                    ..Default::default()
                };
                let (light, seed) = l.update(&f, active, DT);
                if light > 0.05 && !lit {
                    out.push((beat, seed));
                }
                lit = light > 0.05;
            }
        }
        out
    }

    #[test]
    fn strikes_on_the_fourth_beat_of_every_second_bar() {
        let mut l = Lightning::new();
        let found = strikes(&mut l, 128.0, 64, true, None);
        let beats: Vec<u64> = found.iter().map(|(b, _)| *b).collect();
        for planned in (0..64).filter(|b| b % 8 == 3) {
            assert!(
                beats.contains(&planned),
                "no strike on beat {planned}: {beats:?}"
            );
        }
        assert!(beats.len() <= 16, "too many strikes: {beats:?}");
    }

    #[test]
    fn a_drop_always_strikes() {
        let mut l = Lightning::new();
        let found = strikes(&mut l, 128.0, 16, true, Some(5));
        assert!(found.iter().any(|(b, _)| *b == 5), "{found:?}");
    }

    #[test]
    fn no_storm_no_lightning() {
        let mut l = Lightning::new();
        assert!(strikes(&mut l, 128.0, 64, false, Some(5)).is_empty());
    }

    #[test]
    fn switched_off_with_the_strobe() {
        let mut l = Lightning::new();
        l.set_enabled(false);
        assert!(strikes(&mut l, 128.0, 64, true, Some(5)).is_empty());
    }

    #[test]
    fn never_more_than_three_strikes_a_second() {
        // An absurd tempo with a drop on every beat still flashes safely.
        let mut l = Lightning::new();
        let frames_per_beat = 6; // 600 BPM at 60 fps
        let mut starts = Vec::new();
        let mut lit = false;
        for beat in 0..200u64 {
            for i in 0..frames_per_beat {
                let f = Features {
                    beat_count: beat,
                    bpm: 600.0,
                    drop: i == 0,
                    ..Default::default()
                };
                let (light, _) = l.update(&f, true, DT);
                if light > 0.05 && !lit {
                    starts.push((beat * frames_per_beat + i) as f32 * DT);
                }
                lit = light > 0.05;
                assert!(light <= 0.8, "brighter than the cap: {light}");
            }
        }
        for (i, t) in starts.iter().enumerate() {
            let in_window = starts[i..].iter().take_while(|s| **s < t + 1.0).count();
            assert!(
                in_window <= 3,
                "{in_window} strikes in the second after {t}"
            );
        }
    }

    #[test]
    fn every_strike_has_its_own_shape() {
        let mut l = Lightning::new();
        let found = strikes(&mut l, 128.0, 64, true, None);
        let mut seeds: Vec<i64> = found.iter().map(|(_, s)| (s * 1000.0) as i64).collect();
        seeds.sort();
        seeds.dedup();
        assert_eq!(seeds.len(), found.len(), "two strikes share a shape");
    }
}
