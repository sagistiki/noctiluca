//! Keeps flashing within safe limits.
//!
//! Rapid full-screen flashes can trigger seizures in people with
//! photosensitive epilepsy. The widely used guideline (WCAG 2.3.1) is no more
//! than three flashes in any one second, so every flash the visuals ask for
//! goes through this limiter before it reaches the screen.

/// Below this a flash counts as dark.
const DARK: f32 = 0.05;
/// Leave a little more room than the limit strictly needs.
const SAFETY_MARGIN: f32 = 1.05;
/// A flash that is still being asked for after this long is faded out anyway.
const MAX_FLASH_SECS: f32 = 0.4;
const FORCED_FADE_SECS: f32 = 0.1;

pub struct StrobeLimiter {
    min_gap_secs: f32,
    max_intensity: f32,
    enabled: bool,
    since_flash_start: f32,
    /// A flash is on screen right now.
    lit: bool,
    /// The current request is being dropped: it came too soon, or its flash
    /// has already run its course. Cleared when the request goes dark.
    blocked: bool,
    /// The brightest the current flash may still be. It only ever comes
    /// down, so one flash can never swell back up into a second one.
    ceiling: f32,
}

impl StrobeLimiter {
    pub fn new(max_hz: f32, max_intensity: f32) -> Self {
        Self {
            // A rate that is not a positive number means "never".
            min_gap_secs: if max_hz > 0.0 {
                SAFETY_MARGIN / max_hz
            } else {
                f32::INFINITY
            },
            // NaN would slip through `clamp` and then through `min`, removing the cap.
            max_intensity: if max_intensity.is_nan() {
                0.0
            } else {
                max_intensity.clamp(0.0, 1.0)
            },
            enabled: true,
            since_flash_start: f32::MAX,
            lit: false,
            blocked: false,
            ceiling: 0.0,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Changes the brightest a flash may be (0..1; NaN means none).
    pub fn set_max_intensity(&mut self, max: f32) {
        self.max_intensity = if max.is_nan() {
            0.0
        } else {
            max.clamp(0.0, 1.0)
        };
    }

    /// `want` is the brightness the visuals ask for (0..1); returns what is allowed.
    ///
    /// A flash, to the eye, is any time the screen gets brighter. So besides
    /// spacing flashes out, the limiter makes each one strictly fade: once a
    /// flash has started, the output may fall but never rise, and it is over
    /// within about half a second whatever the request does. A new flash can
    /// only begin after the request itself has gone dark.
    pub fn update(&mut self, want: f32, dt: f32) -> f32 {
        self.since_flash_start += dt;
        // NaN compares false with everything, so it has to be asked for by
        // name; a broken value must never reach the screen.
        if !self.enabled || want.is_nan() || want <= DARK {
            self.lit = false;
            self.blocked = false;
            return 0.0;
        }
        if !self.lit && !self.blocked {
            if self.since_flash_start >= self.min_gap_secs {
                self.lit = true;
                self.since_flash_start = 0.0;
                self.ceiling = self.max_intensity;
            } else {
                self.blocked = true;
            }
        }
        if !self.lit {
            return 0.0;
        }

        let mut out = want.min(self.ceiling);
        if self.since_flash_start > MAX_FLASH_SECS {
            let over = self.since_flash_start - MAX_FLASH_SECS;
            out = out.min(self.max_intensity * (-over / FORCED_FADE_SECS).exp());
        }
        self.ceiling = out;
        if out <= DARK {
            self.lit = false;
            self.blocked = true;
            return 0.0;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    /// Runs the limiter over `secs` of a wanted signal and returns the times
    /// at which its output went from dark to lit.
    fn flashes(limiter: &mut StrobeLimiter, secs: f32, want: impl Fn(f32) -> f32) -> Vec<f32> {
        let mut starts = Vec::new();
        let mut lit = false;
        for i in 0..(secs / DT) as usize {
            let t = i as f32 * DT;
            let out = limiter.update(want(t), DT);
            if out > 0.05 && !lit {
                starts.push(t);
            }
            lit = out > 0.05;
        }
        starts
    }

    fn square(hz: f32) -> impl Fn(f32) -> f32 {
        move |t| if (t * hz).fract() < 0.5 { 1.0 } else { 0.0 }
    }

    #[test]
    fn never_more_than_three_flashes_per_second() {
        let mut limiter = StrobeLimiter::new(3.0, 1.0);
        let starts = flashes(&mut limiter, 10.0, square(20.0));
        assert!(starts.len() <= 30, "{} flashes in 10 s", starts.len());
        assert!(
            starts.len() >= 20,
            "limiter is far too strict: {}",
            starts.len()
        );
        // No one-second window may hold more than three.
        for (i, t) in starts.iter().enumerate() {
            let in_window = starts[i..].iter().take_while(|s| **s < t + 1.0).count();
            assert!(
                in_window <= 3,
                "{in_window} flashes in the second after {t}"
            );
        }
    }

    #[test]
    fn slow_flashes_pass_untouched() {
        // 128 BPM is 2.13 kicks a second: under the limit, so every one shows.
        let mut limiter = StrobeLimiter::new(3.0, 1.0);
        let starts = flashes(&mut limiter, 10.0, square(128.0 / 60.0));
        assert_eq!(starts.len(), 22, "{starts:?}");
    }

    #[test]
    fn a_blocked_flash_does_not_start_halfway() {
        // Two long flashes 0.1 s apart: the second is too soon and must be
        // dropped whole, not switched on once the waiting time is over.
        let mut limiter = StrobeLimiter::new(3.0, 1.0);
        let want = |t: f32| {
            if t < 0.1 || (0.2..1.0).contains(&t) {
                1.0
            } else {
                0.0
            }
        };
        let starts = flashes(&mut limiter, 1.2, want);
        assert_eq!(starts.len(), 1, "{starts:?}");
    }

    /// Times at which the output turned from falling to rising: every one is
    /// a flash to the eye, whether or not the screen went fully dark first.
    fn brightenings(limiter: &mut StrobeLimiter, secs: f32, want: impl Fn(f32) -> f32) -> Vec<f32> {
        let mut times = Vec::new();
        let (mut prev, mut low) = (0.0f32, 0.0f32);
        let mut rising = false;
        for i in 0..(secs / DT) as usize {
            let t = i as f32 * DT;
            let out = limiter.update(want(t), DT);
            if out < prev {
                rising = false;
                low = out;
            } else if out > low + 0.05 && !rising {
                rising = true;
                times.push(t);
            }
            low = low.min(out);
            prev = out;
        }
        times
    }

    #[test]
    fn a_request_that_never_goes_dark_cannot_pulse() {
        // 1.0 / 0.1 at 10 Hz never drops under the "dark" level. Each swing
        // back up is still a flash and must be limited like one.
        let mut limiter = StrobeLimiter::new(3.0, 0.6);
        let want = |t: f32| if (t * 10.0).fract() < 0.5 { 1.0 } else { 0.1 };
        let times = brightenings(&mut limiter, 10.0, want);
        assert!(times.len() <= 30, "{} brightenings in 10 s", times.len());
        for (i, t) in times.iter().enumerate() {
            let in_window = times[i..].iter().take_while(|s| **s < t + 1.0).count();
            assert!(
                in_window <= 3,
                "{in_window} brightenings in the second after {t}"
            );
        }
    }

    #[test]
    fn a_flash_on_top_of_a_fading_flash_is_not_let_through() {
        // A drop's flash fading out, with a kick's flash arriving 0.23 s later
        // while the first is still visible.
        let mut limiter = StrobeLimiter::new(3.0, 0.6);
        let fade = |since: f32, tau: f32| {
            if since >= 0.0 {
                (-since / tau).exp()
            } else {
                0.0
            }
        };
        let want = |t: f32| fade(t, 0.12).max(0.6 * fade(t - 0.23, 0.075));
        let times = brightenings(&mut limiter, 1.0, want);
        assert_eq!(times.len(), 1, "{times:?}");
    }

    #[test]
    fn a_stuck_request_fades_to_dark() {
        // If whatever asks for the flash freezes at "on", the screen must not
        // stay veiled in white.
        let mut limiter = StrobeLimiter::new(3.0, 0.6);
        let mut out = 0.0;
        for _ in 0..(1.5 / DT) as usize {
            out = limiter.update(0.4, DT);
        }
        assert_eq!(out, 0.0);
    }

    #[test]
    fn a_nonsense_cap_means_no_flash_at_all() {
        let mut limiter = StrobeLimiter::new(3.0, f32::NAN);
        assert_eq!(limiter.update(1.0, DT), 0.0);
        let mut limiter = StrobeLimiter::new(f32::NAN, 0.6);
        let peak = (0..600)
            .map(|_| limiter.update(1.0, DT))
            .fold(0.0, f32::max);
        assert!(peak <= 0.6);
    }

    #[test]
    fn disabled_is_always_zero() {
        let mut limiter = StrobeLimiter::new(3.0, 1.0);
        limiter.set_enabled(false);
        assert!(!limiter.enabled());
        assert!(flashes(&mut limiter, 5.0, square(2.0)).is_empty());
    }

    #[test]
    fn intensity_is_capped() {
        let mut limiter = StrobeLimiter::new(3.0, 0.4);
        let peak = (0..600)
            .map(|_| limiter.update(1.0, DT))
            .fold(0.0, f32::max);
        assert_eq!(peak, 0.4);
    }

    #[test]
    fn garbage_in_is_dark_out() {
        let mut limiter = StrobeLimiter::new(3.0, 1.0);
        assert_eq!(limiter.update(f32::NAN, DT), 0.0);
        assert_eq!(limiter.update(-5.0, DT), 0.0);
    }
}
