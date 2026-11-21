//! How far apart the crystal's shards are: 0 is the whole crystal, 1 is
//! blown apart. A drop blows it apart in a fraction of a second; through
//! the drop the shards drift back to a loose cluster, in the groove almost
//! whole; a
//! build-up or a breakdown gathers them into the whole crystal again.

use crate::analysis::{Features, Mood};

/// How long the explosion takes to reach full spread.
const BURST_SECS: f32 = 0.25;
/// How long the shards take to come back together.
const GATHER_SECS: f32 = 3.0;
/// The spread held after a drop has blown it apart.
const AFTER_DROP_SECS: f32 = 2.0;
/// How long the camera shakes after a drop (it dies away over about this long).
const SHAKE_SECS: f32 = 0.6;

#[derive(Default)]
pub struct Burst {
    spread: f32,
    /// Time left at full spread after a drop.
    hold: f32,
    /// How hard the camera shakes, 1 at a drop, dying away.
    shake: f32,
}

impl Burst {
    pub fn new() -> Self {
        Self::default()
    }

    /// The spread, and how hard the camera shakes.
    pub fn update(&mut self, f: &Features, dt: f32) -> (f32, f32) {
        if f.drop {
            self.hold = AFTER_DROP_SECS;
            self.shake = 1.0;
        }
        self.shake *= (-dt / (SHAKE_SECS / 3.0)).exp();
        if !self.shake.is_finite() || self.shake < 0.01 {
            self.shake = 0.0;
        }
        self.hold = (self.hold - dt).max(0.0);
        let target = if self.hold > 0.0 {
            1.0
        } else {
            match f.mood {
                Mood::Drop => 0.3,
                Mood::Groove => 0.08,
                Mood::Build | Mood::Breakdown | Mood::Silence => 0.0,
            }
        };
        let secs = if target > self.spread {
            BURST_SECS
        } else {
            GATHER_SECS
        };
        // Eases toward the target: quick at first, then settling.
        self.spread += (target - self.spread) * (1.0 - (-dt / (secs / 3.0)).exp());
        if !self.spread.is_finite() {
            self.spread = 0.0;
        }
        (self.spread.clamp(0.0, 1.0), self.shake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(b: &mut Burst, mood: Mood, drop_first: bool, secs: f32) -> f32 {
        let mut out = 0.0;
        let steps = (secs * 60.0) as usize;
        for i in 0..steps {
            let f = Features {
                mood,
                drop: drop_first && i == 0,
                ..Features::default()
            };
            out = b.update(&f, 1.0 / 60.0).0;
        }
        out
    }

    #[test]
    fn a_drop_blows_it_apart_fast() {
        let mut b = Burst::new();
        assert!(run(&mut b, Mood::Build, false, 5.0) < 0.01);
        assert!(run(&mut b, Mood::Drop, true, 0.4) > 0.9);
    }

    #[test]
    fn a_breakdown_gathers_it_again() {
        let mut b = Burst::new();
        run(&mut b, Mood::Drop, true, 3.0);
        assert!(run(&mut b, Mood::Breakdown, false, 4.0) < 0.05);
    }

    #[test]
    fn a_drop_shakes_the_camera_briefly() {
        let mut b = Burst::new();
        let f = Features {
            mood: Mood::Drop,
            drop: true,
            ..Features::default()
        };
        assert_eq!(b.update(&f, 0.0).1, 1.0);
        let f = Features {
            mood: Mood::Drop,
            ..Features::default()
        };
        let mut shake = 1.0;
        for _ in 0..60 {
            shake = b.update(&f, 1.0 / 60.0).1;
        }
        assert_eq!(shake, 0.0, "still shaking a second later");
    }

    #[test]
    fn the_groove_keeps_it_loose() {
        let mut b = Burst::new();
        let spread = run(&mut b, Mood::Groove, false, 10.0);
        assert!((0.05..0.12).contains(&spread), "{spread}");
    }
}
