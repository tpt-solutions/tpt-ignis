//! Thomson scattering diagnostics: local electron temperature & density
//! profiles inferred from laser light scattering.

use crate::units::*;
use serde::{Deserialize, Serialize};

/// A Thomson-scattering profile point: a radial location with the inferred
/// electron temperature and density there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThomsonScatteringProfile {
    pub shot: u64,
    /// Radial coordinate as a fraction of the minor radius (rho = r/a).
    pub rho: f64,
    /// Local electron temperature, in keV.
    pub electron_temp: KiloElectronVolt,
    /// Local electron density, in 1e19 m^-3.
    pub electron_density: LineDensity,
}
