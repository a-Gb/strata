# Binary Comparison

- Stage: **Core**
- Trust tier: **first-party native**
- Purpose: Exact aligned deltas and proposed moved-region correspondences.
- Inputs: Two source/range bindings, alignment policy.
- Outputs: Delta tiles, equality runs, anchors, confidence metadata.

## Required contracts

- CPU reference semantics before GPU optimization.
- Exact source coverage and sampling metadata.
- Immutable content-addressed artifacts.
- Pick mappings back to exact or explicitly aggregate ranges.
- Resource estimates and cancellation checkpoints.
- Semantic golden tests and adversarial fixtures.

No implementation is included in this scaffold.
