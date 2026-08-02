# Implementation status

Strata is a publicly available pre-alpha analytical workbench with an
executable macOS host. It is not a production security tool, malware classifier,
or supported forensic product. This document is the maintained boundary between
working code and planned architecture.

## Working end to end

- Read-only local file opening with bounded reads, immutable generations, and
  progressive whole-source SHA-256.
- A shared bounded runtime used by the desktop host and headless CLI.
- Deterministic structure, entropy, transition, correlation, repetition,
  periodicity, string, signature, projection, and comparison analyses.
- Linked 2D and 3D views with stable source identifiers, exact or disclosed
  sampled ranges, camera controls, persistent selection, and exact picking.
- WGPU compute for the Alignment and Hamming projection coordinates, guarded by
  CPU/GPU differential tests and an explicit CPU fallback.
- Device-limit-aware texture tiling for tall exact raster views.
- Source-free deterministic session bundles with integrity checks and
  digest-gated reattachment.
- Local `.strata-project` locators for reopening local paths and UI state. These
  are private machine-local files and are excluded from version control.
- Strict UFSC `0.1.x` signature-pack import with explicit rejection accounting,
  candidate-evidence semantics, source attribution, and pack digests.
- Deterministic animation programs, synthetic fixtures, H.264 export, and JSON
  provenance sidecars.
- Optimized Apple Silicon app and compressed DMG generation with architecture,
  deployment-target, signature, private-path, checksum, mounted-image, and
  headless smoke verification.

## Partial or experimental

- The current UI is an `eframe`/`egui` workbench hosted by a thin macOS process;
  native AppKit document lifecycle, menus, sandbox authorization, and
  accessibility acceptance remain incomplete.
- Only a bounded subset of planned GPU analyses is dispatched through WGPU.
  Search, recurrence, DFT, hierarchy, and video rendering retain deterministic
  CPU implementations.
- Plugin crates define capability and scene contracts, but third-party WASM
  component installation and execution are not enabled.
- Integrity-checked sessions detect accidental or unsophisticated edits; they
  are not signed and do not establish adversarial authenticity.
- The default bundle remains ad-hoc and suitable for local testing only.
  The credential-gated `0.1.0` candidate passed hardened Developer ID signing,
  mounted-DMG verification, real Metal differential, native GUI smoke, Apple
  notarization, stapling, and Gatekeeper assessment. A quarantined local copy
  also passed DMG assessment, copied-app assessment, and first-launch smoke;
  separate-machine installation and update delivery remain incomplete.
- The complete cross-platform lock graph still reaches `quick-xml 0.38.4`
  through Linux-only AccessKit/AT-SPI dependencies. That path is affected by
  RUSTSEC-2026-0194 and RUSTSEC-2026-0195, but is absent from the supported
  `aarch64-apple-darwin` graph. It must be upgraded before Linux is added to the
  support matrix.

## Supported development target

- Apple Silicon.
- macOS 15 or newer.
- Stable Rust 1.85 or newer as constrained by the workspace MSRV.
- WGPU over Metal; CPU fallback remains part of the analytical contract.

Other hosts may compile individual backend-neutral crates, but they are not
part of the current acceptance matrix.

## Acceptance gates

```bash
just check
just lint
just test
just validate-video-gallery
just dmg
just verify-macos-gpu
just smoke-macos-gui
```

Run `just advisories-update` before a release audit. `just release-check` then
runs the target-aware dependency policy and advisory check against that local
snapshot without fetching during the gate. Test counts are intentionally not
recorded here; passing commands are the source of truth.

## Stable-release blockers

- Complete the product-name/trademark check. The direct-distribution bundle
  identifier is fixed as `dev.strata.workbench` under team `2NK7ZR2DY7`.
- Define compatibility promises beyond the current `0.1.0` pre-alpha boundary.
- Pass a quarantined clean-machine install test before distributing a binary
  as a release.
- Replace the current Developer ID certificate with a G2-issued certificate
  before its February 1, 2027 expiration.
- Resolve licensing and alias-preservation gaps in any external signature pack
  before vendoring it.
- Remove generated media from unpublished Git history before the first public
  push; do not rewrite history after publication without coordination.

See [Releasing](RELEASING.md) for the maintainer checklist and
[Roadmap](11-roadmap.md) for planned architecture.
