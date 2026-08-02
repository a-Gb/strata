# CLI and local bridge contract

## CLI role

The CLI is a deterministic client of the same source, transform, analysis, provenance, session, and export services as the GUI. It is not a separate analytical implementation.

Working binary name: `strata`.

## Proposed command tree

```text
strata
├── source
│   ├── info <path|connector>
│   ├── hash <path|connector>
│   └── ranges <path|connector>
├── analyze <source>
│   ├── --preset <preset.json>
│   ├── --analyzer <id>
│   ├── --range <start:end>
│   ├── --sampling <policy>
│   ├── --exact
│   └── --output <result.json|artifact-dir>
├── render <source>
│   ├── --preset <preset.json>
│   ├── --format png|tiff|svg|gltf
│   ├── --require-exact
│   └── --sidecar <provenance.json>
├── session
│   ├── create
│   ├── validate <bundle>
│   ├── reattach <bundle> <source>
│   ├── redact <bundle>
│   └── inventory <bundle>
├── plugin
│   ├── inspect <bundle>
│   ├── verify <bundle>
│   └── invoke <plugin> <operation>
├── cache
│   ├── stats
│   ├── verify
│   ├── purge --source <digest>
│   └── purge --all
└── bridge
    ├── serve
    ├── status
    └── token rotate
```

## Machine-readable output

All commands support `--output-format json`. The envelope is stable and line-oriented commands may use NDJSON for progress.

```json
{
  "schema_version": "0.1.0",
  "request_id": "...",
  "status": "complete",
  "result": {},
  "warnings": [],
  "provenance_roots": [],
  "metrics": {
    "wall_ms": 0,
    "bytes_read": 0,
    "cache": "miss"
  }
}
```

Progress event:

```json
{
  "type": "progress",
  "request_id": "...",
  "phase": "analyze.digram",
  "completeness": "partial",
  "covered_bytes": 1048576,
  "total_bytes": 16777216,
  "exactness": "exact"
}
```

## Exit codes

| Code | Meaning |
|---:|---|
| 0 | Completed successfully |
| 2 | Invalid command or schema |
| 3 | Source unavailable or authorization failed |
| 4 | Source digest/generation mismatch |
| 5 | Exactness/completeness requirement not met |
| 6 | Resource limit, cancellation, or GPU unavailable with no allowed fallback |
| 7 | Plugin verification, capability, or execution failure |
| 8 | Session/export/cache transaction failure |
| 9 | Internal invariant violation |

A warning never changes exit code unless the caller requested `--warnings-as-errors`.

## Idempotency

- Analysis is idempotent by semantic cache key.
- `session create` accepts `--idempotency-key`.
- Exports do not overwrite by default; `--replace` uses temporary write plus atomic rename.
- Bridge mutation methods require an idempotency key and return the prior result on replay.
- Plugin invocation declares whether it is pure, seeded, or side-effecting.

## Local bridge purpose

The bridge synchronizes ranges and annotations with Ghidra, rizin, LLDB, or custom tools. It is disabled by default and does not provide arbitrary shell execution.

## Transport and authentication

Recommended initial transport:

- Unix domain socket inside the user’s application support/runtime directory;
- per-session bearer token with short display fingerprint;
- optional OS peer-credential check;
- explicit client approval and source binding;
- local-only; no TCP listener unless separately designed.

## Handshake

```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "bridge.hello",
  "params": {
    "protocol_version": "0.1.0",
    "client": { "name": "ghidra-strata", "version": "0.1.0" },
    "token": "REDACTED",
    "requested_capabilities": ["source.list", "navigate", "selection.publish", "annotation.write"]
  }
}
```

Response returns granted capabilities, session ID, source identities, address spaces, and limits.

## Methods

| Method | Direction | Purpose |
|---|---|---|
| `source.list` | Client→host | List session source IDs, hashes, generations, address spaces |
| `source.attach` | Client→host | Bind a client-side program/image to a matching source digest |
| `navigate.request` | Either | Ask peer to navigate to a range/address |
| `selection.publish` | Either | Share exact or aggregate selection metadata |
| `annotation.upsert` | Either | Create/update a named range annotation idempotently |
| `annotation.delete` | Either | Delete non-sealed annotation with idempotency key |
| `evidence.reference` | Host→client | Share sealed evidence metadata; no mutation |
| `range.read.request` | Client→host | Request bytes only under explicit granted capability |
| `analysis.request` | Client→host | Request a host analyzer for bounded ranges |
| `status.subscribe` | Client→host | Receive source-generation and session events |

## Range message

```json
{
  "source_id": "...",
  "generation": 0,
  "address_space": "file-offset",
  "ranges": [{ "start": 4096, "end": 4352 }],
  "coverage": "exact_contiguous",
  "label": "suspected table",
  "provenance_roots": ["..."]
}
```

## Error model

JSON-RPC application errors include:

```text
SOURCE_NOT_ATTACHED
SOURCE_GENERATION_STALE
ADDRESS_MAPPING_UNAVAILABLE
CAPABILITY_DENIED
RANGE_LIMIT_EXCEEDED
BYTES_REQUIRE_CONFIRMATION
ANALYZER_UNAVAILABLE
INVALID_PROVENANCE
CONFLICT
RATE_LIMITED
INTERNAL
```

Errors include `retryable`, a safe message, and structured detail. They never include source bytes or paths unless the client has the corresponding grant.

## Bridge safety

- Byte transfer is a separate capability from navigation/annotations.
- A client cannot select a source by path alone; digest/identity binding is required.
- Sealed evidence cannot be overwritten through the bridge.
- Every transferred byte count and mutation is logged locally.
- Client disconnect revokes ephemeral handles.
- Source generation changes invalidate prior handles and notify subscribers.
