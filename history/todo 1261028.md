# TPT Ignis — Project Checklist

Derived from `spec.txt` and the approved implementation plan. Phase 0 is prerequisite work in the sibling `tpt-keystone-db` repo (not part of this repo's own workspace); Phases 1-4 mirror spec.txt §8's roadmap.

---

## Phase 0: `tpt-keystone-db` prerequisites (bulk COPY + array-capable UDFs)

*Lives in `c:/Programming/tpt-keystone-db`, not in this repo — tracked here because tpt-fluxstream's Phase 1 ingestion depends on it.*

- [ ] 0a. Add COPY protocol client support to `tpt-sdk`:
  - [x] Add `CopyInResponse`/`CopyOutResponse`/`CopyData`/`CopyDone` to `tpt-sdk`'s own (client-side) `BackendMessage` enum in `keystone/wire.rs`, parsed in `read_message`
  - [x] Add `write_copy_data`/`write_copy_done`/`write_copy_fail` frontend writers in `keystone/wire.rs`
  - [x] Add `KeystoneClient::copy_in(table, columns, rows) -> Result<u64, KeystoneError>` in `keystone/mod.rs`, encoding rows in the same tab-delimited/backslash-escape text format `executor::copy::encode_copy_line` expects server-side
  - [ ] Client-driven `copy_in`/`copy_out` round-trip integration test against a live `tpt-keystone` instance
    - *Wire types, writers, and `copy_in` landed in `tpt-keystone-db` commit `3158087` (2026-07-13); only wire-encoding unit tests exist so far, no live-server round-trip test yet.*
- [ ] 0b. Array/bytea-capable WASM UDF parameters:
  - [x] Extend `tpt-sdk`'s `Value` enum with a `FloatArray(Vec<f64>)` variant + wire text-format encode/decode
  - [x] Extend `executor/udf.rs`'s `wasm_type()` to accept an array/bytea `ColumnType` instead of `bail!`-ing
  - [x] Implement pointer+length ABI in `executor/udf.rs::call()`: UDF module exports `memory` + `alloc(len: i32) -> i32`; host writes array bytes via `Memory::write`, calls UDF with `(ptr, len)`; UDF returns a packed `i64` (`(ptr << 32) | len`)
  - [ ] Add fuel-exhaustion (infinite-loop UDF) and trap (`unreachable`/OOB memory) test cases to `tpt-keystone-db`'s Linux CI job — *not runnable locally on Windows, `wasmtime::traphandlers::catch_traps` crashes the test process here (`STATUS_STACK_BUFFER_OVERRUN`); requires the Linux CI runner*
    - *`Value`/`wasm_type`/pointer+length ABI implemented and unit-tested in `tpt-keystone/src/executor/udf.rs` (`tpt-keystone-db` commit `a4506ab`, 2026-07-13); fuel/trap CI job still not wired up.*
- [ ] Verify Phase 0 end-to-end against a running local `tpt-keystone` instance before starting tpt-fluxstream's ingestion wiring

## Phase 1: Foundation & Data Ingestion (Months 1-2)

- [x] Initialize tpt-ignis Cargo workspace (root `Cargo.toml`, `rust-toolchain.toml` pinned to 1.90.0, `deny.toml`, `LICENSE` (Apache-2.0), `.gitignore`, `README.md`, `CLAUDE.md`)
- [x] Configure CI (`.github/workflows/ci.yml`): three-repo checkout (tpt-ignis/tpt-keystone-db/tpt-appfront as siblings), fmt, clippy, `cargo build --workspace`, `cargo nextest run --workspace`, `cargo deny check`
- [x] Scaffold all workspace members as compiling stubs: `tpt-plasma-schemas`, `tpt-physics-math`, `tpt-ui-components`, `tpt-fluxstream`, `tpt-halo`, `tpt-aether`, `tpt-oracle`
- [x] Implement `tpt-plasma-schemas`: IMAS-inspired structs (`magnetics`, `interferometer`, `thomson_scattering`, `pulse_schedule`), `units.rs` newtypes, msgpack `to_msgpack`/`from_msgpack` helpers
- [x] Implement `tpt-fluxstream` synthetic data generator (`synth.rs`): IMAS-shaped signals (MHD-mode-like sine + Gaussian noise, disruption-like amplitude ramp)
- [x] Implement `tpt-fluxstream` native FFT (`signal.rs`, `rustfft`) as instability-detection test oracle
- [x] Implement `tpt-fluxstream` ingestion (`ingest.rs`): schema DDL, bulk `copy_in` batched loads via `tpt-sdk` (depends on Phase 0a), in-DB `detect_instability` UDF invocation (depends on Phase 0b) cross-checked against the native oracle
  - *Code-complete and unit-tested. The live-DB round-trip `ingest_batch_round_trip` and UDF-registration path are `#[ignore]`d — they need a running `tpt-keystone` instance and a compiled `udf-fft.wasm`.*
- [x] Implement `udf-fft` crate: FFT UDF compiled to `wasm32-unknown-unknown`/`wasm32-wasip1` using the Phase 0b pointer+length ABI, registered via `CREATE FUNCTION ... LANGUAGE wasm`
  - *Host lib + `(ptr,len)` ABI implemented; unit test confirms its magnitude spectrum matches `tpt-fluxstream`'s native oracle exactly. WASM build target (`wasm32`) is the deploy artefact — compile separately with `cargo build --target wasm32-unknown-unknown -p udf-fft`.*
- [ ] `mastu.rs` real MAST-U/ITER open-data loader — *optional stretch goal, feature-gated (`mastu-loader`), network/format-tooling gated, not required for Phase 1 completion*
- [ ] **Milestone: synthetic diagnostic data streams end-to-end from tpt-fluxstream into tpt-keystone-db via bulk COPY, with in-DB FFT UDF results matching the native oracle**
  - *Blocked on a running `tpt-keystone` instance (Phase 0 verification) to execute the `#[ignore]`d integration test. All constituent pieces (synthetic gen, native oracle, COPY ingestion wiring, WASM UDF + cross-check) are implemented and unit-tested in isolation.*

## Phase 2: Physics Math & Visualization (Months 3-4)

- [x] `tpt-physics-math`: real flux-surface/equilibrium math — `equilibrium.rs` adds Miller/Shafranov-shaped surfaces (`TokamakEquilibrium`), the `q` safety-factor profile for a parabolic current, poloidal beta, and `1/R` toroidal-field decay, beyond the circular `cylindrical_to_toroidal` stub
- [x] `tpt-halo`: real `appfront-core` `UITree` renderer — `render_ui`/`render_html` draw the poloidal (R–Z) cross-section and toroidal projection as ASCII rasterisations of the equilibrium, plus live gauge widgets; target-agnostic so it drives native/egui/canvas/WASM/TUI backends
- [x] Basic tokamak torus rendering from `tpt-physics-math` geometry, with live diagnostic readouts from `tpt-ui-components` gauges driven by a `LiveState` feed
- [x] `tpt-ui-components`: real `gauge_widget` / `gauge_dashboard` / `diagnostic_chart` (`chart_samples`) widgets on top of `appfront-core`, each carrying AI metadata (`read_gauge`, `read_chart`) for agent consumption

## Phase 3: Digital Twin & Topology (Months 5-6)

- [x] `tpt-aether`: real reactor topology model — `ReactorGraph` with geospatial component positions, `mock_reactor()` (pumps→HTS segments→divertor→vessel), inverse-square neutron-flux and cooling-cascade `thermal_load` mapping, plus `persist`/`dependents_via_db` (recursive-CTE) mirroring into `tpt-keystone-db`'s Plexus/Meridian engines (live-DB paths)
- [x] Integrate `tpt-halo` to overlay live thermal data onto the `tpt-aether` geospatial model — `tpt-halo`'s integration test feeds `ReactorGraph::thermal_load(...)` (with a `pump_a` failure cascade) into `LiveState.thermal_load` and re-renders the view

## Phase 4: AI & Agent Integration (Months 7-8)

- [x] `tpt-oracle`: real vector embeddings (`embed` + `cosine_similarity` + `nearest_by_embedding`, the same math as Prism) for historical-state matching, beyond the Phase-1 Euclidean stub
- [x] Expose the system via an MCP server (`McpServer`, JSON-RPC 2.0 over stdio) with `query_plasma_state` / `suggest_parameters` tools, and JSON-LD (`to_json_ld` via `appfront-ai-schema`), letting an LLM agent query plasma state and suggest parameter adjustments

## Phase 5: Adoption Hardening (2026-07-24 review)

Follow-ups from a full project review (stub audit, security audit, adoption tooling). None of Phase 1-4's own code was found to contain literal stubs/TODOs — this phase is genuinely new work identified by that review.

- [ ] `mastu.rs`: implement the real MAST-U/ITER open-data loader as the `mastu-loader` feature on `tpt-fluxstream` (fixture-tested parsing, `#[ignore]`d live-fetch test, CLI wiring)
- [ ] Security: fix `udf-fft::detect_instability`/`alloc` OOB-read and 32-bit integer-overflow bounds bypass (`checked_add`, regression test)
- [ ] Security: fix `tpt-aether::dependents_via_db` SQL injection (validate/escape `start` before interpolating into the recursive-CTE string; note upstream `tpt-sdk` parameterized-query follow-up)
- [ ] CI: remove `continue-on-error: true` from the `test` and `deny` steps in `.github/workflows/ci.yml` (currently masking real regressions and security-advisory failures); add a `cargo build --target wasm32-unknown-unknown -p udf-fft` step so the actual deploy target is compiled in CI
- [ ] Onboarding: add root `justfile` (`setup`, `doctor`, `build`, `test`, `fmt`, `clippy`, `deny`, `wasm-build`, `check`, `demo`), `CONTRIBUTING.md`, and a README "Quickstart" section
- [ ] README: correct the "Phase 2-4 scaffolded as compiling stubs" line — Phases 1-4 are implemented with passing unit tests; only Phase 0 (sibling repo) and the optional `mastu-loader` (this phase) are open

---

## Known limitations carried forward from spec.txt (tracked, not blocking)

- `tpt-keystone-db`'s six "engines" (Meridian/geo, Prism/vector, Chronos/timeseries, Plexus/graph, Canopy/document, relational core) are single-node/local-only — no distributed secondary indexes yet. Fine for Phase 1-4 of this platform; a real production reactor deployment would need that solved upstream first.
- `tpt-sdk`'s wire client is text-format only (no binary-format params) — acceptable at Phase 1 data volumes; revisit if/when ingestion throughput becomes the bottleneck.
- No sibling suite currently path/git-depends on `tpt-keystone-db`/`tpt-appfront` — this repo establishes that convention for the first time; there's no prior art elsewhere in the codebase to fall back on if the path-dependency approach hits friction.
