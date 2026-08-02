# ADR 0002: Immutable source snapshots and provenance-first artifacts

- Status: Accepted for scaffold
- Scope: source and evidence model

## Context

Visual projections are lossy and can be misleading. Live or externally modified sources can also make results stale. An analyst must be able to trace every visual feature to the source generation, ranges, transformations, parameters, and sampling that produced it.

## Decision

Treat opened inputs as immutable `SourceSnapshot`s. Live/mutable connectors produce explicit generations. Every result is immutable and carries a provenance token. Every pickable visual feature resolves to source ranges or declares aggregate/approximate semantics.

## Consequences

- Source editing is not part of the product.
- Cache keys and session records are larger but reliable.
- Background results require generation checks before publication.
- Reproducibility and source-free session sharing become possible.

## Rejected

- Mutable in-place byte model: conflates analysis, editing, and evidence history.
- Screenshot-only exports: visually useful but analytically unauditable.
- View-local offsets: cause inconsistent navigation across projections.
