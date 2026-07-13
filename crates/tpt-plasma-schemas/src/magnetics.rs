//! Magnetic diagnostics: equilibrium field measurements and coil currents.

use crate::units::*;
use serde::{Deserialize, Serialize};

/// A magnetic equilibrium snapshot: toroidal field, plasma current, and the
/// geometric axis position. Mirrors an IMAS `magnetics` IDS subset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MagneticState {
    pub shot: u64,
    pub time: Seconds,
    /// Toroidal (confining) magnetic field on axis, in Tesla.
    pub toroidal_field: Tesla,
    /// Total plasma current, in Mega-Amps.
    pub plasma_current: MegaAmps,
    /// Major radius of the magnetic axis, in meters.
    pub magnetic_axis_r: MajorRadius,
    /// Vertical position of the magnetic axis, in meters.
    pub magnetic_axis_z: Meters,
    /// Safety factor at the 95% flux surface (edge).
    pub q95: f64,
}

impl MagneticState {
    pub fn new(
        shot: u64,
        time: f64,
        toroidal_field: f64,
        plasma_current: f64,
        magnetic_axis_r: f64,
        magnetic_axis_z: f64,
        q95: f64,
    ) -> Self {
        Self {
            shot,
            time: Seconds(time),
            toroidal_field: Tesla(toroidal_field),
            plasma_current: MegaAmps(plasma_current),
            magnetic_axis_r: MajorRadius(magnetic_axis_r),
            magnetic_axis_z: Meters(magnetic_axis_z),
            q95,
        }
    }
}
