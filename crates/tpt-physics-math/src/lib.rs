//! Core mathematical operations for plasma physics, shared across the TPT
//! Ignis suite: coordinate transforms and magnetic flux-surface geometry.

use tpt_plasma_schemas::units::*;

pub mod coordinates;
pub mod equilibrium;
pub mod flux_surface;

pub use coordinates::*;
pub use equilibrium::*;
pub use flux_surface::*;

/// Toroidal plasma beta: ratio of plasma pressure to magnetic pressure.
///
/// `beta = 2 * mu0 * <p> / <B^2>`
///
/// With `pressure` in Pascals and `b_field` in Tesla, `mu0 = 4*pi*1e-7 H/m`.
pub fn plasma_beta(pressure_pa: f64, b_field: Tesla) -> f64 {
    const MU0: f64 = 4.0 * std::f64::consts::PI * 1e-7;
    let b_sq = b_field.value().powi(2);
    if b_sq == 0.0 {
        return 0.0;
    }
    2.0 * MU0 * pressure_pa / b_sq
}

/// Larmor (gyro-) radius of a particle with mass `m_kg`, charge `q_c`,
/// speed `v_m_s`, in field `b`.
///
/// `r_L = m * v_perp / (|q| * B)`
pub fn larmor_radius(m_kg: f64, v_perp: f64, q_c: f64, b: Tesla) -> Meters {
    let b_abs = b.value().abs();
    if b_abs == 0.0 {
        return Meters(f64::INFINITY);
    }
    Meters(m_kg * v_perp / (q_c.abs() * b_abs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_is_dimensionless_and_positive() {
        // 1 Pa plasma pressure, 1 T field. beta ~ 2*mu0/1 ~ 2.5e-6
        let b = plasma_beta(1.0, Tesla(1.0));
        assert!(b > 0.0 && b < 1e-3);
    }

    #[test]
    fn larmor_grows_with_field_inverse() {
        let r1 = larmor_radius(1.67e-27, 1e6, 1.6e-19, Tesla(1.0));
        let r2 = larmor_radius(1.67e-27, 1e6, 1.6e-19, Tesla(2.0));
        assert!(r2.value() < r1.value());
    }
}
