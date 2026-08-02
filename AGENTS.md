# Repository Guidelines

## Project Structure & Module Organization

Strata is a pre-alpha executable workbench with a larger target architecture. Rust workspace crates live in `crates/`; keep domain contracts in `strata-core` and preserve the dependency boundaries described in each crate README. Product and architecture specifications are in `docs/`, with current truth in `docs/20-implementation-status.md`, decisions in `docs/adr/`, and Mermaid sources in `docs/diagrams/`. Implemented and planned GPU passes live in `shaders/` and must be labelled accurately. Versioned JSON contracts belong in `schemas/`, component interfaces in `wit/`, and first-party or example package specifications in `plugins/`. Test data belongs in `fixtures/` and must follow `fixtures/README.md`.

## Build, Test, and Development Commands

The pinned stable toolchain includes `rustfmt` and `clippy`. Prefer the repository recipes:

- `just check` runs `cargo check --workspace --all-targets`.
- `just lint` verifies formatting and treats every Clippy warning as an error.
- `just test` runs the complete Rust test suite.
- `cargo fmt --all` formats Rust sources before review.
- `just sbom` generates a CycloneDX SBOM once `cargo-cyclonedx` is installed.

The macOS workbench, CLI, bounded runtime, deterministic analyzers, selected WGPU kernels, sessions, and exports have executable paths. Do not describe contract-only crates, planned shaders, plugin execution, native lifecycle, or distribution hardening as complete without adding and verifying their runtime path.

## Coding Style & Naming Conventions

Use standard `rustfmt` output and four-space indentation. Name modules, functions, and files in `snake_case`; types and traits in `UpperCamelCase`; constants in `SCREAMING_SNAKE_CASE`; and crates with the `strata-` prefix. Workspace policy denies unsafe code and `unwrap`, `expect`, and `panic`; return typed errors instead. Document public APIs and keep exports reachable. Preserve deterministic results, checked byte arithmetic, immutable-source semantics, and provenance for every derived artifact.

## Testing Guidelines

Use Rust unit tests beside the code under `#[cfg(test)]` and crate-level integration tests under `crates/<crate>/tests/`. No numeric coverage threshold is defined. Prioritize range/provenance properties, deterministic fixtures, CPU/GPU differential checks, and semantic goldens specified in `docs/09-observability-testing.md`. Record fixture digests, licenses, and expected properties; never commit proprietary or live-malware samples.

## Commit & Pull Request Guidelines

Use concise imperative subjects, optionally scoped, such as `gpu: validate atlas dimensions`. Pull requests should explain intent and architectural impact, link relevant issues or ADRs, list verification commands, and call out schema, security, or provenance changes. Include screenshots for visible UI/rendering changes and update affected docs or contracts in the same change.
