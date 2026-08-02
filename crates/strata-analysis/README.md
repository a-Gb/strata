# strata-analysis

See the crate-level Rust documentation and the root architecture documents.

The crate combines the architectural analyzer contracts with the implemented,
deterministic POC analysis path. `src/poc.rs` is the stable public facade;
`src/poc/` separates discovery, resonance, statistics, digram counting, and
their regression tests. Keep exact source ranges and deterministic ordering
intact when extending any analyzer.
