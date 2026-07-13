# CLAUDE.md — TPT Ignis

Rust workspace of fusion-development apps built on the sibling `tpt-keystone-db`
and `tpt-appfront` repositories.

## Conventions

- All external dependencies are declared once in the root `Cargo.toml` under
  `[workspace.dependencies]` and inherited via `dep.workspace = true`. Do not
  version-pin a dependency in a member manifest.
- Sibling dependencies (`tpt-sdk`, `tpt-appfront-*`) are **path** dependencies
  pointing at `../tpt-keystone-db/...` and `../tpt-appfront/crates/...`. They
  are checked out as siblings (see CI).
- Internal crates use `tpt-` prefixes; apps live under `apps/`, shared libs
  under `crates/`.
- The Rust toolchain is pinned in `rust-toolchain.toml` (1.90.0). Do not rely
  on newer features without bumping the pin and the CI matrix.
- Tests must run offline by default. Integration tests that need a live
  `tpt-keystone` instance must be `#[ignore]`d so `cargo test` stays green in CI.

## Layout

- `crates/tpt-plasma-schemas` — IMAS-inspired diagnostic structs + msgpack.
- `crates/tpt-physics-math` — coordinate transforms and flux-surface math.
- `crates/tpt-ui-components` — reusable UI widgets (Phase 2+).
- `apps/tpt-fluxstream` — ingestion + native FFT oracle + in-DB UDF wiring.
- `apps/tpt-halo`, `apps/tpt-aether`, `apps/tpt-oracle` — Phase 2-4 apps.
- `udf-fft` — FFT WASM UDF for `tpt-keystone-db`.

## Common commands

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all
```
