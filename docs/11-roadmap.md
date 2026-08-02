# Staged implementation roadmap

This is the target architecture, not a completion ledger. Several vertical
slices now span stages 1–3 while some stage-0 native-host and plugin spikes
remain open. Use [`20-implementation-status.md`](20-implementation-status.md)
for current truth. Future promotion remains gate-driven: do not add view breadth
until the relevant invariants have executable acceptance evidence.

## Stage 0 — architecture spikes

### Spikes

1. **Native host:** AppKit/`objc2` window, file authorization, drag/drop, menu/command integration, accessibility probe.
2. **GPU surface:** `wgpu` Metal surface, one compute pass, one tiled render pass, device-loss simulation.
3. **UI composition:** compare AppKit+`egui`, `winit`+`egui`, and another retained Rust UI candidate for text input, accessibility, docking, and high-DPI behavior.
4. **Source I/O:** bounded random reads and memory-mapped windows over tiny, multi-gigabyte, sparse, and changing files.
5. **Plugin host:** minimal Wasmtime component that requests a range and emits a finding under quotas.
6. **Session:** event journal, source-free save/reopen, digest reattachment.

### Exit gate

Select concrete dependencies and pin versions only after the spikes. Document measured tradeoffs in replacement ADRs.

## Stage 1 — vertical proof

### Scope

- open one local file read-only;
- progressive byte-class atlas using raster and Hilbert layouts;
- entropy atlas;
- synchronized read-only hex inspector;
- shared cursor and exact brush selection;
- CPU and GPU reference/differential tests;
- in-memory cache and basic resource budget;
- provenance for every tile and selection;
- save/reopen source-free session;
- headless CLI preset for the same analyses.

### Exit gate

A user can move from visible feature to exact bytes, reproduce the same artifact through CLI, and survive source mismatch, cancellation, memory pressure, and GPU reset without corrupted state.

## Stage 2 — credible analytical MVP

### Scope

- byte histogram and digram matrix;
- rolling statistic tracks;
- strings and signature analyzers;
- named selections and evidence notebook;
- view linking and workspace presets;
- exact/illustrative export distinction;
- persistent derived cache;
- local structured diagnostics;
- redacted session export;
- first malformed/adversarial corpus.

### Exit gate

The application credibly answers “where?”, “what transition structure?”, and “what exact bytes support this observation?” for unknown binaries.

## Stage 3 — modern differentiators

### Scope

- stride-N and conditional digrams;
- layered digram slices/volume;
- bit-plane, word, width, autocorrelation, run-length, and recurrence views;
- reversible transform branches;
- synchronized dual-source diff and rolling-hash anchors;
- safe parser overlays;
- occurrence indexes and aggregate-cell navigation;
- external WASM analyzer/overlay SDK;
- report frames and reproducible preset export.

### Exit gate

Strata supports real format discovery, version comparison, interleaving/record hypotheses, and third-party analysis without sacrificing provenance or host stability.

## Stage 4 — reverse-engineering ecosystem

### Scope

- Ghidra/rizin/LLDB bridge;
- executable sections, symbols, relocation overlays;
- embedded-object carving and bounded decompression probes;
- parser object graph;
- helper-process isolation for selected native analyzers;
- signed plugin bundles and update policy;
- corpus fingerprint index using deterministic features.

### Exit gate

A visual finding can be escalated into established RE tools and returned as annotations, with source/address identity preserved.

## Stage 5 — advanced and expressive research

### Scope

- sparse trigram cloud and projections;
- transition graphs and Markov residuals;
- live stream sources;
- mutation-stability laboratory;
- optional local ML similarity/anomaly ranking;
- sonification and presentation/story mode;
- trusted out-of-process native extensions if a demonstrated need remains.

### Exit gate

Advanced modes add genuine discovery or communication value and remain reproducible, accessible, resource-bounded, and optional.

## Deliberately deferred

- source editing;
- cloud analysis/service account model;
- arbitrary native third-party dynamic libraries;
- automatic malware verdicts;
- process execution/emulation;
- Metal-specific semantic implementation without a portable reference;
- broad cross-platform UI before macOS interaction and performance are stable.

## De-risk experiments

| Risk | Smallest useful experiment | Pass condition |
|---|---|---|
| Hilbert picking correctness | Exhaustive small-order mapping round trip | Zero mismatches and explicit aggregate semantics |
| GPU histogram contention | Compare atomic, partitioned, and sort/reduce digrams | Stable counts; viable floor-device throughput/memory |
| Large-file responsiveness | Synthetic virtual 1 TiB source with bounded reader | Overview and selection without whole-source allocation |
| AppKit/UI boundary | Docking, IME, accessibility, drag/drop prototype | Native behaviors work without domain leakage |
| WASM plugin overhead | Range-read + finding benchmark under quota | Overhead acceptable for external analyzers |
| 3D utility | Analyst tasks with 2D versus 3D layered digram | Keep only if task success improves |
| Cache correctness | Parameter mutation and semantics-version tests | No false hits; safe invalidation |
| Adversarial visuals | Padding/reordering/encoding perturbation suite | Tool surfaces fragility and conflicting evidence |
