# Performance and resource budgets

These are **design targets**, not measured claims. They exist to make architectural tradeoffs testable.

## Release artifact budgets

| Artifact | Gate |
|---|---:|
| Stripped Apple Silicon executable | ≤20 MiB |
| Compressed local DMG | ≤25 MiB |
| Headless help startup | Report, do not regress silently |

The packaging verifier also rejects a wrong architecture, a mismatch between
the plist and Mach-O deployment targets, and leaked local `/Users` or
`/var/folders` paths. On the development host on 2026-08-02, the first `dist`
build produced a 10,269,872-byte executable and an approximately 5.05 MB DMG.
Headless help averaged 6.6 ms and animation-program validation 6.5 ms across 20 runs.
These figures are a local measurement snapshot, not floor-device acceptance.

## Reference hardware tiers

| Tier | Baseline intent |
|---|---|
| Floor | M1-class Apple Silicon, 8 GB unified memory |
| Primary | M1/M2/M3-class, 16–24 GB unified memory |
| High | Pro/Max/Ultra-class systems with larger memory and GPU |

The feature set must degrade by resolution and cache retention, not by correctness of exact selected-range analysis.

## Latency classes

| Class | Target | Examples |
|---|---:|---|
| Frame-critical | ≤16.7 ms p95 at 60 Hz | pan, zoom, selection overlay, cached tile draw |
| Immediate | ≤50 ms p95 | exact analytical picking, inspector update from cached bytes |
| Interactive preview | ≤250 ms | first coarse visible tile, small-range histogram/digram |
| Progressive response | ≤1 s to first useful overview | opening typical local files and large sampled sources |
| Refinement | visible incremental progress | exact entropy/digram tiles, occurrence indexing |
| Background | no fixed latency | full-source exact analyses, corpus indexing, exports |

No background class is allowed to block frame-critical work.

## Source-size behavior

| Source scale | Required behavior |
|---|---|
| <1 MiB | Exact overview should usually be cheaper than sampling |
| 1 MiB–1 GiB | Progressive coarse-to-exact tiles; full digram feasible under budget |
| 1–100 GiB | Overview through range reads/sampling; exact visible/selected regions; queued full scans |
| >100 GiB / sparse | Metadata-first, explicit sampling, virtual tile pyramid, no whole-source allocation |
| Unbounded stream | Retention window and generation model; bounded indexes |

The executable workbench fixes each large-source projection working set at 64 ×
256 KiB = 16 MiB, plus a 1 MiB exact prefix used by legacy contiguous views.
Focus refinement and matched A/B tile comparison run off the UI thread; their
artifacts distinguish logical coverage from exact resident read ranges.
Whole-source SHA-256 advances in bounded background steps for source-free session
save and reattachment. Exact large-source video export remains range-gated.

## Frame budget

Suggested 60 Hz budget allocation under interaction:

```text
input/state reduction       1.0 ms
scene update                2.0 ms
uploads/compute             3.0 ms
render passes               6.0 ms
present/system margin       4.7 ms
```

The engine should skip nonessential scene recompilation and defer labels/3D refinements when the frame budget is threatened.

## Memory policy targets

### Application working set

Default total cache budget:

```text
max(768 MiB, min(4 GiB, 20% of physical memory))
```

This is a starting policy to validate, not a promise that reported free memory is safely allocatable.

Suggested sub-budgets:

| Pool | Share | Eviction behavior |
|---|---:|---|
| GPU-visible artifacts | 35% | Drop offscreen fine levels first |
| CPU analysis artifacts | 25% | Drop recreatable occurrence/detail indexes |
| Source windows/staging | 20% | Reuse fixed pools; release low-priority windows |
| Render/transient | 10% | Hard ceiling; quality degrade before growth |
| Session/UI/plugin overhead | 10% | Per-plugin quotas and alerts |

A user may raise budgets, but the app must surface the tradeoff and preserve an emergency reserve.

## Persistent cache

Default policy proposal:

- disabled or small on first launch until user chooses retention;
- size cap rather than age alone;
- content-addressed and safe to delete at any time;
- no source bytes by default;
- per-source purge and complete purge commands;
- cache version bump invalidates semantics-incompatible artifacts.

## Analysis throughput metrics

Benchmarks report:

```text
source bytes/s
output bins/points/tiles
read amplification
CPU time
GPU time
wall time
peak CPU memory
peak GPU-visible memory
energy impact where measurable
cache hit state
sampling/exactness
```

A single “files per second” score is not meaningful across source size and view family.

## Quality degradation order

Under pressure:

1. Stop offscreen prefetch.
2. Cancel superseded/low-priority analysis.
3. Evict offscreen fine tiles.
4. Reduce label density and decorative effects.
5. Reduce 3D point count/volume resolution.
6. Use coarser visible tiles while preserving exact selection inspection.
7. Move eligible compute to CPU if GPU allocation is the failure source.
8. Disable the expensive view with a recoverable message.

Never silently change analyzer parameters, sampling policy, or normalization without updating visible state and provenance.

## Performance acceptance gates

### Gate A — interaction

- Cached pan/zoom remains within frame target on the floor device.
- Hover and click do not synchronously scan source ranges.
- Opening a source never blocks the main thread on hashing.

### Gate B — progressive overview

- A coarse byte/entropy overview appears before full-source completion.
- Zooming prioritizes visible tiles.
- Cancelling or changing parameters prevents stale publication.

### Gate C — exactness

- Selected-range exact analysis remains available even when full-source analysis is sampled.
- CPU/GPU artifacts match defined semantics.
- Exact export refuses incomplete/sampled dependencies unless user chooses illustrative export.

### Gate D — pressure

- Artificial memory pressure triggers documented degradation, not process termination.
- GPU device loss preserves session state and source integrity.
- A pathological plugin cannot consume unbounded memory or scheduler time.
