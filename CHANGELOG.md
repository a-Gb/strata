# Changelog

All notable user-facing changes are recorded here.

## [Unreleased]

No user-facing changes yet.

## [0.1.0] - 2026-08-02

### Added

- Executable macOS workbench and headless CLI over a shared bounded runtime.
- Read-only local sources, large-file tile/LOD planning, matched tiled
  comparison, and progressive whole-source SHA-256.
- Linked discovery, structure, grammar, resonance, comparison, and composable
  3D projection views with exact or disclosed sampled source ranges.
- Deterministic CPU analysis paths plus WGPU Alignment and Hamming coordinate
  kernels guarded by differential tests and CPU fallback.
- Source-free session bundles, digest-gated reattachment, local project
  locators, and persisted launch preferences.
- Strict UFSC signature-pack import, embedded candidate search, visual evidence
  overlays, and exact match provenance.
- Deterministic animation programs, synthetic correlated inputs, H.264 export,
  and JSON provenance sidecars.
- Dual MIT/Apache-2.0 licensing, CC0 synthetic-fixture terms, contribution and
  conduct guidance, release checklist, and maintained implementation status.
- Repeatable Apple Silicon `dist` app and DMG packaging with deployment-target,
  signature, architecture, path-leak, mounted-image, and headless smoke gates;
  plus opt-in Developer ID signing, notarization, and stapling commands.
- A reviewed multi-resolution macOS icon with a documented generated-image
  master and repeatable ICNS conversion.
- A `0.1.0` candidate signed with the hardened runtime and Developer ID team
  `2NK7ZR2DY7`, accepted by Apple's notary service, stapled, and accepted by
  Gatekeeper.

### Changed

- Split the application, projection, animation, runtime, and analysis code into
  bounded modules; maintained files are capped at 1,200 lines.
- Reclassified projection, geometry, visual channel, overlay, and comparison
  concepts into orthogonal contracts.
- Made generated renderer output local-only instead of versioned source.
- Reduced the packaged executable with thin LTO, single-unit code generation,
  panic aborts, and symbol stripping while leaving the faster iterative
  release profile unchanged.
- Made the macOS icon filename explicit and advanced the bundle build number so
  LaunchServices cannot prefer an obsolete iconless development bundle.

## Initial architecture snapshot

- Defined the product model, crate boundaries, runtime pipeline, view catalog,
  plugin tiers, provenance invariant, threat model, performance targets, and
  staged roadmap.
- Added WIT and JSON contracts, early WGSL pass specifications, and Mermaid
  architecture diagrams.

[Unreleased]: https://github.com/a-Gb/strata/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/a-Gb/strata/releases/tag/v0.1.0
