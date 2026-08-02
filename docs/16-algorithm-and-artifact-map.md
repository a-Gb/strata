# Algorithm and artifact map

This document maps analytical views to computational shape, output artifact, provenance requirements, and likely execution path. Complexity is expressed against selected input length `N`, region count `R`, lag limit `L`, and active sparse states `K`.

| Analyzer/view | Core computation | Nominal complexity | Canonical artifact | Initial path | Exactness notes |
|---|---|---:|---|---|---|
| Byte class | Per-byte classification | `O(N)` | Categorical tile pyramid | GPU + CPU ref | Exact except sampled overview |
| Raw byte map | Per-byte scalar projection | `O(N)` | Scalar/categorical tiles | GPU + CPU ref | Layout mapping must be exact |
| Hilbert/Morton layout | Integer offset↔coordinate mapping | `O(N)` generation; `O(1)` pick | Tile plus mapping semantics | CPU tables or GPU mapping | Round-trip property required |
| Byte histogram | 256-bin count | `O(N)` | `[u64;256]` | CPU small / GPU large | Integer exact |
| Block entropy | 256-bin hist per window/block | `O(N)` | Scalar track/tile + raw counts | GPU + CPU ref | Window/edge policy recorded |
| Digram | 65,536-bin pair count | `O(N)` | Dense `u64` matrix | GPU + CPU ref | Integer exact; stride explicit |
| Positional digram | Pair count plus offset moments or region bins | `O(N)` | Matrix + moments/slices | GPU | Position encoding may be aggregate |
| Conditional digram | Normalize rows/columns | `O(65,536)` | Float matrix + source counts | GPU/CPU | Raw counts retained |
| Layered digram | Region-local pair counts | `O(N)`, memory up to `O(R·65,536)` | Dense/sparse matrix stack | GPU | Dense/sparse policy recorded |
| Trigram | Pack/count 24-bit keys | `O(N)` expected; sort `O(N log N)` or radix | Sparse keys/counts/sketch | GPU spike + CPU ref | Sketch/top-K is approximate |
| Bit planes | Extract bits | `O(N)` | Binary/scalar tiles | GPU + CPU ref | Exact mapping |
| Word reinterpretation | Gather endian words | `O(N)` | Typed scalar field | GPU/CPU | Alignment/truncation explicit |
| Width sweep | Re-layout and score candidate widths | `O(N·W)` naïve | Candidate scores + small multiples | GPU/CPU hybrid | Ranking is heuristic |
| Run-length | Scan equal-value runs | `O(N)` | Run list + distributions | CPU/GPU scan | Exact |
| Rolling hash | Window hash and index | `O(N)` | Hash track/index | CPU or GPU | Collision handling explicit |
| Similarity anchors | Match indexed rolling hashes | `O(N)` expected | Source-A↔B anchors | CPU | Proposed alignment, not proof |
| Exact delta | Compare aligned bytes | `O(N)` | Delta tile + equality runs | GPU + CPU ref | Exact under supplied alignment |
| Autocorrelation | Direct `O(N·L)` or FFT `O(N log N)` | Variable | Lag scores | CPU/GPU spike | Numeric tolerance, range-limited |
| Recurrence plot | Pairwise similarity | `O(N²)` | Tiled matrix | GPU, sampled/tiled | Usually sampled or selected-range only |
| Change-point detection | Statistic/model over tracks | `O(N)`–`O(N log N)` | Ranked boundaries | CPU | Heuristic with evidence features |
| Strings | Encoding state machine | `O(N)` | Region findings | CPU/WASM | Candidate policy explicit |
| Signatures | Fixed-offset probes + rare-byte anchored embedded index | `O(R + N·K)` bounded | Candidate regions + pack digest | CPU | Declared versus relaxed offset explicit; weak/repetitive patterns cannot flood embedded results |
| Bounded decompression | Decoder-specific | Input/output bounded | Derived byte stream + mapping | Isolated CPU/WASM | Exact decoder output, resource-truncated possible |
| Region similarity | Feature extraction + nearest-neighbor index | `O(N)` features; index-dependent | Region graph / neighbor list | CPU + optional GPU | Feature/model/index version recorded |
| ML reranking | Model inference | Model-dependent | Score annotation | Optional local | Never sole evidence |

## Artifact families

### Tile pyramid

```text
TileArtifact {
  source + generation
  transform/analyzer semantics
  level + coordinate
  source coverage
  scalar/categorical payload
  reducer metadata
  exactness + completeness
  provenance root
}
```

Used by atlases, local statistics, recurrence, delta, and semantic masks.

### Dense matrix

```text
MatrixArtifact<T> {
  width, height
  row/column domain descriptors
  raw values
  normalization-independent counts where possible
  aggregate occurrence policy
}
```

Used by histograms, digrams, conditional distributions, and projections.

### Sparse field

```text
SparseFieldArtifact<Key, Value> {
  key domain
  sorted or indexed entries
  residual/omitted mass
  top-K/sketch policy
  optional source-region moments
}
```

Used by trigrams, transition graphs, and sparse volumes.

### Offset track

```text
TrackArtifact<T> {
  windows/ranges
  value per window
  window and edge policy
  pyramid reducers
}
```

Used by entropy, novelty, density, change points, and alignment signals.

### Region findings

```text
FindingArtifact {
  exact/approximate range set
  kind + label
  attributes
  confidence basis
  supporting artifact IDs
}
```

Used by strings, signatures, parsers, carved candidates, and anomaly ranking.

### Occurrence index

Digrams, signatures, repeated hashes, and search results may materialize occurrence indexes on demand. An overview artifact need not retain every offset. The view must expose whether occurrence navigation is unavailable, sampled, paginated, or exact.

## Planner heuristics

The planner considers:

```text
input length
visible/selected/full domain
required exactness
artifact already cached
GPU support and pressure
CPU/GPU transfer cost
output size
contention pattern
occurrence-index requirement
plugin/source capability
cancellation likelihood
```

Small jobs should remain CPU-side when dispatch/upload overhead is greater than useful work. Full-source/high-bandwidth jobs become GPU candidates. The choice is observable and does not change analyzer semantics.

## Numerical policy

- Raw counts use integers and exact comparison.
- Entropy and normalized probabilities preserve raw counts and declare log base, zero policy, and precision.
- FFT/autocorrelation outputs declare normalization and tolerance.
- Floating-point scene coordinates are not used as source offsets.
- Large offsets remain `u64` or split integer values through picking and provenance paths.
- Any downcast or bin saturation is an error or an explicit bounded approximation.
