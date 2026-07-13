//! tpt-fluxstream CLI — Phase-1 ingestion driver.
//!
//! Generates a synthetic diagnostic shot and (optionally) streams it into a
//! live `tpt-keystone` instance. With no `TPT_KEYSTONE_ADDR` set it just
//! prints the native instability report, which is handy for local smoke tests.

use std::path::PathBuf;

use clap::Parser;
use tpt_fluxstream::{
    detect_instability,
    ingest::{ensure_schema, ingest_batch, load_udf_wasm_base64},
    schema::ddl_detect_instability,
    synth::{generate_samples, SignalParams},
};

#[derive(Parser, Debug)]
#[command(
    name = "tpt-fluxstream",
    about = "Plasma data engine: synthetic ingest + in-DB FFT"
)]
struct Cli {
    /// tpt-keystone Postgres-wire address, e.g. 127.0.0.1:5432.
    #[arg(long, env = "TPT_KEYSTONE_ADDR")]
    addr: Option<String>,

    /// Number of samples to synthesize.
    #[arg(long, default_value_t = 1024)]
    n: usize,

    /// Sample period in seconds.
    #[arg(long, default_value_t = 1e-4)]
    dt: f64,

    /// Coherent mode frequency in Hz.
    #[arg(long, default_value_t = 50.0)]
    mode_hz: f64,

    /// Path to the compiled `udf-fft.wasm` to register in-DB.
    #[arg(long)]
    udf_wasm: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let params = SignalParams {
        n: cli.n,
        dt: cli.dt,
        mode_freq_hz: cli.mode_hz,
        ..Default::default()
    };
    let samples = generate_samples(&params);
    let report = detect_instability(&samples, params.shot);
    println!(
        "native oracle: dominant_freq={:.3} Hz  score={:.4}",
        report.dominant_freq_hz, report.instability_score
    );

    let Some(addr) = cli.addr else {
        println!("TPT_KEYSTONE_ADDR not set; skipping DB ingestion.");
        return Ok(());
    };

    let mut client = tpt_sdk::keystone::KeystoneClient::connect(&addr).await?;
    ensure_schema(&mut client).await?;

    if let Some(wasm) = cli.udf_wasm {
        let b64 = load_udf_wasm_base64(&wasm)?;
        client.query(&ddl_detect_instability(&b64)).await?;
        println!("registered detect_instability UDF from {}", wasm.display());
    }

    let n = ingest_batch(&mut client, &samples).await?;
    println!("ingested {n} samples into {addr}");
    Ok(())
}
