//! What the set has done lately, for the Live page's timeline: the section
//! (groove, build, drop…), the theme and the lights' effect, once a second
//! over the last 20 minutes.

use std::collections::VecDeque;

/// Seconds kept.
pub const WINDOW_SECS: usize = 20 * 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    /// 0 silence, 1 groove, 2 build, 3 drop, 4 breakdown.
    pub section: u8,
    pub theme: u8,
    pub effect: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Section,
    Theme,
    Effect,
}

/// A stretch of one value: from and to as parts of the window (now is 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub from: f32,
    pub to: f32,
    pub value: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Timeline {
    samples: VecDeque<Sample>,
    acc: f32,
}

impl Timeline {
    /// Called every frame; keeps a sample a second.
    pub fn record(&mut self, dt: f32, sample: Sample) {
        if !dt.is_finite() {
            return;
        }
        self.acc += dt;
        if self.samples.is_empty() {
            self.samples.push_back(sample);
            self.acc = 0.0;
        }
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.samples.push_back(sample);
            if self.samples.len() > WINDOW_SECS {
                self.samples.pop_front();
            }
        }
    }

    /// Seconds recorded (at most the window).
    pub fn secs(&self) -> usize {
        self.samples.len()
    }

    /// The lane as stretches of one value, oldest first, placed in the
    /// window so that now is at its right end.
    pub fn runs(&self, lane: Lane) -> Vec<Run> {
        let n = self.samples.len();
        let start = WINDOW_SECS.saturating_sub(n);
        let at = |i: usize| (start + i) as f32 / WINDOW_SECS as f32;
        let value = |s: &Sample| match lane {
            Lane::Section => s.section,
            Lane::Theme => s.theme,
            Lane::Effect => s.effect,
        };
        let mut out: Vec<Run> = Vec::new();
        for (i, s) in self.samples.iter().enumerate() {
            let v = value(s);
            match out.last_mut() {
                Some(r) if r.value == v => r.to = at(i + 1),
                _ => out.push(Run { from: at(i), to: at(i + 1), value: v }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sample_a_second_in_runs_ending_now() {
        let mut t = Timeline::default();
        let s = |section| Sample { section, theme: 0, effect: 0 };
        for _ in 0..600 {
            t.record(0.1, s(1)); // a minute of groove
        }
        for _ in 0..300 {
            t.record(0.1, s(3)); // half a minute of drop
        }
        let runs = t.runs(Lane::Section);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].value, 1);
        assert!((runs[1].to - 1.0).abs() < 1e-4, "now is at the right end");
        let drop_len = (runs[1].to - runs[1].from) * WINDOW_SECS as f32;
        assert!((drop_len - 30.0).abs() <= 1.5, "{drop_len}");
        assert_eq!(t.runs(Lane::Theme).len(), 1);
    }

    #[test]
    fn keeps_twenty_minutes_and_ignores_broken_time() {
        let mut t = Timeline::default();
        for i in 0..(WINDOW_SECS + 500) {
            t.record(1.0, Sample { section: (i / 100 % 5) as u8, theme: 0, effect: 0 });
        }
        assert_eq!(t.secs(), WINDOW_SECS);
        assert!(t.runs(Lane::Section).first().unwrap().from.abs() < 1e-6);
        t.record(f32::NAN, Sample::default());
        assert_eq!(t.secs(), WINDOW_SECS);
    }
}
