//! Newtype wrappers carrying physical units. These prevent accidental mixing
//! of, e.g., Tesla and Gauss, and give the schema structs self-documenting
//! fields. All stored internally as SI base units.

use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

macro_rules! unit_newtype {
    ($name:ident, $inner:ty, $unit:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
        )]
        pub struct $name(pub $inner);

        impl $name {
            pub fn new(v: $inner) -> Self {
                Self(v)
            }
            pub fn value(&self) -> $inner {
                self.0
            }
        }

        impl From<$inner> for $name {
            fn from(v: $inner) -> Self {
                Self(v)
            }
        }

        impl Add for $name {
            type Output = Self;
            fn add(self, o: Self) -> Self {
                Self(self.0 + o.0)
            }
        }
        impl Sub for $name {
            type Output = Self;
            fn sub(self, o: Self) -> Self {
                Self(self.0 - o.0)
            }
        }
        impl Mul<$inner> for $name {
            type Output = Self;
            fn mul(self, s: $inner) -> Self {
                Self(self.0 * s)
            }
        }
        impl Div<$inner> for $name {
            type Output = Self;
            fn div(self, s: $inner) -> Self {
                Self(self.0 / s)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", self.0, $unit)
            }
        }
    };
}

// Magnetic field in Tesla.
unit_newtype!(Tesla, f64, "T");
// Line-integrated density in 1e19 m^-2 (typical interferometer unit).
unit_newtype!(LineDensity, f64, "e19 m^-2");
// Electron temperature in keV.
unit_newtype!(KiloElectronVolt, f64, "keV");
// Temperature in eV (base).
unit_newtype!(ElectronVolt, f64, "eV");
// Plasma current in Mega-Amps.
unit_newtype!(MegaAmps, f64, "MA");
// Time in seconds.
unit_newtype!(Seconds, f64, "s");
// Toroidal angle in radians.
unit_newtype!(Radians, f64, "rad");
// Minor radius in meters.
unit_newtype!(Meters, f64, "m");
// Major radius in meters.
unit_newtype!(MajorRadius, f64, "m");
