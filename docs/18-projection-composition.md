# Projection composition contract

The 3D lab composes independent analytical decisions instead of treating every visual result as a peer “mode”:

```text
source bytes -> sample domain -> projection -> geometry -> visual channels -> overlays
```

The implemented POC state is versioned by [`projection-composition.schema.json`](../schemas/projection-composition.schema.json). Its six basic projections are Address Raster, Hilbert Plane/Cube, Transition Field, Bit-Plane Stack, Complexity Phase, and Section Prism. The P1 analytical catalog adds Alignment Lattice, Recurrence Plane, Repetition Skyline, Spectral Waterfall, Hamming Hypercube, and Hierarchical Block Volume. Polar and helical address paths remain available as advanced address submodes; surface is a geometry, not a projection.

P1 controls are projection-sensitive. Alignment exposes stride ranking; recurrence and repetition disclose window, bounded prior search, candidate budget, threshold, exact partner range, and match length; spectrum discloses its DFT window and bin ceiling; hierarchy discloses depth, minimum block, and split threshold. Hamming uses one fixed basis so positions remain comparable across files.

## Comparison semantics

- **Single** renders projection A.
- **Split** is the analytical default and gives A and B separate viewports.
- **Overlay** draws both coordinate systems while preserving linked source identity.
- **Morph** interpolates coordinates for orientation and presentation only. It is not evidence of semantic continuity.

Each reusable sample retains `point_id`, three representative contributor offsets, and the exact half-open range used for feature calculation. Split, overlay, morph, and the eight bit-plane instances preserve that identity. Picking selects the exact analyzed range; cohort selection deduplicates repeated visual instances and coalesces their exact source ranges.

## Evidence boundaries

- Address, Hilbert, transition, and bit-plane coordinates are deterministic raw mappings.
- Complexity features are heuristic statistics. Entropy alone does not distinguish compression from encryption.
- Section Prism uses exact living/parser regions when available and visibly falls back to deterministic address blocks otherwise.
- The same fixed projection basis and parameters must be used for cross-file comparison.
- Every 3D configuration retains a 2D setting or split view because occlusion is not an analytical result.
- A sampled large-source datum identifies both its exact resident read range and the larger logical coverage represented by that tile. Clicking it queues bounded level-zero focus tiles; the sampled overview is never described as complete analysis.

## Runtime and compatibility

Sessions persist the composition without source bytes. Legacy session fields remain readable and map Trigram to Transition Field, Orbit/Helix to advanced address paths, and Terrain to Hilbert plus Surface. Animation programs may embed the same composition; older v1 programs without it retain the legacy four-stop renderer.

The POC keeps at most 64 source tiles / 16 MiB resident for a large-file projection plan. Tile keys bind source identity, generation, semantic version, level, coordinate, exactness, and parameter digest. Alignment and Hamming coordinates use a WGPU compute kernel only after a startup CPU/GPU differential passes; recurrence, DFT, and hierarchy retain bounded CPU-reference semantics. Any GPU failure is visible and falls back before publication.
