//! Coordinate transformations between cylindrical (R, Z, phi) and toroidal
//! (Cartesian x, y, z) coordinates used to render the tokamak.

use tpt_plasma_schemas::units::*;

/// Convert a cylindrical tokamak coordinate to Cartesian 3D.
///
/// * `r`  — major radius offset (distance from the machine axis), meters
/// * `z`  — vertical height, meters
/// * `phi`— toroidal angle, radians
///
/// Returns `(x, y, z)` where the toroidal (machine) axis is `z`.
pub fn cylindrical_to_toroidal(
    r: MajorRadius,
    z: Meters,
    phi: Radians,
) -> (Meters, Meters, Meters) {
    let cphi = phi.value().cos();
    let sphi = phi.value().sin();
    (Meters(r.value() * cphi), Meters(r.value() * sphi), z)
}

/// Inverse of [`cylindrical_to_toroidal`]: Cartesian 3D back to cylindrical.
pub fn toroidal_to_cylindrical(x: Meters, y: Meters, z: Meters) -> (MajorRadius, Meters, Radians) {
    let r = (x.value().powi(2) + y.value().powi(2)).sqrt();
    let phi = y.value().atan2(x.value());
    (MajorRadius(r), z, Radians(phi))
}

/// Build a ring of `n` points tracing a circle of major radius `r` at height
/// `z`, useful for rendering a flux-surface cross-section in the poloidal
/// plane (the `R-Z` plane).
pub fn poloidal_ring(r: MajorRadius, z: Meters, n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let phi = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            (r.value() * phi.cos(), z.value() + r.value() * phi.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylindrical_round_trips() {
        let r = MajorRadius(1.85);
        let z = Meters(0.0);
        let phi = Radians(std::f64::consts::FRAC_PI_2);
        let (x, y, z2) = cylindrical_to_toroidal(r, z, phi);
        let (r2, z3, phi2) = toroidal_to_cylindrical(x, y, z2);
        assert!((r2.value() - r.value()).abs() < 1e-9);
        assert!((z3.value() - z.value()).abs() < 1e-9);
        assert!((phi2.value() - phi.value()).abs() < 1e-9);
    }

    #[test]
    fn poloidal_ring_has_n_points() {
        let ring = poloidal_ring(MajorRadius(1.0), Meters(0.0), 8);
        assert_eq!(ring.len(), 8);
    }
}
