# strata-session

Source-free, integrity-checked persistence for the bounded Strata POC.

`SessionBundle` stores `manifest.json` and `journal.ndjson` only. The manifest retains an alias, byte length, and SHA-256 fingerprint for the source; it never stores the source path or bytes. The journal is typed, append-only, and integrity checked during load.

The on-disk contract is documented in [`docs/17-poc-session-bundle.md`](../../docs/17-poc-session-bundle.md) and specified by [`schemas/session-bundle.schema.json`](../../schemas/session-bundle.schema.json).
