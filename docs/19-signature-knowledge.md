# Signature knowledge

Status: working UFSC `0.1.x` adapter and linked visual evidence.

## Flow

```text
UFSC JSON bytes
  -> strict bounded import
  -> immutable compiled catalog
  -> declared-offset probes
  -> rare-byte anchored embedded search
  -> ranked candidate ranges
  -> Discover map + inspector + 3D voxel outlines
```

## Analyst semantics

- **Declared offset** means the bytes match where the catalog says they should.
- **Embedded search** relaxes a BOF rule and is always labeled as a hypothesis.
- Category colour is a thin outline. It does not replace raw byte colour.
- Amber selection overrides signature colour.
- Each catalog match reserves a deterministic exact 3D sample, so short magic
  ranges remain visible even when bounded uniform sampling skips nearby bytes.
- Every match retains exact ranges, pattern, wildcard count, candidate records,
  source attribution, pack version, and pack SHA-256.

## Strict UFSC subset

Accepted:

- one normalized hex pattern per record;
- `bof`, non-negative fixed, and `eof`/footer offsets;
- exact bytes plus `??` wildcard bytes;
- patterns of 2–256 bytes.

Skipped and counted:

- missing or multiple patterns;
- variable/container-path rules;
- invalid, oversized, or one-byte patterns;
- unsupported envelope versions or packs above 16 MiB.

Embedded search is more conservative than declared matching. It requires at
least four exact bytes, three distinct values, no dominant padding byte, and
additional source/category support for collision-prone patterns. Each rule is
indexed by its rarest exact byte rather than its first byte.

## External producer audit snapshot

Audited pack: `file_signatures_latest.json`, SHA-256
`acf6c39f0dbf38569c4d4ea67014a0d87c2f93d7a41f7e393e0302d3530105f4`.

- 3,401 input records;
- 2,869 executable Strata rules;
- 532 explicitly skipped records;
- 508 embedded-search-eligible rules;
- 87 named upstream sources in the producer export.

Do not vendor this generated pack. The audited producer snapshot lacks
the `LICENSE` file referenced by its README, gives every record the same
`medium` strength, and scalar merge behavior can collapse competing aliases
for shared magic such as `CA FE BA BE`. Strata therefore derives confidence
from byte specificity, diversity, offset agreement, and corroborating sources;
it does not trust the strength field.

## Use

Load a pack from Inspector -> **Signature knowledge**, or set:

```bash
STRATA_SIGNATURE_PACK=/absolute/path/to/file_signatures_latest.json
```

Inspect a pack and optional source without opening the GUI:

```bash
cargo run -p strata-analysis --example inspect_signature_pack -- \
  /absolute/path/to/file_signatures_latest.json \
  /absolute/path/to/source.bin
```

## Next contract work

- Preserve all aliases during producer merges instead of overwriting a label.
- Export source-license identifiers per record and for the complete pack.
- Represent compound/relative rules explicitly for RIFF, ISO-BMFF, ZIP/OLE
  members, trailers, and nested-container paths.
- Feed corroborated signature families into the Pattern Explanation Engine so a
  header, companion field, trailer, entropy boundary, and parser probe can form
  one testable hypothesis.
