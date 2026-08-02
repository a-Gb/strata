# Intended outcomes and acceptance scenarios

## Outcome 1 — unknown firmware triage

### Input

A multi-gigabyte firmware image with boot code, tables, erased regions, compressed payloads, and an unknown filesystem.

### Workflow

1. Open read-only; coarse byte-class and entropy atlases appear progressively.
2. Erased `0xff` regions, low-entropy tables, and high-entropy payloads separate visually.
3. Brush a transition boundary; exact bytes and rolling tracks refine.
4. Compare raw, digram, stride-N, and strings views for the selected region.
5. Apply a bounded decompression probe or parser overlay as a branch.
6. Promote the selected range and findings into an evidence record.
7. Export ranges and navigation metadata to a disassembler bridge.

### Acceptance

Every displayed boundary and finding identifies exact source ranges, analysis state, and sampling. No parser is required to obtain the initial map.

## Outcome 2 — versioned binary comparison

### Input

Two application builds or firmware revisions with moved sections and local modifications.

### Workflow

1. Place sources in a comparison group.
2. View synchronized atlases with exact deltas under known alignment.
3. Run rolling-hash anchors to propose moved-region correspondences.
4. Inspect entropy and n-gram divergence tracks.
5. Select a changed region and navigate both exact byte ranges.
6. Export an alignment ribbon and evidence frames.

### Acceptance

The tool distinguishes exact comparison from proposed alignment and never maps bytes across sources without recording the method and confidence.

## Outcome 3 — record width and interleaving discovery

### Input

An undocumented sensor or image dump with repeated fixed-width records and interleaved channels.

### Workflow

1. Width sweep and autocorrelation rank candidate record lengths.
2. Select a candidate lag; atlas rows and stride-N digrams update.
3. Create deinterleave branches for lanes.
4. Reinterpret lanes as signed/unsigned words under both endian modes.
5. Inspect bit planes and spectrogram/track views.

### Acceptance

The user can reproduce the exact transform branch and recover each derived value’s contributing source bytes.

## Outcome 4 — packed or obfuscated executable

### Input

A packed binary containing small readable headers and a high-entropy body.

### Workflow

1. Atlas and entropy tracks identify regions without claiming encryption or compression.
2. Digram/conditional digram contrast header and body grammars.
3. Signature and executable overlays remain candidate evidence.
4. A user tests XOR/shift transforms or a bounded unpacking plugin on a selected range.
5. Results are compared side-by-side with the source.

### Acceptance

The application describes observed properties and transform outcomes, not a malware or encryption verdict.

## Outcome 5 — embedded object discovery

### Input

A container or disk image with embedded images, archives, and duplicated assets.

### Workflow

1. Signatures and structural boundaries create candidate regions.
2. Rolling-hash similarity links duplicates.
3. Semantic overlays show candidate objects atop physical layout.
4. The user extracts selected exact ranges with provenance sidecars.

### Acceptance

Extraction is explicit, read-only, and records why a range was selected. False signatures remain visible as candidates.

## Outcome 6 — huge sparse source

### Input

A sparse disk image or virtual source too large to read fully.

### Workflow

1. Source map distinguishes holes, unavailable regions, and real zero bytes.
2. Overview uses declared systematic/adaptive sampling.
3. Zooming and selecting trigger exact reads only for visible ranges.
4. Full-source analyses remain optional queued jobs.

### Acceptance

The UI never renders holes as silently equivalent to zero bytes and never suggests the overview is exact when sampled.

## Outcome 7 — corpus fingerprint retrieval

### Input

A local corpus of binaries from several formats and compiler/architecture families.

### Workflow

1. Compute deterministic feature bundles: histograms, n-grams, entropy distributions, region summaries, recurrence signatures.
2. Query by a selected region or full sample.
3. Browse nearest samples and feature-level explanations.
4. Optionally add a local ML reranker whose score is secondary evidence.

### Acceptance

Similarity results identify contributing features, corpus/index versions, and known fragility. Visual resemblance is not presented as identity.

## Outcome 8 — teaching and visual communication

### Input

A curated binary demonstrating text, image, code, compression, padding, and repeated records.

### Workflow

1. Build a story sequence across atlas, digram, layered digram, and semantic views.
2. Animate longitudinal slices and selections.
3. Export frames with legends and reproducibility links.
4. Use optional palette or sonification modes without losing analytical metadata.

### Acceptance

Presentation mode is visually compelling while every frame remains tied to source, parameters, and exact/approximate status.

## Outcome 9 — hostile plugin or malformed input

### Input

A plugin loops, requests excessive bytes, or emits oversized geometry; a source contains malformed nested structures.

### Workflow

1. Capability broker rejects or bounds requests.
2. Runtime terminates the plugin on quota breach.
3. Parser/decompressor failure becomes a scoped result error.
4. Other views and evidence remain available.

### Acceptance

No source modification, host crash, uncontrolled network access, or silent evidence corruption occurs.

## Product-level acceptance matrix

| Capability | Minimum acceptable outcome |
|---|---|
| Open | Read-only, progressive, cancellable, no main-thread full hash |
| Visualize | Multiple linked views with visible exactness/sampling |
| Navigate | Visual feature to exact ranges or explicit aggregate mapping |
| Hypothesize | Reversible transform branches with ancestry |
| Interpret | Semantic overlays isolated from raw evidence |
| Compare | Alignment method and confidence explicit |
| Reproduce | CLI/GUI session produces matching semantic artifacts |
| Extend | External plugin is capability-scoped and resource-limited |
| Export | Inventory, redaction, provenance, atomic write |
| Fail | Scoped degradation with session/source integrity preserved |
