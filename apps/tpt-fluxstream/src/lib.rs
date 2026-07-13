//! tpt-fluxstream — the Plasma Data Engine.
//!
//! Responsibilities (Phase 1):
//! * generate IMAS-shaped synthetic diagnostic signals ([`synth`]),
//! * provide a native FFT-based instability-detection *oracle* ([`signal`])
//!   that the in-database WASM UDF (`udf-fft`) must agree with,
//! * stream those signals into `tpt-keystone-db` over the Postgres wire
//!   protocol via bulk `COPY` ([`ingest`]).

pub mod detection;
pub mod ingest;
pub mod schema;
pub mod signal;
pub mod synth;

pub use detection::{detect_instability, InstabilityReport};
pub use schema::ddl;
