//! FFT, frequency bands and a log-spaced spectrum.

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

pub const BANDS: usize = 4;
pub const SPECTRUM_BINS: usize = 64;
pub const FFT_SIZE: usize = 2048;
pub const HOP: usize = 512;

/// Band edges in Hz: sub, bass, mid, high.
pub const BAND_EDGES: [(f32, f32); BANDS] = [
    (20.0, 60.0),
    (60.0, 250.0),
    (250.0, 2000.0),
    (2000.0, 12000.0),
];

/// Music carries far less energy in the highs than in the lows. These weights
/// undo the typical tilt so that every band is roughly as lively as the rest.
const BAND_TILT: [f32; BANDS] = [1.0, 1.0, 2.5, 6.0];

/// A band is scaled by its own recent peak, but never by less than this
/// fraction of the loudest band. Without the limit a band that only holds
/// leakage or hiss would be amplified up to full scale.
const MIN_REF_RATIO: f32 = 0.35;

const SPECTRUM_LOW_HZ: f32 = 30.0;
const SPECTRUM_HIGH_HZ: f32 = 16_000.0;
const SPECTRUM_RANGE_DB: f32 = 50.0;
const FLUX_MAX_HZ: f32 = 200.0;
const KICK_ZONE_HZ: f32 = 150.0;
const PEAK_HALF_LIFE_SECS: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
pub struct SpectrumFrame {
    /// Band amplitudes as measured.
    pub raw_bands: [f32; BANDS],
    /// Band amplitudes scaled to 0..1 by the recent peak, so the microphone
    /// level does not matter.
    pub bands: [f32; BANDS],
    /// Log-spaced spectrum, 30 Hz..16 kHz, 0..1 on a 50 dB scale.
    pub spectrum: [f32; SPECTRUM_BINS],
    /// How much the low end (below 200 Hz) grew since the previous frame.
    pub flux_low: f32,
    /// Loudness of this frame, 0..1 relative to the recent peak.
    pub level: f32,
    /// Loudness of this frame as measured (RMS).
    pub rms: f32,
    /// Amplitude below 150 Hz as measured: the zone the kick lives in.
    pub low: f32,
    /// Where the sound's weight sits above the kick and sub, in Hz (the
    /// spectral centroid of the amplitudes, 150 Hz..12 kHz): low for dark,
    /// muffled music, high for bright music. The kick and the sub are left
    /// out: every techno track has them, and their power would drown the rest.
    pub centroid_hz: f32,
    /// How noise-like the mids and highs are (spectral flatness, 300 Hz to
    /// 8 kHz), 0..1: near 0 for clean tones and chords, higher for noise,
    /// distortion and hats.
    pub flatness: f32,
    /// How much the highs (above 4 kHz) grew since the previous frame,
    /// relative to their level: peaks on hats, claps and snares.
    pub flux_high: f32,
}

pub struct Spectrum {
    sr: f32,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    window_sum: f32,
    /// Samples that have not filled a whole FFT window yet.
    pending: Vec<f32>,
    buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    mag: Vec<f32>,
    prev_low: Vec<f32>,
    /// The highs of the previous frame, for `flux_high`.
    prev_high: Vec<f32>,
    high_bins: (usize, usize),
    centroid_bins: (usize, usize),
    flat_bins: (usize, usize),
    kick_zone_bins: usize,
    band_bins: [(usize, usize); BANDS],
    /// Fractional FFT bin range covered by each spectrum bin.
    spectrum_ranges: Vec<(f32, f32)>,
    band_peak: [f32; BANDS],
    spectrum_peak: f32,
    level_peak: f32,
    peak_decay: f32,
}

impl Spectrum {
    pub fn new(sr: u32) -> Self {
        let sr = sr as f32;
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        // Hann window: fades each slice in and out so its edges do not smear
        // across the whole spectrum.
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT_SIZE as f32).cos())
            .collect();
        let to_bin = |hz: f32| hz * FFT_SIZE as f32 / sr;
        let nyquist = FFT_SIZE / 2;

        let band_bins = BAND_EDGES.map(|(lo, hi)| {
            let lo = (to_bin(lo).round() as usize).clamp(1, nyquist - 1);
            let hi = (to_bin(hi).round() as usize).clamp(lo + 1, nyquist);
            (lo, hi)
        });

        let octaves = (SPECTRUM_HIGH_HZ / SPECTRUM_LOW_HZ).ln();
        let edge = |i: usize| {
            to_bin(SPECTRUM_LOW_HZ * (octaves * i as f32 / SPECTRUM_BINS as f32).exp())
                .min(nyquist as f32 - 1.0)
        };
        let spectrum_ranges = (0..SPECTRUM_BINS).map(|i| (edge(i), edge(i + 1))).collect();

        let flux_bins = (to_bin(FLUX_MAX_HZ).round() as usize).clamp(2, nyquist);
        let kick_zone_bins = (to_bin(KICK_ZONE_HZ).round() as usize).clamp(2, nyquist);
        let range = |lo: f32, hi: f32| {
            let lo = (to_bin(lo).round() as usize).clamp(1, nyquist - 1);
            (lo, (to_bin(hi).round() as usize).clamp(lo + 1, nyquist))
        };
        let high_bins = range(4000.0, 12_000.0);

        Self {
            sr,
            fft,
            window_sum: window.iter().sum(),
            window,
            pending: Vec::with_capacity(FFT_SIZE * 2),
            buf: vec![Complex::default(); FFT_SIZE],
            scratch,
            mag: vec![0.0; nyquist],
            prev_low: vec![0.0; flux_bins],
            prev_high: vec![0.0; high_bins.1 - high_bins.0],
            high_bins,
            centroid_bins: range(150.0, 12_000.0),
            flat_bins: range(300.0, 8000.0),
            kick_zone_bins,
            band_bins,
            spectrum_ranges,
            band_peak: [0.0; BANDS],
            spectrum_peak: 0.0,
            level_peak: 0.0,
            peak_decay: 0.5f32.powf(HOP as f32 / sr / PEAK_HALF_LIFE_SECS),
        }
    }

    /// Frames produced per second.
    pub fn hop_rate(&self) -> f32 {
        self.sr / HOP as f32
    }

    /// Feeds samples in; calls `on_frame` once for every completed hop.
    pub fn push(&mut self, mono: &[f32], mut on_frame: impl FnMut(&SpectrumFrame)) {
        self.pending.extend_from_slice(mono);
        let mut start = 0;
        while self.pending.len() - start >= FFT_SIZE {
            let frame = self.analyse(start);
            on_frame(&frame);
            start += HOP;
        }
        self.pending.drain(..start);
    }

    fn analyse(&mut self, start: usize) -> SpectrumFrame {
        let slice = &self.pending[start..start + FFT_SIZE];
        let rms = (slice.iter().map(|s| s * s).sum::<f32>() / FFT_SIZE as f32).sqrt();

        for ((out, s), w) in self.buf.iter_mut().zip(slice).zip(&self.window) {
            *out = Complex::new(s * w, 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buf, &mut self.scratch);
        // Scaled so that a full-scale sine reads about 1.0.
        let scale = 2.0 / self.window_sum;
        for (m, c) in self.mag.iter_mut().zip(&self.buf) {
            *m = c.norm() * scale;
        }

        let mut flux_low = 0.0;
        for (prev, now) in self.prev_low.iter_mut().zip(&self.mag).skip(1) {
            flux_low += (now - *prev).max(0.0);
            *prev = *now;
        }

        let low = self.mag[1..self.kick_zone_bins]
            .iter()
            .map(|m| m * m)
            .sum::<f32>()
            .sqrt();

        let mut raw_bands = [0.0; BANDS];
        let mut tilted = [0.0; BANDS];
        for b in 0..BANDS {
            let (lo, hi) = self.band_bins[b];
            raw_bands[b] = self.mag[lo..hi].iter().map(|m| m * m).sum::<f32>().sqrt();
            tilted[b] = raw_bands[b] * BAND_TILT[b];
            self.band_peak[b] = (self.band_peak[b] * self.peak_decay).max(tilted[b]);
        }
        let loudest = self.band_peak.iter().copied().fold(0.0, f32::max);
        let mut bands = [0.0; BANDS];
        for b in 0..BANDS {
            let reference = self.band_peak[b].max(loudest * MIN_REF_RATIO).max(1e-3);
            bands[b] = (tilted[b] / reference).min(1.0);
        }

        let mut spectrum = [0.0; SPECTRUM_BINS];
        let mut frame_peak = 0.0f32;
        for (out, &(lo, hi)) in spectrum.iter_mut().zip(&self.spectrum_ranges) {
            let centre_hz = (lo + hi) * 0.5 * self.sr / FFT_SIZE as f32;
            // +3 dB per octave above 200 Hz, so the highs are visible at all.
            let tilt = (centre_hz / 200.0).sqrt().clamp(1.0, 8.0);
            *out = self.magnitude_between(lo, hi) * tilt;
            frame_peak = frame_peak.max(*out);
        }
        self.spectrum_peak = (self.spectrum_peak * self.peak_decay).max(frame_peak);
        let reference = self.spectrum_peak.max(1e-3);
        for v in &mut spectrum {
            let db = 20.0 * (v.max(1e-9) / reference).log10();
            *v = (1.0 + db / SPECTRUM_RANGE_DB).clamp(0.0, 1.0);
        }

        self.level_peak = (self.level_peak * self.peak_decay).max(rms);
        let level = (rms / self.level_peak.max(5e-3)).min(1.0);

        // The standard spectral centroid: the amplitude-weighted average
        // frequency. (Measured per octave instead, it hardly told real tracks
        // apart; this way a change of track shows clearly.)
        let (lo, hi) = self.centroid_bins;
        let (mut weighted, mut total) = (0.0f64, 0.0f64);
        for (i, m) in self.mag[lo..hi].iter().enumerate() {
            weighted += *m as f64 * (lo + i) as f64;
            total += *m as f64;
        }
        let hz_per_bin = self.sr / FFT_SIZE as f32;
        let centroid_hz = if total > 1e-9 {
            (weighted / total) as f32 * hz_per_bin
        } else {
            0.0
        };

        // Flatness: geometric mean over arithmetic mean of the power.
        let (lo, hi) = self.flat_bins;
        let (mut log_sum, mut sum) = (0.0f64, 0.0f64);
        for m in &self.mag[lo..hi] {
            let p = (*m as f64) * (*m as f64) + 1e-14;
            log_sum += p.ln();
            sum += p;
        }
        let n = (hi - lo) as f64;
        let flatness = if sum > 1e-10 * n {
            ((log_sum / n).exp() / (sum / n)) as f32
        } else {
            0.0
        };

        // Rise of the highs, as a share of how loud they are now, so a quiet
        // room and a loud one give the same peaks.
        let (lo, hi) = self.high_bins;
        let (mut rise, mut energy) = (0.0f32, 0.0f32);
        for (prev, now) in self.prev_high.iter_mut().zip(&self.mag[lo..hi]) {
            rise += (now - *prev).max(0.0);
            energy += now;
            *prev = *now;
        }
        let flux_high = if energy > 1e-6 { rise / energy } else { 0.0 };

        SpectrumFrame {
            raw_bands,
            bands,
            spectrum,
            flux_low,
            level,
            rms,
            low,
            centroid_hz,
            flatness,
            flux_high,
        }
    }

    /// Loudest magnitude in a fractional bin range. Ranges narrower than one
    /// FFT bin (the low end of a log axis) are interpolated instead.
    fn magnitude_between(&self, lo: f32, hi: f32) -> f32 {
        if hi - lo < 1.0 {
            let pos = (lo + hi) * 0.5;
            let i = pos as usize;
            let next = (i + 1).min(self.mag.len() - 1);
            return self.mag[i] + (self.mag[next] - self.mag[i]) * pos.fract();
        }
        let from = lo.ceil() as usize;
        let to = (hi.ceil() as usize).min(self.mag.len());
        self.mag[from..to].iter().copied().fold(0.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::sine;

    fn last_frame(sr: u32, signal: &[f32]) -> SpectrumFrame {
        let mut s = Spectrum::new(sr);
        let mut last = None;
        s.push(signal, |f| last = Some(*f));
        last.expect("at least one frame")
    }

    fn loudest(bands: [f32; BANDS]) -> usize {
        (0..BANDS)
            .max_by(|a, b| bands[*a].total_cmp(&bands[*b]))
            .unwrap()
    }

    #[test]
    fn sine_lands_in_right_band() {
        for (freq, band) in [(35.0, 0), (120.0, 1), (800.0, 2), (5000.0, 3)] {
            let f = last_frame(48000, &sine(freq, 1.0, 48000, 0.8));
            assert_eq!(loudest(f.bands), band, "{freq} Hz gave {:?}", f.bands);
            // The other bands must stay quiet, not be auto-gained up to full.
            for b in 0..BANDS {
                if b != band {
                    assert!(f.bands[b] < 0.5, "{freq} Hz leaked: {:?}", f.bands);
                }
            }
        }
    }

    #[test]
    fn auto_gain_is_level_independent() {
        let loud = last_frame(48000, &sine(100.0, 5.0, 48000, 1.0));
        let quiet = last_frame(48000, &sine(100.0, 5.0, 48000, 0.05));
        assert!(loud.bands[1] > 0.7, "loud {:?}", loud.bands);
        assert!(
            (loud.bands[1] - quiet.bands[1]).abs() < 0.15,
            "loud {} quiet {}",
            loud.bands[1],
            quiet.bands[1]
        );
    }

    #[test]
    fn silence_is_zero_not_nan() {
        let f = last_frame(48000, &vec![0.0; 2 * 48000]);
        let all = f
            .bands
            .iter()
            .chain(&f.raw_bands)
            .chain(&f.spectrum)
            .chain([&f.flux_low, &f.level, &f.rms, &f.low]);
        for v in all {
            assert!(v.is_finite() && *v < 0.01, "got {v}");
        }
    }

    #[test]
    fn spectrum_peak_sits_at_the_tone() {
        // 1 kHz on a 30 Hz..16 kHz log axis sits at 55.8 % of the way along.
        let f = last_frame(48000, &sine(1000.0, 1.0, 48000, 0.8));
        let peak = (0..SPECTRUM_BINS)
            .max_by(|a, b| f.spectrum[*a].total_cmp(&f.spectrum[*b]))
            .unwrap();
        assert!((34..=37).contains(&peak), "peak bin {peak}");
    }

    #[test]
    fn works_at_44100() {
        let f = last_frame(44100, &sine(100.0, 1.0, 44100, 0.8));
        assert_eq!(loudest(f.bands), 1);
    }

    #[test]
    fn low_flux_fires_on_a_bass_hit_only() {
        let sr = 48000;
        let mut signal = vec![0.0; sr as usize / 2];
        signal.extend(sine(60.0, 0.5, sr, 0.8));
        let mut s = Spectrum::new(sr);
        let mut flux = Vec::new();
        s.push(&signal, |f| flux.push(f.flux_low));
        let onset_frame = (sr as usize / 2) / HOP;
        let before: f32 = flux[..onset_frame.saturating_sub(4)].iter().sum();
        let around: f32 = flux[onset_frame.saturating_sub(4)..onset_frame + 4]
            .iter()
            .sum();
        assert!(before < 1e-4, "flux before the hit: {before}");
        assert!(around > 0.1, "flux at the hit: {around}");
    }
}
