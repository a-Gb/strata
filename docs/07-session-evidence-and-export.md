# Session, evidence, cache, and export

## Session bundle

A `.strata-session` is a directory bundle during development and may become a deterministic ZIP container for interchange.

```text
example.strata-session/
├── manifest.json
├── session.sqlite
├── journal.ndjson
├── evidence/
│   ├── records.json
│   └── snapshots/
├── presets/
├── plugin-state/
├── artifacts/              # optional, derived and content-addressed
└── sources/                # references only by default
    └── source-<id>.json
```

## Default privacy behavior

The bundle stores:

- source name or redacted alias;
- length, digest, timestamps, and address mappings;
- authorized bookmark/reference only when explicitly requested;
- selections, annotations, transforms, view state, analysis specifications;
- derived artifacts only when configured;
- no source bytes by default.

A “portable evidence bundle” may include selected source ranges, but the export dialog must enumerate byte counts, ranges, and sensitivity warnings before writing.

## Manifest

The manifest contains:

```text
schema_version
application_semantics_version
created_at / modified_at
source references and digest state
session database digest
journal digest
artifact inventory
enabled plugin IDs and versions
redaction policy
required capabilities
```

## Journal and undo

User intent is represented as append-only session events. Periodic snapshots accelerate load. Undo creates inverse events or moves the visible head; it does not rewrite historical evidence silently.

Evidence records are immutable after sealing. Corrections append a superseding record with rationale.

## Evidence record

An evidence record captures:

- analyst-authored claim;
- status: hypothesis, corroborated, rejected, or informational;
- confidence and rationale;
- named selections;
- source and generation identities;
- exact view/preset state;
- supporting analysis artifacts;
- screenshots or vector snapshots;
- provenance DAG root;
- creation and supersession metadata.

Evidence is not automatically created from every analyzer finding. The analyst explicitly promotes findings, retaining the human-in-the-loop distinction.

## Cache tiers

| Tier | Contents | Policy |
|---|---|---|
| Frame/transient | command buffers, picking targets, staging | Destroyed/reused rapidly |
| Session RAM | visible tiles, occurrence indexes, active analyses | Budgeted LRU with priority |
| Persistent derived cache | recomputable tiles/statistics keyed by source hash | User-configurable size and retention |
| Session artifacts | pinned results needed for reproducibility | Included only by policy |

Raw source chunks are not persisted in the derived cache unless a connector requires it and the user enables it.

## Export types

### Analytical image

- PNG/TIFF for raster views;
- SVG/PDF-like vector representation where the view is vector-compatible;
- legend, scale, source range, sampling, and provenance sidecar;
- optional embedded metadata with a stable record ID.

### Data

- JSON/NDJSON for findings and provenance;
- CSV/Arrow for matrices, tracks, and occurrence lists;
- raw selected-range extraction;
- NumPy-compatible arrays as a later exporter;
- glTF for 3D point/mesh views, accompanied by source mapping metadata.

### Reproducible preset

A compact JSON object that can be run by CLI against a matching source. It names transforms, analyzers, view state, export parameters, and exactness requirements.

### Report

A Markdown/HTML report generated from evidence frames. Report generation must not silently upgrade hypotheses to conclusions. Each frame links to a record ID and source digest.

### External bridge

A local message containing source identity, address space, ranges, labels, and optional selected bytes. Bytes are excluded unless the receiving action explicitly requests and the user authorizes them.

## Atomicity and recovery

- Session writes use transactions.
- Large artifacts are written content-addressed, fsynced where appropriate, then referenced transactionally.
- Exports write to temporary files and rename atomically.
- On load, manifest and journal digests are checked.
- A damaged bundle can recover the last valid database snapshot and journal prefix.
- Unknown plugin state is preserved as opaque versioned blobs but not executed.

## Redaction

Before sharing, the user can:

- replace source names and paths with aliases;
- strip security-scoped references;
- remove analyst identity;
- exclude screenshots containing raw strings/bytes;
- include only selected evidence records;
- retain digests while omitting timestamps;
- inspect a final bundle inventory.
