# Dependency and implementation plan

Application dependencies are exact-pinned in crate manifests, `Cargo.lock` is
committed, and the workspace records an MSRV. The table distinguishes current
choices from candidates that still require a measured architecture spike.

## Language and workspace

- Rust 2024 edition.
- Stable toolchain for production; nightly only in isolated tooling if unavoidable.
- Workspace-level linting, formatting, advisory, license, and supply-chain policy.
- `aarch64-apple-darwin` as the first release target.

## Dependency families

| Concern | Preferred candidate | Alternatives / notes |
|---|---|---|
| GPU compute/render | `wgpu`, WGSL/Naga | Quarantined Metal path only after profiling |
| Apple frameworks | `objc2`, framework crates, `block2`, `dispatch2` | Minimize handwritten Objective-C FFI |
| Workbench UI | `egui` candidate over custom `wgpu` canvas | Compare with `winit` host and another retained UI in spike |
| Window/raw handles | Native AppKit host; `raw-window-handle` as needed | Avoid duplicating lifecycle ownership |
| Async I/O/IPC | `tokio` | Keep CPU analysis off async executor |
| CPU parallelism | `rayon` | Dedicated bounded pools for isolation and priority |
| Channels | `crossbeam-channel` or async bounded channels | Backpressure and cancellation required |
| Memory mapping | `memmap2` | Windowed reads remain canonical fallback |
| Bytes/buffers | `bytes`, `smallvec` where measured | Avoid pervasive clever containers |
| Hashing | `sha2` (current), `blake3` candidate for internal cache keys | Record algorithm and digest state in every public contract |
| Serialization | `serde`, `serde_json`, optional `postcard`/CBOR | JSON for interchange; binary only for large internal artifacts |
| Schemas | `schemars` or checked hand-authored JSON Schema | WIT for plugin boundary |
| Storage | SQLite via `rusqlite` or equivalent | Single writer; WAL/recovery tests |
| Compression | `zstd` for derived artifacts | Never auto-decompress source without quotas |
| Errors | `thiserror`; `anyhow` only at application boundaries | Stable structured error codes across APIs |
| Tracing | `tracing`, OSLog/signpost bridge | No source-derived dynamic fields by default |
| Plugins | `wasmtime`, `wasmtime-wasi`, component bindgen | Capability-scoped host imports |
| Capability filesystem | `cap-std` family where applicable | Plugins should usually receive source handles, not filesystem |
| Math/arrays | Minimal custom typed buffers initially | Add ndarray/Arrow only when formats justify cost |
| Spatial indexes | `rstar` or custom tile index | Needed for complex picking/annotations, not deterministic layouts |
| Bitsets/range sets | `roaring`, interval structure candidate | Benchmark against normalized vectors for common cases |
| GPU data layout | `bytemuck`, `encase` candidates | Audited POD boundaries; avoid unsafe in domain crates |
| CLI | bounded handwritten parser (current), `clap` candidate | Machine-readable output and stable exit codes |
| Testing | `proptest`, `insta`, fuzzing toolchain | Semantic artifact goldens before screenshots |
| Benchmarks | `criterion` plus custom end-to-end harness | Record device/OS/backend/input metadata |
| Supply chain | `cargo-deny`, `cargo-vet`, CycloneDX tooling | Signed release provenance |

## Dependency rules

1. Core domain types prefer `std` and small stable dependencies.
2. UI, OS, GPU, storage, and plugin runtimes remain at outer layers.
3. No parser dependency enters the trusted core without a threat review.
4. No wildcard versions or unpinned Git dependencies in release branches.
5. Avoid duplicate heavy runtimes and competing async executors.
6. Feature flags must not produce semantically different artifacts without an implementation semantics version.
7. Unsafe code is denied workspace-wide by default and allowed only in specifically audited boundary crates.
8. Every added dependency needs purpose, license, maintenance signal, transitive cost, unsafe footprint, and replacement path documented.

## Dependency review and upgrade procedure

```text
1. Select one bounded vertical slice and candidate versions.
2. Record toolchain, macOS SDK, Xcode, and deployment target.
3. Update and commit `Cargo.lock` for the application workspace.
4. Set package MSRV and CI matrix.
5. Generate dependency tree, license report, advisory report, and SBOM.
6. Add cargo-vet review/import policy.
7. Capture API/performance findings in ADRs.
8. Upgrade deliberately with semantic golden and GPU differential tests.
```

## Recommended architecture choice

**Robust path:** AppKit/`objc2` host + `wgpu` Metal + custom visualization canvas + a lightweight immediate-mode UI layer for panels, with Wasmtime components for external plugins.

### Fast path

`winit` + `egui` + `wgpu` can validate the analytical engine faster, but may defer native document, menu, accessibility, and macOS integration work.

### Exploratory path

A newer retained Rust UI/GPU framework may reduce application code, but should not own the domain, source, scheduler, provenance, or analyzer boundaries until its stability and accessibility are proven.
