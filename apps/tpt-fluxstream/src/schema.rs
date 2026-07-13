//! SQL DDL for the diagnostic time-series tables tpt-fluxstream manages.
//!
//! These strings target `tpt-keystone-db`'s Postgres-wire dialect (see
//! `tpt-keystone/src/executor/ddl.rs`). Types follow `ColumnType::from_name`:
//! `bigint`, `double precision`, `text`, `bool`, `timestamp`, etc.

/// DDL for the core `diagnostics` time-series table. One row per sample.
pub fn ddl_diagnostics() -> &'static str {
    "CREATE TABLE diagnostics (
        shot        bigint,
        sample_idx  bigint,
        time        double precision,
        channel     text,
        value       double precision
    )"
}

/// DDL for a per-shot instability summary produced by the in-DB UDF.
pub fn ddl_instability() -> &'static str {
    "CREATE TABLE instability_report (
        shot              bigint,
        dominant_freq_hz  double precision,
        dominant_mag      double precision,
        instability_score  double precision
    )"
}

/// `CREATE FUNCTION` for the WASM FFT UDF `detect_instability`.
///
/// The UDF takes a `float8[]` window of samples and returns a `float8[]`
/// magnitude spectrum (one-sided). `wasm_base64` is the
/// `udf-fft` crate compiled to `wasm32` and base64-encoded.
pub fn ddl_detect_instability(wasm_base64: &str) -> String {
    format!(
        "CREATE FUNCTION detect_instability(float8[]) RETURNS float8[] LANGUAGE wasm AS '{wasm_base64}'"
    )
}

/// Full Phase-1 schema script (tables only — UDF registration is separate so
/// the compiled wasm can be injected at deploy time).
pub fn ddl() -> Vec<&'static str> {
    vec![ddl_diagnostics(), ddl_instability()]
}
