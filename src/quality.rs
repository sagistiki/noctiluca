//! Keeps the show at 60 frames a second when the computer is busy (three
//! surfaces of a heavy look, Serato beside it): if frames keep coming late,
//! the pictures are drawn smaller (1080 → 900 → 720 pixels tall), and when
//! things have been easy for a good while, one step bigger again. Never
//! bigger than the quality chosen in the settings.

/// Frames longer than this are late (at 60 a second a frame is 16.7 ms).
const LATE_MS: f32 = 18.5;
/// Frames shorter than this leave room to grow.
const EASY_MS: f32 = 17.2;
/// Late for this long: one step down.
const LATE_FOR_SECS: f32 = 2.0;
/// Easy for this long: one step up.
const EASY_FOR_SECS: f32 = 30.0;
const STEPS: [u32; 3] = [1080, 900, 720];

pub struct Governor {
    /// The tallest allowed (from the settings).
    ceiling: u32,
    height: u32,
    late_for: f32,
    easy_for: f32,
}

impl Governor {
    pub fn new(ceiling: u32) -> Self {
        Self {
            ceiling,
            height: ceiling,
            late_for: 0.0,
            easy_for: 0.0,
        }
    }

    /// How tall pictures are drawn now.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// A new ceiling from the settings; starts again from it.
    pub fn set_ceiling(&mut self, ceiling: u32) {
        if ceiling != self.ceiling {
            *self = Self::new(ceiling);
        }
    }

    /// One frame took `frame_ms`; returns the new height when it changes.
    pub fn frame(&mut self, frame_ms: f32) -> Option<u32> {
        if !frame_ms.is_finite() || frame_ms <= 0.0 || frame_ms > 500.0 {
            return None; // a stall (a dragged window, a sleeping laptop), not load
        }
        let secs = frame_ms / 1000.0;
        if frame_ms > LATE_MS {
            self.late_for += secs;
            self.easy_for = 0.0;
        } else {
            self.late_for = (self.late_for - secs).max(0.0);
            self.easy_for = if frame_ms < EASY_MS {
                self.easy_for + secs
            } else {
                0.0
            };
        }
        let lower = STEPS.iter().copied().find(|h| *h < self.height);
        let higher = STEPS
            .iter()
            .rev()
            .copied()
            .find(|h| *h > self.height && *h <= self.ceiling);
        if self.late_for >= LATE_FOR_SECS
            && let Some(h) = lower
        {
            self.height = h;
            self.late_for = 0.0;
            self.easy_for = 0.0;
            return Some(h);
        }
        if self.easy_for >= EASY_FOR_SECS
            && let Some(h) = higher
        {
            self.height = h;
            self.easy_for = 0.0;
            return Some(h);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Governor, ms: f32, secs: f32) -> Vec<u32> {
        (0..(secs * 1000.0 / ms) as usize)
            .filter_map(|_| g.frame(ms))
            .collect()
    }

    #[test]
    fn slow_frames_lower_the_quality() {
        let mut g = Governor::new(1080);
        assert_eq!(run(&mut g, 25.0, 2.5), vec![900]);
        assert_eq!(g.height(), 900);
    }

    #[test]
    fn never_below_720() {
        let mut g = Governor::new(1080);
        run(&mut g, 40.0, 60.0);
        assert_eq!(g.height(), 720);
    }

    #[test]
    fn recovers_slowly() {
        let mut g = Governor::new(1080);
        run(&mut g, 25.0, 2.5);
        assert!(run(&mut g, 16.7, 20.0).is_empty(), "back up too soon");
        assert_eq!(run(&mut g, 16.7, 12.0), vec![1080]);
    }

    #[test]
    fn never_above_the_setting() {
        let mut g = Governor::new(900);
        run(&mut g, 16.0, 120.0);
        assert_eq!(g.height(), 900);
    }

    #[test]
    fn one_stall_is_not_load() {
        let mut g = Governor::new(1080);
        assert_eq!(g.frame(2000.0), None);
        for _ in 0..10 {
            g.frame(16.7);
        }
        assert_eq!(g.height(), 1080);
    }
}
