# strata-runtime

Shared source, tiling, digest, comparison, and analysis orchestration used by
every Strata frontend.

The crate owns no UI state and retains no source bytes beyond explicitly bounded
overview, matched-diff, or analyzer artifacts. Progressive whole-source hashing
publishes only progress and a sealed digest. See the crate-level Rust
documentation for the current production-promotion contract.
