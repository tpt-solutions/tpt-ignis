# TPT Ignis Suite

A Rust Cargo workspace of fusion-development applications built on
[`tpt-keystone-db`](https://github.com/tpt-solutions/tpt-keystone-db) and
[`tpt-appfront`](https://github.com/tpt-solutions/tpt-appfront).

See [`spec.txt`](spec.txt) for the full design document and the implementation
roadmap. [`todo.md`](todo.md) tracks the work.

## Workspace layout

```
tpt-ignis/
├── Cargo.toml            # Workspace root & shared dependency definitions
├── rust-toolchain.toml   # Pins the Rust toolchain (1.90.0)
├── crates/
│   ├── tpt-plasma-schemas/   # IMAS-inspired diagnostic data models
│   ├── tpt-physics-math/     # Flux-surface / coordinate math
│   └── tpt-ui-components/    # Reusable egui/ratatui widgets (Phase 2+)
├── apps/
│   ├── tpt-fluxstream/  # Plasma data ingestion + in-DB FFT UDFs
│   ├── tpt-halo/        # Universal visualization client (Phase 2+)
│   ├── tpt-aether/      # Digital-twin / reactor topology (Phase 3+)
│   └── tpt-oracle/      # AI disruption prediction + MCP (Phase 4+)
└── udf-fft/             # FFT UDF compiled to wasm32 for tpt-keystone-db
```

`tpt-keystone-db` and `tpt-appfront` are **sibling** repositories (not nested).
They are path-dependencies in the root `Cargo.toml`, so they must be checked out
alongside this repo:

```
c:/Programming/
├── tpt-keystone-db/
├── tpt-appfront/
└── tpt-ignis/      <- this repo
```

## Build

```sh
cargo build --workspace
cargo nextest run --workspace   # or `cargo test --workspace`
cargo clippy --workspace --all-targets
cargo fmt --all
cargo deny check
```

## Phase status

- **Phase 0** (sibling prerequisites): implemented in `tpt-keystone-db`'s
  `tpt-sdk` (`COPY ... FROM STDIN` client + `KeystoneClient::copy_in`,
  `Value`/`encode_copy_line`). Array/bytea UDF ABI is partially present.
- **Phase 1** (Foundation & ingestion): workspace initialized; `tpt-plasma-schemas`,
  `tpt-physics-math`, and `tpt-fluxstream` (synthetic generator + native FFT
  oracle + ingestion) implemented with passing unit tests.
- **Phase 2-4**: application crates scaffolded as compiling stubs; real
  implementations tracked in `todo.md`.
