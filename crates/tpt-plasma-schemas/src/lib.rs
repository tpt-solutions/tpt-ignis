//! IMAS-inspired diagnostic data models for the TPT Ignis suite.
//!
//! These structs mirror the shape of the ITER Integrated Modeling & Analysis
//! Suite (IMAS) data dictionary enough for synthetic fusion diagnostics to be
//! represented, serialized to Postgres-wire text (via `tpt-sdk`'s `Value`s)
//! and to MessagePack (for UI rendering). They are intentionally a small,
//! self-contained subset — not a full IMAS implementation.

pub mod interferometer;
pub mod magnetics;
pub mod msgpack;
pub mod pulse_schedule;
pub mod thomson_scattering;
pub mod units;

pub use units::*;

/// A single time-stamped diagnostic sample. `time` is in seconds relative to
/// the start of the pulse; `channel` identifies the diagnostic subsystem.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    pub shot: u64,
    pub time: f64,
    pub channel: String,
    pub value: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_serializes_to_json() {
        let s = Sample {
            shot: 1,
            time: 0.0,
            channel: "b".into(),
            value: 1.0,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Sample = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn units_are_self_documenting_via_display() {
        assert_eq!(Tesla(5.3).to_string(), "5.3T");
        assert_eq!(MegaAmps(15.0).to_string(), "15MA");
    }
}
