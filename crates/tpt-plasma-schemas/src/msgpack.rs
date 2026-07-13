//! MessagePack (de)serialization helpers for shipping diagnostic structs to
//! the UI layer (egui/glow render targets over `tpt-appfront`).

use rmp_serde::{decode, encode};
use serde::Serialize;

/// Error type for MessagePack (de)serialization via `rmp-serde`.
#[derive(Debug, thiserror::Error)]
pub enum MsgpackError {
    #[error("msgpack encode error: {0}")]
    Encode(String),
    #[error("msgpack decode error: {0}")]
    Decode(String),
}

/// Anything `Serialize` for MessagePack.
pub trait ToMsgpack: Serialize {
    /// Encode `self` to a MessagePack byte vector.
    fn to_msgpack(&self) -> Result<Vec<u8>, MsgpackError> {
        encode::to_vec_named(self).map_err(|e| MsgpackError::Encode(e.to_string()))
    }
}

/// Anything `Deserialize` from MessagePack.
pub trait FromMsgpack: Sized {
    /// Decode `Self` from MessagePack bytes.
    fn from_msgpack(bytes: &[u8]) -> Result<Self, MsgpackError>;
}

impl<T: Serialize> ToMsgpack for T {}

impl<T: serde::de::DeserializeOwned> FromMsgpack for T {
    fn from_msgpack(bytes: &[u8]) -> Result<Self, MsgpackError> {
        decode::from_slice(bytes).map_err(|e| MsgpackError::Decode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magnetics::MagneticState;

    #[test]
    fn msgpack_round_trips_magnetic_state() {
        let s = MagneticState::new(1234, 2.5, 5.3, 15.0, 1.85, 0.02, 3.1);
        let bytes = s.to_msgpack().expect("encode");
        let back: MagneticState = MagneticState::from_msgpack(&bytes).expect("decode");
        assert_eq!(s, back);
    }

    #[test]
    fn msgpack_round_trips_units_display() {
        let t = crate::units::Tesla(3.2);
        let bytes = t.to_msgpack().expect("encode");
        let back: crate::units::Tesla = crate::units::Tesla::from_msgpack(&bytes).expect("decode");
        assert_eq!(t, back);
    }
}
