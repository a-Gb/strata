# View and analyzer catalog

## View composition model

A Strata workspace is not a fixed dashboard. Views are nodes bound to a source, transform graph, and one or more analysis artifacts. Views can be linked by cursor, selection, camera domain, normalization, or comparison group.

Each catalog entry is tagged:

The implemented 3D POC uses the orthogonal [projection composition contract](18-projection-composition.md): sample domain, projection, geometry, channels, overlays, and A/B comparison are independent state.

- **MVP**: validates the core architecture.
- **Core**: required for a credible first release.
- **Advanced**: high-value follow-on.
- **Experimental**: useful or expressive, but not on the critical path.

## A. Locality and spatial structure

| View | Stage | Representation | Analytical use | Primary interaction |
|---|---:|---|---|---|
| Byte-class atlas | MVP | Raster/Hilbert/Morton categorical map | Find text-like, null, `0xff`, binary, and boundary regions | Pan, zoom, brush, palette switch |
| Entropy atlas | MVP | Multi-resolution scalar field | Locate regular versus high-complexity regions | Window-size scrub, threshold masks |
| Raw byte heatmap | MVP | 0–255 scalar field | See gradients, tables, repeated values | Histogram-linked normalization |
| Layout laboratory | Core | Same data across multiple space-filling layouts | Separate real structure from projection artifacts | Synchronized small multiples |
| Bit-plane atlas | Core | Eight binary planes or packed mosaic | Reveal flags, masks, image planes, interleaving | Plane isolate, animate, combine |
| Word lens | Core | 16/24/32/64-bit reinterpretation | Detect endian fields, counters, samples, pointers | Width/endian/signedness sweep |
| Width sweep | Advanced | Grid of candidate row widths | Expose raw image dimensions and record lengths | Drag width, rank periodic candidates |
| Sparse/hole map | Advanced | Explicit absent/zero/unmapped regions | Inspect disk images and sparse firmware | Toggle logical versus physical coverage |
| Section-packed atlas | Advanced | Parser regions packed as blocks | Compare semantic and physical organization | Click section to reveal original offsets |
| Semantic overlay atlas | Core | Atlas plus strings/signatures/sections | Keep pre-semantic and semantic evidence together | Layer opacity and filter |
| Live append atlas | Experimental | Time-growing strip/tile map | Explore streams and captures | Pause, pin generation, rolling window |

## B. Statistical grammar and transition space

| View | Stage | Representation | Analytical use | Primary interaction |
|---|---:|---|---|---|
| Byte histogram | MVP | 256-bin bar/line plot | Baseline distribution and selection comparison | Brush bins, log/linear scale |
| Digram matrix | MVP | 256×256 count/probability texture | Reveal adjacent-byte transition grammar | Crosshair, row/column conditional view |
| Positional digram | Core | Digram intensity plus source-position moments | Distinguish transitions concentrated by region | Hue/position mode and range filtering |
| Conditional digram | Core | `P(next \| current)` matrix | Remove dominance by frequent source bytes | Normalize by row/column/global |
| Stride-N digram | Core | Pair matrix at configurable stride | Detect interleaving, word fields, periodic records | Live stride scrub and candidate ranking |
| Layered digram | Core | Stack of region matrices | Show where transition grammars appear | Slice, sweep, volume, region brush |
| Trigram cloud | Advanced | Sparse 3D points or voxel density | Fingerprint local byte grammar | Orbit, clip planes, top-K filter |
| Trigram projections | Advanced | XY/XZ/YZ matrices and small multiples | Analyze sparse trigram domain without 3D occlusion | Linked projection brushing |
| Transition graph | Experimental | Byte states as nodes, weighted directed edges | Expose constrained alphabets and transition hubs | Threshold, community layout, path query |
| Markov residual | Advanced | Observed minus model expectation | Surface surprising transitions | Select model order and significance |

## C. Sequence, repetition, and periodicity

| View | Stage | Representation | Analytical use | Primary interaction |
|---|---:|---|---|---|
| Rolling-stat strip | MVP | Offset-aligned tracks | Compare entropy, density, mean, novelty, strings | Stack/reorder tracks, linked cursor |
| Autocorrelation plot | Core | Lag versus correlation | Infer record sizes, channels, periodic padding | Select range, click lag to set stride/width |
| Recurrence plot | Advanced | Position×position similarity matrix | Reveal repeated blocks and sequence motifs | Brush diagonal/off-diagonal structures |
| Rolling-hash similarity atlas | Core | Tile-level nearest/repeated region map | Find duplicated or moved structures | Jump between matches |
| Run-length view | Core | Value/run distributions and offset track | Detect padding, masks, RLE, erased flash | Filter by minimum run |
| Frequency/spectrogram lens | Experimental | FFT/STFT of numeric reinterpretation | Explore audio, sensor, or periodic numeric data | Width/endian/sample-type controls |
| Lempel-Ziv complexity strip | Advanced | Local dictionary-growth proxy | Differentiate repetitive and complex regions | Compare against entropy |
| Change-point map | Advanced | Ranked structural boundary candidates | Focus inspection on likely transitions | Sensitivity and evidence expansion |

## D. Semantic and reverse-engineering overlays

| View/analyzer | Stage | Output | Notes |
|---|---:|---|---|
| Hex/ASCII inspector | MVP | Exact bytes around cursor/selection | Not a full editor; read-only and synchronized |
| Strings analyzer | MVP | Encoding-aware string ranges | ASCII/UTF-8/UTF-16 initially; confidence visible |
| Magic/signature analyzer | MVP | Candidate file signatures | Never silently claims a type from magic alone |
| Generic region annotations | MVP | Named ranges, comments, tags | Analyst-owned evidence layer |
| Executable section bridge | Core | Sections/symbols/relocations from safe parser | Parser output remains an overlay, not source truth |
| Embedded-object candidates | Core | Candidate ranges and extraction actions | Uses signatures plus structural corroboration |
| Integer/pointer plausibility | Advanced | Candidate scalar/vector fields | Address-space aware; architecture hypothesis explicit |
| Compression probes | Advanced | Bounded decoder outcomes | Isolated, resource-limited, no automatic expansion bomb |
| Disassembler bridge | Advanced | Navigation and annotation exchange | Ghidra/rizin/LLDB local IPC; no duplicated disassembler |
| Parser object graph | Experimental | Nodes/edges linked to source ranges | Useful for containers and nested formats |

## E. Comparison and corpus analysis

| View | Stage | Representation | Use |
|---|---:|---|---|
| Synchronized dual atlas | Core | Side-by-side linked layouts | Compare versions or packed/unpacked samples |
| Exact delta atlas | Core | Equal/XOR/delta classes | Show changed bytes under known alignment |
| Alignment ribbon | Advanced | Source A↔B anchor mapping | Visualize insertions, deletions, and moved blocks |
| Statistic delta tracks | Core | Entropy/histogram/n-gram divergence by offset | Find behavior changes without exact byte identity |
| Region similarity graph | Advanced | Regions as nodes, similarity as weighted edges | Cluster repeated tables, code families, assets |
| Corpus fingerprint browser | Advanced | Embedding/fingerprint projection with exemplars | Retrieve similar samples; deterministic features first |
| Mutation stability view | Experimental | Same sample under synthetic benign mutations | Evaluate whether a fingerprint is robust or fragile |

## F. Reversible transform laboratory

Transforms never overwrite the source. They form a visible branch in the provenance graph.

| Transform | Stage | Use | Safety/validity note |
|---|---:|---|---|
| Slice/concatenate | MVP | Isolate or assemble ranges | Exact mapping retained |
| Offset/phase shift | MVP | Test alignment hypotheses | Range clipping explicit |
| Stride/deinterleave | Core | Separate channels or record fields | Produces discontiguous source mappings |
| Endian/word reinterpret | Core | Inspect numeric fields | Interpretation only; no byte mutation |
| Bit mask/plane/extract | Core | Reveal flags and packed channels | Loss model recorded |
| XOR/rotate/shift | Advanced | Probe light obfuscation | Key/operation explicit; no “decoded” claim |
| Byte permutation | Advanced | Test channel/order hypotheses | Inverse required for reversible status |
| Bounded decompression | Advanced | Inspect candidate compressed ranges | Isolated and quota-limited |
| Text decoding | Core | Explore encodings | Invalid sequence policy recorded |
| User expression | Experimental | Restricted vector expression over bytes/words | Sandboxed DSL, no arbitrary native execution |

## G. Interactive and expressive modes

| Mode | Stage | Purpose | Guardrail |
|---|---:|---|---|
| Linked brushing | MVP | Select once, inspect everywhere | Exact/aggregate mapping shown |
| Focus lens | Core | Alternate view under cursor without opening a pane | Lens parameters visible |
| Animated offset traversal | Core | Follow structure longitudinally | Animation never changes analysis state silently |
| 3D slice choreography | Experimental | Explore layered digram/trigram volumes | 2D analytical fallback always available |
| Sonification | Experimental | Hear periodicity, density, or transitions | Mapping preset and range recorded |
| Palette lab | Experimental | Perceptual/accessibility and artistic mappings | Raw values and color legend always recoverable |
| Presentation/story mode | Advanced | Sequence evidence views for teaching/reporting | Each frame links to session state and source hash |
| Collaborative annotation export | Advanced | Share source-free findings | No live cloud service required |

## Recommended default workspace

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ Source tabs | command palette | hash state | analysis status | GPU budget │
├─────────────┬───────────────────────────────────────┬──────────────────────┤
│ Source tree │ Byte/entropy atlas                    │ Inspector             │
│ + overlays  │                                       │ hex / values / stats  │
│             │                                       │ provenance            │
├─────────────┼──────────────────────┬────────────────┼──────────────────────┤
│ Tracks      │ Digram matrix        │ Layered slice  │ Evidence notebook     │
│             │                      │                │ selections / claims   │
└─────────────┴──────────────────────┴────────────────┴──────────────────────┘
```

The default is intentionally 2D and legible. 3D and expressive views are opt-in tools, not the application’s home screen.
