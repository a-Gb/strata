# Product brief

## Name and positioning

**Strata** is a GPU-native binary visual analysis workbench for Apple Silicon. It sits between raw byte inspection and format-specific semantic tooling. It is intended to turn opaque data into navigable, reproducible hypotheses without pretending that visual appearance is proof.

## Problem

Hex editors preserve exact values but collapse global structure. Parsers and disassemblers expose meaning but require assumptions that may be wrong, unavailable, or adversarially manipulated. Existing binary visualizers each illuminate a useful projection, but commonly isolate that projection from the rest of the investigative workflow.

Strata unifies five questions:

| Question | Product surface | Typical answer |
|---|---|---|
| Where is structure? | Atlas views | Boundaries, patches, sparse areas, high-entropy payloads |
| What relationships occur? | Grammar views | Digrams, trigrams, stride-N transitions, recurrence |
| What does it resemble? | Morphology views | Known fingerprints, repeated regions, architecture-like distributions |
| What can be interpreted? | Semantic overlays | Strings, sections, symbols, parser objects, signatures |
| Why should I trust this? | Evidence/provenance | Exact ranges, parameters, sampling, implementation identity |

## Primary users

1. Reverse engineers triaging unknown executables, firmware, memory dumps, archives, and embedded payloads.
2. Digital-forensics practitioners locating anomalies and producing reviewable evidence.
3. File-format and protocol researchers discovering record widths, periodicity, endianness, and encoding structure.
4. Security researchers comparing packed, obfuscated, mutated, or versioned binaries.
5. Educators and students learning how byte-level structure manifests visually.
6. Artists and data-visualization practitioners exploring arbitrary data, with analytical truth preserved beneath aesthetic projections.

## Jobs to be done

- Open a multi-gigabyte or sparse source without loading it all into memory.
- See a meaningful first structural overview quickly.
- Move from a visible feature to its exact source bytes in one gesture.
- Compare multiple projections without losing selection or coordinate context.
- Test reversible hypotheses such as width, stride, endianness, XOR, deinterleave, or offset alignment.
- Escalate a selected range to a hex inspector, parser, disassembler bridge, extractor, or external command.
- Save a source-free session that another analyst can reproduce against the same source hash.
- Export imagery without discarding the evidence chain that produced it.

## Product principles

### 1. One source, many synchronized projections

No visualization owns the truth. All views subscribe to a shared source, coordinate model, selection set, and transform graph.

### 2. Progressive before exhaustive

The first response should use sampling or coarse tiles. Exact results refine in place. The UI must label approximation, coverage, and completion rather than presenting sampled output as exact.

### 3. Reversible interaction

Every visual selection maps back to source ranges. Every transform records enough information to reproduce or invert it when mathematically possible.

### 4. CPU truth, GPU acceleration

A deterministic CPU reference implementation defines expected behavior for core analyzers. GPU kernels accelerate and aggregate but are tested against the reference within explicit integer or floating-point tolerances.

### 5. Safe by default

Sources are read-only. Plugins receive only declared capabilities and selected ranges. Network, process access, raw native loading, and persistence of source bytes are opt-in.

### 6. Explain before classify

Deterministic statistics and visible evidence precede machine-learned ranking. ML may retrieve similar samples or prioritize anomalies, but never becomes the sole provenance for a visual claim.

### 7. Interesting is allowed; misleading is not

3D, animation, sonification, and generative palettes are valid exploration modes when they retain traceability, parameter disclosure, and an analytical fallback.

## Non-goals for the first product

- Replacing Ghidra, Binary Ninja, Hopper, IDA, a full hex editor, or a full forensic suite.
- Executing untrusted binaries or emulating instruction sets.
- Automatically proving file type, encryption, malware, or intent from appearance.
- Uploading sources to a service for analysis.
- Editing source bytes in place.
- Supporting every desktop platform before the Apple Silicon path is stable.

## Product surfaces

### Workbench

A multi-pane, keyboard-first desktop environment with source navigator, visualization canvas, inspector, evidence notebook, and command palette.

### CLI

A headless companion for deterministic analysis, cache warming, preset execution, session validation, and export. It shares domain and analyzer crates with the GUI.

### Plugin SDK

A WIT-defined WebAssembly component interface for analyzers, format overlays, declarative views, exporters, and commands. First-party high-performance views remain native crates.

### Bridge API

A local, disabled-by-default IPC endpoint for Ghidra, LLDB, rizin, or custom tools to navigate ranges and exchange annotations without exposing source bytes unnecessarily.

## Success outcomes

- The user can identify structural regions in an unknown source before choosing a parser.
- A selected visual anomaly resolves to exact byte ranges with no ambiguity.
- Two independent machines can reproduce an exported view from a session manifest and matching source hash.
- A malformed input or plugin can fail without corrupting the source or taking down the entire application.
- The core interaction remains responsive while exact analysis continues incrementally.
