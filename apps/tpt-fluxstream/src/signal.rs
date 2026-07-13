//! Native FFT instability-detection oracle.
//!
//! This is the reference implementation the in-database WASM UDF (`udf-fft`)
//! must agree with. Both compute the real-input FFT magnitude spectrum of a
//! signal and surface the dominant frequency / growth as an instability score.
//! Keeping the oracle in-tree lets `ingest` cross-check the DB UDF results
//! against a trusted local computation.

use num_complex::Complex64;
use rustfft::{num_complex::Complex, FftPlanner};
use tpt_plasma_schemas::Sample;

/// Output of the native instability detection.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    /// Sample frequency in Hz.
    pub sample_hz: f64,
    /// FFT bin frequencies (Hz), length `n/2`.
    pub freqs: Vec<f64>,
    /// FFT magnitude per bin, length `n/2`.
    pub magnitude: Vec<f64>,
    /// Frequency (Hz) of the largest non-DC bin.
    pub dominant_freq_hz: f64,
    /// Magnitude at the dominant bin.
    pub dominant_magnitude: f64,
}

/// Compute the real-input FFT magnitude spectrum of `signal` sampled at
/// `dt` seconds per sample.
pub fn magnitude_spectrum(signal: &[f64], dt: f64) -> Spectrum {
    let n = signal.len();
    let sample_hz = if dt > 0.0 { 1.0 / dt } else { 1.0 };

    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(n);
    let mut buf: Vec<Complex64> = signal.iter().map(|&x| Complex::new(x, 0.0)).collect();
    fft.process(&mut buf);

    // One-sided spectrum: bins 0..=n/2.
    let half = n / 2;
    let mut freqs = Vec::with_capacity(half + 1);
    let mut magnitude = Vec::with_capacity(half + 1);
    let mut dominant_idx = 0usize;
    let mut dominant_mag = 0.0f64;
    for k in 0..=half {
        let f = (k as f64) * sample_hz / (n as f64);
        let m = buf[k].norm();
        freqs.push(f);
        magnitude.push(m);
        // Skip DC (k==0) when searching for the dominant oscillation.
        if k > 0 && m > dominant_mag {
            dominant_mag = m;
            dominant_idx = k;
        }
    }

    let dominant_freq = freqs[dominant_idx];

    Spectrum {
        sample_hz,
        freqs,
        magnitude,
        dominant_freq_hz: dominant_freq,
        dominant_magnitude: dominant_mag,
    }
}

/// Convenience: extract the `value` series from samples.
pub fn values_of(samples: &[Sample]) -> Vec<f64> {
    samples.iter().map(|s| s.value).collect()
}

/// An instability score in `[0, 1]`: normalized dominant magnitude relative
/// to the total signal energy. High coherence → high score.
pub fn instability_score(spectrum: &Spectrum) -> f64 {
    let total: f64 = spectrum.magnitude.iter().map(|m| m * m).sum();
    if total <= 0.0 {
        return 0.0;
    }
    (spectrum.dominant_magnitude * spectrum.dominant_magnitude / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::SignalParams;

    #[test]
    fn spectrum_finds_mode_frequency() {
        let p = SignalParams {
            n: 1000,
            dt: 1e-3,
            mode_freq_hz: 50.0,
            ..Default::default()
        };
        let s = crate::synth::generate(&p);
        let vals: Vec<f64> = s.iter().map(|(_, v)| *v).collect();
        let spec = magnitude_spectrum(&vals, p.dt);
        // Nyquist = 500 Hz; 50 Hz bin should be exactly dominant (1 Hz bins).
        assert!(
            (spec.dominant_freq_hz - 50.0).abs() < 0.5,
            "got {}",
            spec.dominant_freq_hz
        );
        assert!(spec.magnitude.len() == 501);
    }

    #[test]
    fn score_is_in_unit_interval() {
        let p = SignalParams::default();
        let samples = crate::synth::generate_samples(&p);
        let vals = values_of(&samples);
        let spec = magnitude_spectrum(&vals, p.dt);
        let score = instability_score(&spec);
        assert!((0.0..=1.0).contains(&score));
    }
}
