# Curated binary films

These four programs are analytical demonstrations, not decorative camera reels.
Each one is paired with a deterministic synthetic binary whose exact structure,
license, and SHA-256 are recorded in [`fixtures/video/manifest.json`](../../fixtures/video/manifest.json).
Every rendered voxel retains the same stable source ID and exact byte range
through projection changes.

| Preset | Ground truth in the input | Projection story | Visible encoding |
|---|---|---|---|
| [Firmware Stratigraphy](firmware-stratigraphy.json) | Four 256-byte strata: erased padding, text, fixed records, high-complexity payload | Hilbert locality becomes fixed complexity phase space, then returns | Cividis address colour; entropy height; change-rate size |
| [XOR Correlation Reveal](xor-correlation-reveal.json) | `320..576` is copied to `576..832` with exact XOR `0xa7` | Address-separated regions move from Hilbert space into a fixed statistical basis | Cyan-to-amber address colour distinguishes co-located regions; the manifest, not proximity alone, proves the XOR relation |
| [Interleave Lattice](interleave-lattice.json) | 24 by 16 RGB samples; three little-endian 16-bit lanes; six-byte records | A 144-byte address raster opens into a six-residue alignment lattice | Cividis byte value exposes alternating low/high bytes and lane gradients |
| [Bitplane Blueprint](bitplane-blueprint.json) | 32 by 16 grayscale plane | The address image separates into eight address-stable bit layers | Restrained monochrome occupancy; every source byte is instantiated in each plane |

The look is intentionally voxel-first: square samples, near-black neutral
backgrounds, bounded contrast, low or absent guides, and no bloom, flare, or
synthetic density glow. Palette interpolation happens in linear light and the
encoded MP4 declares BT.709 colour metadata.

## Reproduce

```bash
just video-fixtures
just validate-video-gallery
just render-video-gallery
```

List or materialize a preset from the executable:

```bash
cargo run -p strata-poc -- --list-video-presets
cargo run -p strata-poc -- --write-video-preset firmware-stratigraphy output/program.json
```

Each render produces an H.264 MP4 and `.strata.json` provenance sidecar under
`output/video/`. The sidecar pins the source digest, full program, and frame
count without embedding source bytes.

## Reading the films

- Position is analytical data, not ornament. A morph changes the coordinate
  system while preserving point identity.
- Colour always names one disclosed channel: address, byte value, or entropy.
- Height and size are independent channels and are omitted when they do not add
  information.
- Statistical convergence is a discovery cue, not proof. Exact relationships
  remain in fixture ground truth and byte provenance.
- 3D views are orientation aids. The opening or closing view is deliberately
  planar where a planar view communicates the result more honestly.

Generated nine-frame review sheets live beside the videos as `*.contact.png`;
they are intended for transition QA rather than as substitute evidence.
