//! Synthetic diagnostic signal generation.
//!
//! Produces IMAS-shaped time series reminiscent of real fusion diagnostics:
//! * a slow MHD-like coherent oscillation (e.g. a tearing-mode `m/n` average
//!   frequency) on top of
//! * Gaussian measurement noise, and
//! * an optional disruption-like amplitude ramp where the signal grows
//!   exponentially before a "disruption" time.

use rand_distr::{Distribution, Normal};
use tpt_plasma_schemas::Sample;

/// Parameters for a synthetic diagnostic signal.
#[derive(Debug, Clone)]
pub struct SignalParams {
    pub shot: u64,
    pub channel: String,
    /// Number of samples.
    pub n: usize,
    /// Sample period in seconds.
    pub dt: f64,
    /// Coherent oscillation frequency (Hz).
    pub mode_freq_hz: f64,
    /// Coherent oscillation amplitude.
    pub mode_amp: f64,
    /// Standard deviation of Gaussian sensor noise.
    pub noise_std: f64,
    /// If `Some(t)`, the signal ramps up exponentially toward `t`
    /// (disruption-like), with the given e-folding growth rate (1/s).
    pub disruption: Option<(f64, f64)>,
}

impl Default for SignalParams {
    fn default() -> Self {
        Self {
            shot: 1,
            channel: "b_toroidal".into(),
            n: 1024,
            dt: 1e-4,
            mode_freq_hz: 50.0,
            mode_amp: 1.0,
            noise_std: 0.05,
            disruption: None,
        }
    }
}

/// Generate a synthetic signal as a vector of `(time, value)` pairs.
pub fn generate(params: &SignalParams) -> Vec<(f64, f64)> {
    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, params.noise_std).expect("valid normal");
    let omega = 2.0 * std::f64::consts::PI * params.mode_freq_hz;
    let mut out = Vec::with_capacity(params.n);
    for i in 0..params.n {
        let t = i as f64 * params.dt;
        let mut v = params.mode_amp * (omega * t).sin();
        if let Some((t_disrupt, growth)) = params.disruption {
            if t < t_disrupt {
                // Disruption-like amplitude ramp: grows exponentially as the
                // disruption time approaches.
                v += params.mode_amp * (growth * t).exp().clamp(0.0, 50.0);
            }
        }
        v += normal.sample(&mut rng);
        out.push((t, v));
    }
    out
}

/// Generate [`Sample`]s for ingestion into the database.
pub fn generate_samples(params: &SignalParams) -> Vec<Sample> {
    generate(params)
        .into_iter()
        .map(|(t, v)| Sample {
            shot: params.shot,
            time: t,
            channel: params.channel.clone(),
            value: v,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_requested_sample_count() {
        let p = SignalParams {
            n: 256,
            ..Default::default()
        };
        assert_eq!(generate(&p).len(), 256);
        assert_eq!(generate_samples(&p).len(), 256);
    }

    #[test]
    fn disruption_ramps_amplitude_up() {
        let p = SignalParams {
            n: 500,
            dt: 1e-3,
            mode_amp: 1.0,
            disruption: Some((0.4, 10.0)),
            ..Default::default()
        };
        let s = generate(&p);
        let early = s[10].1.abs();
        let late = s[390].1.abs();
        assert!(
            late > early,
            "disruption should amplify signal toward t_disrupt"
        );
    }
}
