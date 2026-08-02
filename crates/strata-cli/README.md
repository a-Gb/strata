# strata-cli

`strata` is the headless client for the same bounded Structure + entropy
artifact used by the executable POC.

```bash
cargo run -p strata-cli -- analyze Cargo.toml \
  --preset examples/presets/structure-entropy-fast.json \
  --range 0x0:0x200 \
  --output-format json
```

The command opens the source read-only, seals a progressive SHA-256, runs the
shared generation-aware analyzer, and prints source-path-free JSON containing
the source digest, covered exact ranges, preset, and canonical artifact digest.
The current runtime caps one analysis request at 64 MiB and applies a
120-second CLI deadline.

Exit codes: `2` usage/contract error, `3` source I/O, `4` source generation or
identity mismatch, `6` cancellation/resource bound, and `9` internal failure.
