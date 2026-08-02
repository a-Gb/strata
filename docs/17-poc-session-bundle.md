# Bounded POC session bundle

The executable POC persists a deliberately small, source-free session bundle. It is the current implementation contract for `strata-session`; it does not supersede the broader future-session architecture in [07-session-evidence-and-export.md](07-session-evidence-and-export.md).

## Directory layout

```text
investigation.strata-session/
├── manifest.json
└── journal.ndjson
```

Both files are replaced via a temporary file in the same directory, `sync_all`, and rename. A process interruption between the two replacements can leave a mixed pair, but loading rejects it through the journal digest rather than silently accepting it.

No source file, payload bytes, path, URL, or bookmark is written by this format. Exact byte offsets may be persisted as investigation metadata; offsets identify positions but do not contain source payload.

## `manifest.json`

The versioned contract is [session-bundle.schema.json](../schemas/session-bundle.schema.json). Version 1 has six required fields:

| Field | Meaning |
|---|---|
| `schema` | Constant `strata-session-bundle` |
| `version` | Constant `1` |
| `source` | Alias, byte length, and SHA-256 only |
| `workspace` | Opaque JSON owned by the workbench |
| `journal_sha256` | SHA-256 of the exact `journal.ndjson` bytes |
| `journal_event_count` | Expected number of journal lines/events |

`source.alias` is a display label, not a locator; it must not contain `/` or `\`. The digest fields are lowercase 64-character SHA-256 hex. Serializing an identical in-memory bundle produces deterministic manifest and journal bytes.

The generic bundle layer deliberately treats `workspace` as opaque JSON and therefore cannot prove that a caller's workspace payload is source-free. That guarantee belongs to the POC's typed workspace decoder, which uses `deny_unknown_fields` to admit only its explicit source-free contract.

## `journal.ndjson`

Each non-empty line is one JSON object:

```json
{"sequence":0,"event":{"type":"view_changed","payload":{"view":"regions"}}}
```

The `event.type` variants are `workspace_changed`, `view_changed`, `selection_changed`, `hypothesis_applied`, and `annotation_added`. The payload is opaque JSON except for `workspace_changed`, whose payload is the opaque workspace JSON directly.

The journal is append-only and sequence numbers must be contiguous and zero-based. Non-empty journals end in a newline; the empty journal is zero bytes. The runtime validates schema/version, digest, event count, JSON shape, and ordering before returning a loaded bundle. The JSON Schema describes the manifest only; it cannot validate the digest or cross-file ordering.

## Reattachment boundary

Loading restores no source handle and reads no external source. Reattachment is an explicit caller action: candidate bytes are hashed in memory and compared to the saved byte length and SHA-256. The typed result is either `Match` or `Mismatch` with only expected/actual lengths and digests. Candidate bytes are never added to the bundle as a consequence of that check.

This makes a bundle portable as investigation state while requiring the user or host to deliberately provide a matching source again.
