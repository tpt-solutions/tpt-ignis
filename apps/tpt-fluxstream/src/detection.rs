//! Instability detection used by both the native oracle and the in-DB UDF.
//!
//! The native oracle (`signal`) and the `udf-fft` WASM module both compute
//! the one-sided FFT magnitude spectrum of a signal window. The host then
//! reduces that spectrum to an [`InstabilityReport`]. `ingest` cross-checks
//! the DB UDF's spectrum against the native oracle's to confirm they agree.

use crate::signal::{instability_score, magnitude_spectrum};
use tpt_plasma_schemas::Sample;

/// A reduced instability assessment for one signal window.
#[derive(Debug, Clone, PartialEq)]
pub struct InstabilityReport {
    pub shot: u64,
    pub dominant_freq_hz: f64,
    pub dominant_magnitude: f64,
    pub instability_score: f64,
}

/// Run the native oracle end-to-end on a sample window.
pub fn detect_instability(samples: &[Sample], shot: u64) -> InstabilityReport {
    let values: Vec<f64> = samples.iter().map(|s| s.value).collect();
    let dt = samples.first().map(|s| s.time).unwrap_or(0.0);
    let next = samples.get(1).map(|s| s.time).unwrap_or(0.0);
    let dt = if dt == next { 1.0 } else { (next - dt).abs() };
    let dt = dt.max(1e-12);

    let spec = magnitude_spectrum(&values, dt);
    InstabilityReport {
        shot,
        dominant_freq_hz: spec.dominant_freq_hz,
        dominant_magnitude: spec.dominant_magnitude,
        instability_score: instability_score(&spec),
    }
}

/// Reduce a magnitude spectrum (as returned by the `udf-fft` module, length
/// `n/2 + 1`) to an [`InstabilityReport`], mirroring the reduction in
/// [`detect_instability`] so the two paths are comparable.
pub fn report_from_magnitudes(
    shot: u64,
    sample_hz: f64,
    n: usize,
    magnitudes: &[f64],
) -> InstabilityReport {
    let mut dominant_idx = 0usize;
    let mut dominant_mag = 0.0f64;
    for (k, &m) in magnitudes.iter().enumerate() {
        if k > 0 && m > dominant_mag {
            dominant_mag = m;
            dominant_idx = k;
        }
    }
    let dominant_freq = (dominant_idx as f64) * sample_hz / (n as f64);
    let total: f64 = magnitudes.iter().map(|m| m * m).sum();
    let score = if total <= 0.0 {
        0.0
    } else {
        (dominant_mag * dominant_mag / total).clamp(0.0, 1.0)
    };
    InstabilityReport {
        shot,
        dominant_freq_hz: dominant_freq,
        dominant_magnitude: dominant_mag,
        instability_score: score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::SignalParams;

    #[test]
    fn native_and_reduction_agree_on_dominant_freq() {
        let p = SignalParams {
            n: 1024,
            dt: 1e-4,
            mode_freq_hz: 50.0,
            ..Default::default()
        };
        let samples = crate::synth::generate_samples(&p);
        let native = detect_instability(&samples, p.shot);

        // Re-derive via the spectrum + reduction (the path the UDF takes).
        let vals: Vec<f64> = samples.iter().map(|s| s.value).collect();
        let spec = magnitude_spectrum(&vals, p.dt);
        let reduced = report_from_magnitudes(p.shot, spec.sample_hz, vals.len(), &spec.magnitude);

        assert!((native.dominant_freq_hz - reduced.dominant_freq_hz).abs() < 1e-9);
        assert!((native.instability_score - reduced.instability_score).abs() < 1e-9);
    }
}
