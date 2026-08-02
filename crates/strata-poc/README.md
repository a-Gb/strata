# Strata executable POC

Run the desktop app with `cargo run -p strata-poc`. The 3D Lab retains compact
three-byte projection samples, so camera and morph animation do not reread or
resample the source on every frame.

## Code organization

`main.rs` owns process startup, shared state, and the application shell. Cohesive
`app_*` modules own runtime analysis, source loading, sessions, controls,
inspectors, discovery, canvases, and projection interaction. Stateless mapping
and serialization contracts remain in their dedicated modules. `projection/`
separates model, sampling, coordinates, colour, and tests; `video/` separates
program, presets, software rendering, export, and tests. Their facade modules
preserve the existing crate API without collecting implementation detail.

Maintained source and documentation files are limited to 1,200 lines. Run
`just check-lines`; `just check` and `just lint` enforce the same invariant.

## Discovery workbench

The default `Discover` view turns bounded analysis signals into an analyst-led
reverse-engineering loop. It ranks correlations, maps every claim back to exact
source ranges, previews reversible transforms without mutating source bytes,
and promotes tested hypotheses into provenance-bearing evidence. Linked ranges
share one selection across Discover, Structure, Resonance, and the 3D Lab.

The bundled `demo://investigation-binary` fixture contains structured data and
a known single-byte-XOR branch for deterministic correlation/deobfuscation
testing. Analysis is capped by inspected bytes, windows, and returned findings;
confidence labels never replace the displayed evidence and offsets.

## Opening and comparing local sources

Use `Open…`, `Cmd+O`, or drag a file into the window to attach source A. File
dialogs are native and reads are read-only. Sources through 64 MiB retain the
contiguous path. Larger sources publish a systematic whole-address overview
from at most 64 × 256 KiB resident tiles; clicking a sampled 3D datum queues
exact level-zero focus tiles on a background worker. The Inspector discloses
logical length, LOD, resident bytes, coverage, and exact versus sampled state.

Dropping two files assigns A and B; Revision diff also exposes an explicit
source-B chooser and comparison landing state. Large pairs are compared through
matched bounded tiles, with a clickable overview strip that distinguishes
logical coverage from each exact paired read. The ordinary diff atlas remains
an explicitly labelled exact prefix. Whole-source SHA-256 advances in the
background for source-free session save and reattachment; a preview is never
treated as a complete fingerprint. Legacy Structure/Discover views likewise
label their bounded exact prefix rather than claiming full-source coverage.

## P1 analytical projections and GPU gate

The 3D projection chooser includes Alignment Lattice, Recurrence Plane,
Repetition Skyline, Spectral Waterfall, Hamming Hypercube, and Hierarchical
Block Volume. Parameters remain context-sensitive, A/B split/overlay/morph
keeps stable point IDs, recurrence partners remain exact Shift-click targets,
and all modes persist in sessions and animation programs.

Alignment and Hamming coordinates run through a real WGPU compute pipeline only
after the CPU/GPU differential passes. Search, DFT, and hierarchy stay bounded
CPU references. The UI exposes the adapter/fallback state. Run the same native
acceptance gate directly with:

```bash
cargo run -p strata-poc -- --gpu-self-test
```

## Selection dossier

Every exact primary or comparison selection creates a persistent source-free
dossier above the active canvas. It reports byte count, entropy, diversity,
text-like proportion, structure-artifact coverage, and intersecting findings,
evidence, regions, correlations, hypotheses, branches, and comparison regions.
Its fixed actions carry the same ranges into Structure, Grammar, Resonance, 3D,
comparison, reversible XOR testing, or evidence promotion. High diversity is
reported as an observation, never as a compression or encryption verdict.

## Source-free investigation sessions

Use the fixed `SESSION` control in the global header to save or reopen an
investigation. A `.strata-session` directory contains a deterministic manifest
and append-only NDJSON event trail, but no source bytes or local source path.
Its source identity is limited to a redacted alias, byte length, and SHA-256.

Reopened sessions start detached: byte-dependent controls and views remain
disabled while exact ranges, finding dispositions, evidence, comparison state,
branches, cohorts, projection settings, and camera state stay visible. `Verify
held source` checks the current in-memory source; `Reattach path` reads a chosen
candidate and activates the saved workspace only after both length and digest
match. A mismatch leaves the detached investigation unchanged.

`Cmd+S` saves the current session; reopen one from the fixed `SESSION` menu.
`Cmd+O` opens source A. A session directory can also be supplied as the first
command-line argument or dropped into the window. Saves replace each bundle
file atomically in its destination directory, and loads validate event order,
counts, digests, schema versions, and the typed POC workspace contract.

## Local projects and launch preferences

Use the fixed `Project` control in the header to write a `.strata-project`
file. This small, versioned JSON locator binds a sibling source-free session
bundle to the local source path and optional UFSC signature-pack path plus its
SHA-256. Reopening it restores the saved page, workbench mode, exact byte
ranges, evidence state, projection composition/channels/sampling, camera, and
view-specific controls. Byte-dependent analysis resumes only after the source
matches the session's saved length and whole-source SHA-256; a changed
signature pack likewise fails its pinned-digest check.

Local project locators intentionally contain filesystem paths and should not be
shared as evidence. Share the `.strata-session` directory instead. The Project
window can remember the last project, reopen it at launch, and retain a default
signature pack for ordinary source opens. These preferences are bounded JSON at
`~/Library/Application Support/Strata/project-preferences.json` (override with
`STRATA_PREFERENCES_PATH`). A project file can be passed as the first command-
line argument, selected from the Project window, or dropped into the app.

## Selection resonance

The Resonance view turns the shared selection into a live content query. Five
aligned rows compare the probe against the whole source at successively larger
window sizes. Address runs left to right, match strength rises within each row,
and a bright vertical ridge means that an echo survives across structural
scales. Clicking an echo jumps the shared selection to its exact source range.

The evidence control switches among exact positional bytes, coarse byte-class
shape, and distribution-plus-entropy texture. Every displayed score is exact
for its sampled candidate; the right-side delta labels disclose the bounded
whole-source sampling step. Results are cached until the source, selection, or
query parameters change.

## Programmable video

The 3D inspector can export the current settings as a deterministic H.264 MP4.
It also writes a `.strata.json` sidecar containing the complete animation
program, frame count, and source SHA-256 without embedding source bytes.

Render a checked-in program headlessly:

```bash
just render-poc-video examples/video/morph-through-spaces.json
```

Validate one without producing output:

```bash
cargo run -p strata-poc -- --validate-program examples/video/morph-through-spaces.json
```

Or create a starting program:

```bash
cargo run -p strata-poc -- --write-example-program output/my-animation.json
```

Four source-correlated presets can be listed or materialized without opening the
GUI:

```bash
cargo run -p strata-poc -- --list-video-presets
cargo run -p strata-poc -- --write-video-preset interleave-lattice output/program.json
```

The checked programs and their exact synthetic inputs are catalogued in
[`examples/video/README.md`](../../examples/video/README.md). The gallery covers
firmware strata, a known XOR relationship, interleaved fixed-width records, and
bit-plane image structure.

Programs keyframe normalized time (`at`), projection interpolation (`morph`),
camera yaw/pitch, zoom, and optional exact-byte focus. With a projection
composition, `morph` maps `0..=3` from named projection A to B; legacy programs
without a composition retain the trigram/orbit/helix/terrain sequence. Programs
also fix source, resolution, frame rate, duration, sampling, point budget,
channels, display look, easing, and overwrite policy. Set `source` to
`demo://investigation-binary`, `demo://composite-firmware`, or a local file
path. Relative paths resolve from the process working directory.

The display look is deliberately separate from analytical channels. The curated
films use crisp square voxels, restrained contrast, and little or no guide
ornament—never bloom or density flare. Sequential palette interpolation is done
in linear-light sRGB; H.264 outputs declare BT.709 colour space, primaries, and
transfer metadata.

When `composition` selects a P1 projection, the offline renderer computes the
same bounded CPU-reference artifact before rendering. Spectrum and recurrence
exports automatically cap analytical samples so a presentation program cannot
silently request unbounded work.

Entropy terrain gives the binary a geography. It maps linear address onto a
locality-preserving 2D Morton plane, uses normalized Shannon entropy from each
64-byte neighborhood as altitude, and blends from blue structured valleys
through cyan into amber high-entropy ridges. It remains an exact, pickable view:
the terrain is another cached position for the same source-addressed samples.

Set `source_range` to an exclusive absolute byte range when a video should
sample one binary section densely. Set `focus_offset` on every keyframe to keep
the nearest retained source byte centered while rotating, morphing, zooming, or
travelling between offsets. Samples from a range retain absolute source-file
offsets in memory and provenance.

The `macho-*` examples analyze the current 54 MiB Strata arm64 debug executable:

- `macho-self-morph-turntable.json` morphs the whole executable during a rotation.
- `macho-const-focus-dive.json` dives into the `__TEXT,__const` renderer strings.
- `macho-symbol-helix-traverse.json` travels between animation symbols in `__LINKEDIT`.
- `macho-binary-climate-self-portrait.json` makes the executable morph into its own entropy terrain and dives toward the renderer's embedded `Programmable video` string.

Those section offsets are build-specific; their generated sidecars pin the
exact executable SHA-256 used for the checked output videos.

FFmpeg is discovered through `STRATA_FFMPEG`, Homebrew's standard paths, then
`PATH`. Failed encoding preserves the rendered PNG directory named in the
error. Successful exports remove transient frames after the MP4 and provenance
sidecar are finalized. Run `just validate-video-gallery` or
`just render-video-gallery` to exercise all four curated programs.
