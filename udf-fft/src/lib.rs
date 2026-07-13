//! `udf-fft` — the FFT instability-detection UDF for `tpt-keystone-db`.
//!
//! Compiled to `wasm32-unknown-unknown` (or `wasm32-wasip1`) and registered
//! via `CREATE FUNCTION detect_instability(float8[]) RETURNS float8[]
//! LANGUAGE wasm AS '<base64>'`.
//!
//! It follows the engine's `(ptr, len)` linear-memory ABI (see
//! `tpt-keystone/src/executor/udf.rs`):
//! * **Args** (`float8[]`): passed as an `(i32 ptr, i32 len)` pair. The
//!   engine allocates a buffer with our exported `alloc`, writes the little-
//!   endian `f64` elements, and calls the entry point.
//! * **Return** (`float8[]`): a packed `i64` = `(ptr << 32) | len`. The
//!   engine reads `len` `f64` elements back out of linear memory.
//!
//! The math here mirrors `tpt-fluxstream::signal::magnitude_spectrum` exactly
//! so the in-DB results cross-check against the native oracle.

#![allow(static_mut_refs)]

use num_complex::Complex64;
use rustfft::{num_complex::Complex, FftPlanner};

/// Linear bump allocator. Single-threaded WASM: a plain bump pointer over a
/// static heap. The engine calls `alloc(len)` both to hand us input buffers
/// and (via our entry point) to obtain the output buffer.
const HEAP_SIZE: usize = 4 * 1024 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];
static mut HEAP_OFFSET: usize = 0;

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    let len = len.max(0) as usize;
    // Align to 8 bytes (f64) so we never produce a misaligned pointer.
    let aligned = (len + 7) & !7;
    unsafe {
        let off = HEAP_OFFSET;
        if off + aligned > HEAP_SIZE {
            return 0; // signal allocation failure
        }
        HEAP_OFFSET = off + aligned;
        HEAP.as_ptr().add(off) as i32
    }
}

/// Compute the one-sided FFT magnitude spectrum of `signal` (length `n`).
/// Returns `n/2 + 1` magnitudes. Mirrors the native oracle.
pub fn magnitude_spectrum(signal: &[f64]) -> Vec<f64> {
    let n = signal.len();
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(n);
    let mut buf: Vec<Complex64> = signal.iter().map(|&x| Complex::new(x, 0.0)).collect();
    fft.process(&mut buf);

    let half = n / 2;
    let out: Vec<f64> = buf[..=half].iter().map(|c| c.norm()).collect();
    out
}

/// Entry point: `detect_instability(ptr, len) -> (ptr << 32) | len`.
#[no_mangle]
pub extern "C" fn detect_instability(ptr: i32, len: i32) -> i64 {
    let len = len.max(0) as usize;
    if len == 0 {
        return 0;
    }
    let signal: Vec<f64> = unsafe {
        let base = HEAP.as_ptr().add(ptr as usize) as *const f64;
        std::slice::from_raw_parts(base, len).to_vec()
    };

    let spectrum = magnitude_spectrum(&signal);
    let out_len = spectrum.len();

    let out_ptr = alloc((out_len * std::mem::size_of::<f64>()) as i32);
    if out_ptr == 0 {
        return 0;
    }
    unsafe {
        let base = HEAP.as_ptr().add(out_ptr as usize) as *mut f64;
        for (i, &v) in spectrum.iter().enumerate() {
            base.add(i).write(v);
        }
    }

    // Pack (ptr << 32) | len as i64.
    ((out_ptr as i64) << 32) | (out_len as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum_matches_native_oracle() {
        // A 50 Hz sine sampled at 10 kHz; native oracle lives in tpt-fluxstream.
        let n = 1024usize;
        let dt = 1e-4;
        let omega = 2.0 * std::f64::consts::PI * 50.0;
        let signal: Vec<f64> = (0..n).map(|i| (omega * (i as f64) * dt).sin()).collect();

        let spec = magnitude_spectrum(&signal);
        assert_eq!(spec.len(), n / 2 + 1);

        // Compare against the native oracle from the tpt-fluxstream crate.
        let native = tpt_fluxstream::signal::magnitude_spectrum(&signal, dt);
        assert_eq!(spec.len(), native.magnitude.len());
        for (a, b) in spec.iter().zip(native.magnitude.iter()) {
            assert!((a - b).abs() < 1e-9, "{a} vs {b}");
        }
    }
}
