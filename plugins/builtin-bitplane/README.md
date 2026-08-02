# Bit Plane and Word Lens

- Stage: **Core**
- Trust tier: **first-party native**
- Purpose: Reveal packed flags, channels, word widths, endianness, and numeric interpretations.
- Inputs: Byte ranges, bit/word width, endian, signedness.
- Outputs: Bit/scalar fields with exact source mapping.

## Required contracts

- CPU reference semantics before GPU optimization.
- Exact source coverage and sampling metadata.
- Immutable content-addressed artifacts.
- Pick mappings back to exact or explicitly aggregate ranges.
- Resource estimates and cancellation checkpoints.
- Semantic golden tests and adversarial fixtures.

No implementation is included in this scaffold.
