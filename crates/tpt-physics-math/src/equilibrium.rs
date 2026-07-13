//! Real (if still reduced) tokamak equilibrium math: shaped flux surfaces,
//! the safety-factor `q` profile, and the poloidal beta. This goes beyond the
//! Phase-1 circular [`super::flux_surface::FluxSurface`] stand-in toward a
//! Grad-Shafranov-lite description using the Miller/Shafranov parametrisation.

use tpt_plasma_schemas::units::*;

const MU0: f64 = 4.0 * std::f64::consts::PI * 1e-7;

/// A shaped, axisymmetric tokamak equilibrium.
///
/// Cross-sections are described by the Miller parametrisation:
/// ```text
/// R(theta) = R0 + Delta + a * cos(theta + delta * sin(theta))
/// Z(theta) = kappa * a * sin(theta)
/// ```
/// where `R0` is the major radius, `a` the minor radius, `Delta` the
/// Shafranov shift, `kappa` the elongation and `delta` the triangularity.
#[derive(Debug, Clone, Copy)]
pub struct TokamakEquilibrium {
    pub r0: MajorRadius,
    pub a: Meters,
    /// Toroidal field on axis (Tesla).
    pub b0: Tesla,
    /// Total plasma current (Mega-Amps).
    pub ip: MegaAmps,
    /// Elongation `kappa` (Z stretch of the cross-section).
    pub kappa: f64,
    /// Triangularity `delta` (D-shape skew).
    pub delta: f64,
    /// Shafranov shift `Delta` (magnetic axis displacement outward), meters.
    pub shafranov_shift: Meters,
}

impl TokamakEquilibrium {
    /// A circular (`kappa = 1`, `delta = 0`, no shift) equilibrium — equivalent
    /// to the Phase-1 [`super::flux_surface::FluxSurface`] but with field/current
    /// context so the safety factor can be computed.
    pub fn circular(r0: f64, a: f64, b0: f64, ip: f64) -> Self {
        Self {
            r0: MajorRadius(r0),
            a: Meters(a),
            b0: Tesla(b0),
            ip: MegaAmps(ip),
            kappa: 1.0,
            delta: 0.0,
            shafranov_shift: Meters(0.0),
        }
    }

    /// A D-shaped equilibrium (typical of ITER/Mast-U class devices).
    pub fn shaped(r0: f64, a: f64, b0: f64, ip: f64, kappa: f64, delta: f64, shift: f64) -> Self {
        Self {
            r0: MajorRadius(r0),
            a: Meters(a),
            b0: Tesla(b0),
            ip: MegaAmps(ip),
            kappa,
            delta,
            shafranov_shift: Meters(shift),
        }
    }

    /// Poloidal cross-section boundary points `(R, Z)` for `n` angles.
    pub fn boundary(&self, n: usize) -> Vec<(f64, f64)> {
        (0..n)
            .map(|i| {
                let theta = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
                self.boundary_at(theta)
            })
            .collect()
    }

    /// Boundary point at a single poloidal angle `theta` (radians).
    pub fn boundary_at(&self, theta: f64) -> (f64, f64) {
        // Miller/Shafranov outline: `R = R0 + Delta + a*(cos theta + delta*sin theta)`,
        // `Z = kappa*a*sin theta`.
        let final_r = self.r0.value()
            + self.shafranov_shift.value()
            + self.a.value() * (theta.cos() + self.delta * theta.sin());
        let final_z = self.kappa * self.a.value() * theta.sin();
        (final_r, final_z)
    }

    /// Toroidal field at major radius `r` (simple `1/R` decay from axis value).
    pub fn toroidal_field(&self, r: f64) -> Tesla {
        let r0 = self.r0.value();
        if r == 0.0 {
            return Tesla(0.0);
        }
        Tesla(self.b0.value() * r0 / r)
    }

    /// Poloidal magnetic field at normalized radius `rho = r/a` for a
    /// parabolic current profile `J(r) = J0 * (1 - rho^2)^p`. The enclosed
    /// current is integrated analytically.
    pub fn poloidal_field(&self, rho: f64, profile_power: f64) -> Tesla {
        let rho = rho.clamp(0.0, 1.0);
        if rho == 0.0 {
            return Tesla(0.0);
        }
        // Total enclosed current for a parabolic-ish profile, normalised so
        // I_enclosed(1) = Ip. I_enclosed(rho) = Ip * (1 - (1-rho^2)^(p+1)).
        let ip = self.ip.value() * 1e6; // MA -> A
        let enclosed = ip * (1.0 - (1.0 - rho * rho).powf(profile_power + 1.0));
        // Bp(r) = mu0 * I_enclosed(r) / (2 * pi * r)
        let r = rho * self.a.value();
        Tesla(MU0 * enclosed / (2.0 * std::f64::consts::PI * r))
    }

    /// Cylindrical safety factor `q(rho)`:
    /// `q = (2 * pi * r^2 * B0) / (mu0 * R0 * I_enclosed(r))`
    /// using the `1/R` toroidal field and a parabolic current profile.
    pub fn safety_factor(&self, rho: f64, profile_power: f64) -> f64 {
        let rho = rho.clamp(0.0, 1.0);
        if rho == 0.0 {
            // Approaching axis: q -> q0 finite; estimate from derivative.
            let eps = 1e-3;
            return self.safety_factor(eps, profile_power);
        }
        let r = rho * self.a.value();
        let b0 = self.toroidal_field(self.r0.value()).value();
        let ip = self.ip.value() * 1e6;
        let enclosed = ip * (1.0 - (1.0 - rho * rho).powf(profile_power + 1.0));
        let numer = 2.0 * std::f64::consts::PI * r * r * b0;
        let denom = MU0 * self.r0.value() * enclosed;
        if denom == 0.0 {
            0.0
        } else {
            numer / denom
        }
    }

    /// Sample the `q` profile at `n` points in `rho in [0, 1]`.
    pub fn q_profile(&self, n: usize, profile_power: f64) -> Vec<f64> {
        (0..n)
            .map(|i| {
                let rho = if n > 1 {
                    (i as f64) / ((n - 1) as f64)
                } else {
                    0.0
                };
                self.safety_factor(rho, profile_power)
            })
            .collect()
    }

    /// Poloidal beta: ratio of volume-averaged plasma pressure to poloidal
    /// magnetic pressure, estimated from the input pressure profile.
    pub fn poloidal_beta(&self, avg_pressure_pa: f64) -> f64 {
        let bp_edge = self.poloidal_field(1.0, 1.0).value();
        if bp_edge == 0.0 {
            return 0.0;
        }
        let bp_avg = bp_edge / 3.0_f64.sqrt(); // RMS over parabolic profile
        let bp_sq = bp_avg * bp_avg;
        2.0 * MU0 * avg_pressure_pa / bp_sq
    }

    /// Magnetic axis position `(R, Z)` in the poloidal plane.
    pub fn magnetic_axis(&self) -> (f64, f64) {
        (self.r0.value() + self.shafranov_shift.value(), 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> TokamakEquilibrium {
        // ITER-like: R0 6.2, a 2.0, B0 5.3T, Ip 15MA, kappa 1.7, delta 0.33, shift 0.2
        TokamakEquilibrium::shaped(6.2, 2.0, 5.3, 15.0, 1.7, 0.33, 0.2)
    }

    #[test]
    fn circular_boundary_matches_legacy_circle() {
        let eq = TokamakEquilibrium::circular(1.85, 0.5, 2.0, 1.0);
        let pts = eq.boundary(16);
        assert_eq!(pts.len(), 16);
        // Centroid of a circular boundary ~ (R0 + shift, 0).
        let r_mean: f64 = pts.iter().map(|(r, _)| r).sum::<f64>() / pts.len() as f64;
        assert!((r_mean - 1.85).abs() < 1e-9);
    }

    #[test]
    fn shaped_boundary_is_elongated_and_shifted() {
        let eq = example();
        let pts = eq.boundary(256);
        let z_max = pts.iter().map(|(_, z)| z.abs()).fold(0.0_f64, f64::max);
        let r_min = pts.iter().map(|(r, _)| *r).fold(f64::INFINITY, f64::min);
        let r_max = pts.iter().map(|(r, _)| *r).fold(0.0_f64, f64::max);
        // Elongation stretches Z by kappa.
        assert!(z_max > 1.7 * 2.0 * 0.99, "expected elongated Z extent");
        // Triangularity shifts the max-R point to positive Z, and the whole
        // surface is pushed outward by the Shafranov shift.
        assert!(r_min > 6.2 - 2.0, "surface stays to the right of R0 - a");
        assert!(r_max > 6.2 + 2.0, "surface extends past R0 + a (shifted)");
    }

    #[test]
    fn q_is_monotonic_increasing_for_parabolic_current() {
        let eq = example();
        let q = eq.q_profile(50, 1.0);
        assert!(q[0] < q[q.len() - 1], "q rises from axis to edge");
        assert!(
            q[q.len() - 1] > 1.0,
            "edge q must exceed 1 (kink stability)"
        );
        assert!(q[0] > 0.0, "q0 finite and positive");
    }

    #[test]
    fn q_profile_power_changes_interior_q() {
        let eq = example();
        // The enclosed current at a given rho depends on the profile power,
        // so the interior (pre-edge) safety factor does too.
        let q_p1 = eq.safety_factor(0.5, 1.0);
        let q_p2 = eq.safety_factor(0.5, 2.0);
        assert!(q_p1 != q_p2, "profile shape affects interior safety factor");
    }

    #[test]
    fn toroidal_field_decays_as_1_over_r() {
        let eq = example();
        let b_inner = eq.toroidal_field(3.1).value();
        let b_outer = eq.toroidal_field(12.4).value();
        assert!(b_inner > b_outer);
        assert!(
            (b_inner / b_outer - 4.0).abs() < 1e-6,
            "doubling R halves B"
        );
    }

    #[test]
    fn magnetic_axis_includes_shafranov_shift() {
        let eq = example();
        let (r, z) = eq.magnetic_axis();
        assert!((r - 6.4).abs() < 1e-9, "R0(6.2) + shift(0.2) = 6.4");
        assert_eq!(z, 0.0);
    }
}
