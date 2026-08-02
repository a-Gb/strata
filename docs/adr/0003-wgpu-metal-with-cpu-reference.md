# ADR 0003: `wgpu`/Metal acceleration with CPU reference semantics

- Status: Proposed
- Scope: compute and rendering

## Context

Byte classification, histograms, entropy blocks, n-grams, tiling, and rendering are parallel workloads. Direct Metal can maximize platform specificity but would couple domain semantics to one backend and enlarge the unsafe/FFI surface.

## Decision

Use `wgpu` with the Metal backend for production compute and rendering. Define core analyzer semantics through CPU reference implementations. Admit a quarantined Metal-specific path only after measured need and with fallback.

## Consequences

- Core semantics can be tested headlessly and differentially.
- Some platform-specific opportunities may be delayed.
- GPU artifacts must record implementation semantics and exactness.
- Shader validation and resource policy can be centralized.

## Rejected as initial architecture

- Direct Metal everywhere: fastest route to lock-in and broad FFI surface.
- CPU-only: simpler but leaves major interactive and visualization throughput unused.
- Metal Performance Shaders as the central API: not a natural fit for the full custom visualization workload.
