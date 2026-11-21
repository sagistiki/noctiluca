//! noctiluca: a live club visualizer.
//!
//! The crate is a library plus a thin binary so that tests and examples can
//! reach every module.

pub mod analysis;
pub mod audio;
pub mod director;
#[cfg(unix)]
pub mod dmx;
pub mod render;
pub mod strobe;
pub mod synth;

#[cfg(test)]
mod tests {
    use crate::analysis::Analyzer;
    use crate::director::Director;
    use crate::strobe::StrobeLimiter;
    use crate::synth::test_track;

    /// The whole chain as the app runs it: music in, flash brightness out.
    /// Returns the times at which the screen got brighter.
    fn flashes_on_test_track(fps: usize, seed: u64) -> Vec<f32> {
        let sr = 48000;
        let track = test_track(sr);
        let (mut analyzer, mut director) = (Analyzer::new(sr), Director::new(seed));
        let mut limiter = StrobeLimiter::new(3.0, 0.6);
        let dt = 1.0 / fps as f32;

        let mut times = Vec::new();
        let (mut prev, mut low, mut rising) = (0.0f32, 0.0f32, false);
        // Twice through: two drops, the second with the tempo already locked.
        for (i, chunk) in track
            .iter()
            .chain(&track)
            .copied()
            .collect::<Vec<_>>()
            .chunks(sr as usize / fps)
            .enumerate()
        {
            analyzer.push(chunk);
            let show = director.update(&analyzer.features(), dt);
            let out = limiter.update(show.flash, dt);
            assert!(out <= 0.6, "brighter than the cap: {out}");
            if out < prev {
                rising = false;
                low = out;
            } else if out > low + 0.05 && !rising {
                rising = true;
                times.push(i as f32 * dt);
            }
            low = low.min(out);
            prev = out;
        }
        times
    }

    #[test]
    fn no_more_than_three_flashes_in_any_second_of_a_real_run() {
        for fps in [60, 120] {
            for seed in [1, 2, 3] {
                let times = flashes_on_test_track(fps, seed);
                assert!(times.len() >= 2, "the drops should flash at all: {times:?}");
                for (i, t) in times.iter().enumerate() {
                    let in_window = times[i..].iter().take_while(|s| **s < t + 1.0).count();
                    assert!(
                        in_window <= 3,
                        "{fps} fps, seed {seed}: {in_window} flashes after {t}: {times:?}"
                    );
                }
            }
        }
    }
}
pub mod app_state;
pub mod burst;
pub mod lightning;
pub mod lights;
pub mod mapping;
pub mod net;
pub mod output;
pub mod quality;
pub mod settings;
pub mod stage;
pub mod sync;
pub mod timeline;
pub mod ui;
