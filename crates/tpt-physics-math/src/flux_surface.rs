//! Simplified magnetic flux-surface geometry. We use a circular (large-aspect
//! ratio) tokamak approximation: flux surfaces are concentric circles in the
//! `R-Z` (poloidal) plane, centered on the magnetic axis. This is the
//! Phase-1 stand-in for a full Grad-Shafranov solve (Phase 2+).

use ndarray::Array1;
use tpt_plasma_schemas::units::*;

/// A circular flux surface at minor radius `a`, centered on the magnetic axis
/// `(r0, z0)` in the poloidal plane.
#[derive(Debug, Clone, Copy)]
pub struct FluxSurface {
    pub axis_r: MajorRadius,
    pub axis_z: Meters,
    pub minor_radius: Meters,
}

impl FluxSurface {
    pub fn new(axis_r: f64, axis_z: f64, minor_radius: f64) -> Self {
        Self {
            axis_r: MajorRadius(axis_r),
            axis_z: Meters(axis_z),
            minor_radius: Meters(minor_radius),
        }
    }

    /// Poloidal cross-section points (R, Z) tracing this surface.
    pub fn cross_section(&self, n: usize) -> Vec<(f64, f64)> {
        (0..n)
            .map(|i| {
                let theta = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
                (
                    self.axis_r.value() + self.minor_radius.value() * theta.cos(),
                    self.axis_z.value() + self.minor_radius.value() * theta.sin(),
                )
            })
            .collect()
    }

    /// Surface area of a circular torus (2 * pi^2 * R0 * a).
    pub fn surface_area(&self) -> f64 {
        2.0 * std::f64::consts::PI.powi(2) * self.axis_r.value() * self.minor_radius.value()
    }

    /// Volume enclosed by a circular torus (2 * pi^2 * R0 * a^2).
    pub fn volume(&self) -> f64 {
        2.0 * std::f64::consts::PI.powi(2) * self.axis_r.value() * self.minor_radius.value().powi(2)
    }
}

/// Compute a radial profile `f(rho)` sampled on `n` points in `[0, a]`,
/// given a closure producing the value at each normalized radius `rho = r/a`.
pub fn radial_profile<F>(a: Meters, n: usize, f: F) -> Array1<f64>
where
    F: Fn(f64) -> f64,
{
    let mut v = Array1::zeros(n);
    for (i, s) in v.iter_mut().enumerate() {
        let rho = if n > 1 {
            (i as f64) / ((n - 1) as f64)
        } else {
            0.0
        };
        *s = f(rho) * a.value();
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flux_surface_area_and_volume_positive() {
        let fs = FluxSurface::new(1.85, 0.0, 0.5);
        assert!(fs.surface_area() > 0.0);
        assert!(fs.volume() > 0.0);
    }

    #[test]
    fn cross_section_is_closed_loop() {
        let fs = FluxSurface::new(1.85, 0.0, 0.5);
        let pts = fs.cross_section(16);
        assert_eq!(pts.len(), 16);
    }

    #[test]
    fn radial_profile_scales_by_minor_radius() {
        let p = radial_profile(Meters(0.5), 3, |rho| rho);
        assert_eq!(p.len(), 3);
        assert!((p[2] - 0.5).abs() < 1e-12);
    }
}
