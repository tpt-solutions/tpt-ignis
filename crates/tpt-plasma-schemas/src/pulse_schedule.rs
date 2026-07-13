//! Pulse schedule: the planned时间序列 of actuator setpoints for a shot.

use crate::units::*;
use serde::{Deserialize, Serialize};

/// One scheduled setpoint in a pulse program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub time: Seconds,
    /// Target toroidal field, in Tesla.
    pub target_toroidal_field: Tesla,
    /// Target plasma current, in Mega-Amps.
    pub target_plasma_current: MegaAmps,
    /// Auxiliary heating power, in MW.
    pub heating_power_mw: f64,
}

/// The planned program for a single shot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PulseSchedule {
    pub shot: u64,
    pub description: String,
    pub entries: Vec<ScheduleEntry>,
}
