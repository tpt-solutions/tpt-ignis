//! Interferometer diagnostics: line-integrated electron density.

use crate::units::*;
use serde::{Deserialize, Serialize};

/// A single line-integrated density measurement from an interferometer chord.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterferometerMeasurement {
    pub shot: u64,
    pub time: Seconds,
    /// Line-integrated density along the chord, in 1e19 m^-2.
    pub density: LineDensity,
    /// Chord index (which sight-line through the plasma).
    pub chord: u32,
}
