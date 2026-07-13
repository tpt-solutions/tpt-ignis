//! Ingestion: stream synthetic diagnostics into `tpt-keystone-db` via the
//! bulk `COPY ... FROM STDIN` path of `tpt-sdk`'s `KeystoneClient`.
//!
//! The hot path converts each [`Sample`] into a row of `tpt-sdk::Value`s and
//! hands a whole batch to `copy_in`, which is one wire round-trip per batch
//! instead of one per row.
//!
//! Integration tests here require a *live* `tpt-keystone` instance and are
//! therefore `#[ignore]`d so `cargo test` stays green in CI.

use std::path::PathBuf;

use tpt_plasma_schemas::Sample;
use tpt_sdk::keystone::{KeystoneClient, Value};

/// Errors raised by the ingestion pipeline.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("sdk error: {0}")]
    Sdk(#[from] tpt_sdk::keystone::KeystoneError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no samples to ingest")]
    Empty,
}

/// Convert a [`Sample`] into the column order expected by `diagnostics`:
/// `(shot, sample_idx, time, channel, value)`.
pub fn sample_to_row(s: &Sample, idx: usize) -> Vec<Value> {
    vec![
        Value::Int(s.shot as i64),
        Value::Int(idx as i64),
        Value::Float(s.time),
        Value::Text(s.channel.clone()),
        Value::Float(s.value),
    ]
}

/// Ingest one batch of samples into the `diagnostics` table via `COPY`.
pub async fn ingest_batch(
    client: &mut KeystoneClient,
    samples: &[Sample],
) -> Result<u64, IngestError> {
    if samples.is_empty() {
        return Err(IngestError::Empty);
    }
    let rows: Vec<Vec<Value>> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| sample_to_row(s, i))
        .collect();
    let n = client
        .copy_in(
            "diagnostics",
            &["shot", "sample_idx", "time", "channel", "value"],
            &rows,
        )
        .await?;
    Ok(n)
}

/// Read a base64-encoded `udf-fft` wasm module from disk (deployment artefact).
pub fn load_udf_wasm_base64(path: &PathBuf) -> Result<String, IngestError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(path)?;
    Ok(STANDARD.encode(bytes))
}

/// Ensure the Phase-1 schema exists (idempotent callers should DROP/CREATE or
/// guard with a catalog check — left to the operator for now).
pub async fn ensure_schema(client: &mut KeystoneClient) -> Result<(), IngestError> {
    use crate::schema::{ddl_diagnostics, ddl_instability};
    client.query(ddl_diagnostics()).await?;
    client.query(ddl_instability()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::SignalParams;

    #[test]
    fn sample_to_row_is_well_formed() {
        let p = SignalParams {
            shot: 7,
            channel: "b_tor".into(),
            ..Default::default()
        };
        let samples = crate::synth::generate_samples(&p);
        let row = sample_to_row(&samples[0], 0);
        assert_eq!(row.len(), 5);
        assert_eq!(row[0], Value::Int(7));
        assert!(matches!(row[4], Value::Float(_)));
    }

    // Live-DB integration tests. Run manually with a running tpt-keystone:
    //   cargo test --package tpt-fluxstream -- --ignored
    #[tokio::test]
    #[ignore = "requires a live tpt-keystone instance"]
    async fn ingest_batch_round_trip() {
        let mut client = KeystoneClient::connect("127.0.0.1:5432").await.unwrap();
        let p = SignalParams {
            n: 64,
            ..Default::default()
        };
        let samples = crate::synth::generate_samples(&p);
        let n = ingest_batch(&mut client, &samples).await.unwrap();
        assert_eq!(n as usize, samples.len());
    }
}
