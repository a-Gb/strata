# Seeing the Binary: BinVis, Veles, and CantorDust as Visual Reverse-Engineering Systems

This is a repository-authored design synthesis, not vendored documentation from
the projects discussed below. Project names remain the property of their
respective owners.

## Abstract

BinVis, Veles, and CantorDust belong to a class of tools that treat visualization as a pre-semantic analysis layer for arbitrary binary data. Rather than beginning with a parser, file signature, instruction decoder, or assumed architecture, they transform byte sequences into spatial structures that exploit human sensitivity to repetition, boundaries, texture, density, and anomalies.

Their shared objective is not to replace hex editors, disassemblers, entropy analysis, or format-specific parsers. It is to answer an earlier question: **what kind of structure exists here, and where should an analyst look next?**

The systems differ principally in the relationship they preserve:

- **BinVis preserves locality in file-offset space.**
- **Veles preserves statistical relationships among adjacent byte values.**
- **CantorDust emphasizes recognizable visual fingerprints and their operational use during reverse engineering.**

Together they establish binary visualization as a human-in-the-loop triage mechanism: a lossy but rapid transformation from opaque byte streams into navigable hypotheses.

---

## 1. Problem Model

Let a binary object be represented as:

[
B=(b_0,b_1,\ldots,b_{N-1}),\qquad b_i\in{0,\ldots,255}
]

Traditional tools expose (B) through one of two extremes:

1. **Raw representations**, such as hexadecimal or ASCII dumps, retain exact values but provide weak global structure.
2. **Semantic representations**, such as disassembly or file parsing, produce meaningful objects but require assumptions about format, architecture, alignment, or encoding.

Binary visualization occupies the intermediate layer:

[
B \rightarrow \text{visual representation} \rightarrow \text{analyst hypothesis}
]

The transformation is usually many-to-one and therefore non-invertible. Its purpose is not exact decoding but perceptual compression: retaining selected structural invariants while discarding detail that obstructs rapid inspection.

A useful binary visualizer should support four operations:

- **Segmentation:** reveal transitions between structurally different regions.
- **Classification:** expose visual signatures associated with code, text, images, tables, padding, compression, or encryption.
- **Localization:** map a visual feature back to an offset or selectable byte range.
- **Escalation:** hand the selected region to a hex editor, parser, disassembler, decompressor, or statistical tool.

---

## 2. BinVis: Locality-Preserving Binary Cartography

### Aim

BinVis is designed as a broad structural overview of a binary object. Its motivating observation is that conventional hex inspection is inherently local: an analyst can examine the bytes presently on screen but cannot easily determine whether a different region, perhaps megabytes away, contains padding, text, repeated tables, executable code, or compressed material.

BinVis therefore treats the binary as a one-dimensional terrain that must be projected onto a two-dimensional surface without destroying local relationships. Its browser implementation supports interactive exploration, multiple byte-color mappings, entropy visualization, intuitive scan-based selection, and export of selected ranges. Analysis was initially performed locally in the browser, avoiding server-side transmission of the opened file.

### Visual concept

BinVis separates visualization into two functions:

[
\text{pixel position}=L(i),\qquad
\text{pixel color}=C(b_i,\mathcal{N}_i)
]

Here, (L) maps file offset (i) into image coordinates, while (C) maps the byte—or statistics around it—to color.

The elementary color model groups bytes into perceptually useful classes:

- `0x00`: null or zero-filled regions;
- `0xFF`: fully set bytes and common erased-memory padding;
- printable values: likely text or text-like tables;
- all other values: generic binary material.

This intentionally sacrifices byte-level precision to expose macroscale texture. More detailed palettes may map all 256 values into distinct but related colors, increasing structural resolution at the cost of immediate semantic readability.

The critical design decision is the layout function (L). A simple raster or zigzag layout is intuitive because rows correspond approximately to increasing file offsets, but narrow features may be fragmented across row boundaries. Space-filling curves—particularly the Hilbert curve—improve locality preservation:

[
|i-j|\ \text{small} \Rightarrow
|L(i)-L(j)|\ \text{usually small}
]

This causes contiguous regions to form coherent patches rather than thin, broken stripes. Hilbert layouts preserve local clustering more effectively than Z-order layouts, although their geometry makes exact offsets less visually obvious. BinVis consequently exposes both an intuitive scan layout and locality-preserving layouts rather than asserting that one projection is universally optimal.

A second mapping uses local Shannon entropy:

[
H(W_i)=-\sum_{v=0}^{255}p_i(v)\log_2 p_i(v)
]

where (W_i) is a window around offset (i). Low-entropy regions suggest padding, repeated structures, sparse tables, or highly regular code. High-entropy regions suggest compression, encryption, hashes, or naturally noisy media. Entropy does not identify the underlying mechanism, but it makes boundaries and exceptional regions immediately visible.

### Core visual metaphor

**BinVis is a map.** Offset is geography; color is material composition; zoom and selection convert overview into evidence.

---

## 3. Veles: Byte-Transition Geometry

### Aim

Veles combines a hex-oriented binary explorer with statistical visualization and an extensible analysis framework. Rather than asking primarily where particular byte classes occur, it asks which byte transitions occur, how frequently they occur, and how those relationships change through the file. Its developers describe the visualizations as format-independent statistical representations: executable, image, archive, and disk data are all treated initially as undifferentiated byte sequences.

### Digram representation

For each adjacent byte pair:

[
g_i=(b_i,b_{i+1})
]

Veles constructs a (256\times256) frequency matrix:

[
D[x,y]=\sum_{i=0}^{N-2}
\mathbf{1}[b_i=x \land b_{i+1}=y]
]

Each possible first byte becomes an (x)-coordinate and each possible successor becomes a (y)-coordinate. Pixel luminance represents normalized pair frequency. Dense blocks, lines, voids, diagonals, and isolated points therefore reveal transition grammars that cannot be seen from a simple byte histogram.

Position within the file can be encoded chromatically: pairs concentrated near the beginning tend toward one hue, pairs concentrated near the end toward another, while broadly distributed pairs remain comparatively neutral. The image simultaneously represents **what transitions exist**, **how common they are**, and approximately **where they occur**.

### Trigram and layered representations

Veles generalizes the same model to byte triples:

[
t_i=(b_i,b_{i+1},b_{i+2})
]

forming a sparse volumetric field:

[
T[x,y,z]=\operatorname{count}(x,y,z)
]

The resulting (256^3) state space acts as a visual fingerprint of local byte grammar. Instruction encodings, text, bitmap structures, serialized integers, and compressed streams occupy this space differently. Machine-code families may produce recognizable bars or planes because instruction prefixes, operand encodings, and common byte subsequences constrain transition probabilities.

The **layered digram** view introduces file position explicitly. The input is divided into 256 regions, a digram distribution is computed for each, and those matrices are stacked into a three-dimensional cube. Formally:

[
D_k[x,y]=
\operatorname{count}*{i\in R_k}(b_i=x,b*{i+1}=y)
]

This reveals not merely that a transition exists, but in which longitudinal region it becomes active. Headers, repeated records, code sections, symbol data, compressed payloads, and trailing indexes can emerge as separate strata.

A minimap supplements these statistical views. Equal-sized file regions are reduced to texels representing either average byte value or Shannon entropy, providing a lower-cost positional overview linked to the richer n-gram geometry.

### Core visual metaphor

**Veles is a phase space.** Bytes are states; adjacency is motion; frequency creates density; file position becomes depth or hue.

---

## 4. CantorDust: Visual Fingerprinting for Reverse Engineering

### Aim

CantorDust was conceived as a radical extension of the hex editor: an instrument for identifying patterns in unknown binary material before rigid interpretation begins. Its central operational concern is the gap between raw tools, which expose bytes without interpretation, and semantic tools, which require the analyst to select the correct interpretation in advance.

This gap matters when the analyst does not yet know whether a region contains executable instructions, text, graphics, audio, firmware tables, encoded data, or several concatenated formats. CantorDust seeks to let the analyst recognize a likely “species” of data from its visual fingerprint and then choose the appropriate downstream tool. The original system was presented in 2012 and was later released as a Ghidra-integrated implementation.

### Visual concept

A central CantorDust visualization is the byte digraph, conceptually equivalent to a directed transition matrix. Sequential bytes ((b_i,b_{i+1})) are mapped to Cartesian coordinates. Repeated pairs intensify corresponding points, creating structures characteristic of the source data.

Human-readable text, for example, occupies constrained byte ranges. Lowercase-to-lowercase transitions form one cluster; uppercase-to-lowercase transitions form another; spaces, punctuation, and line endings create horizontal or vertical features. Machine code produces different line systems based on architecture-specific instruction distributions. Audio, bitmap data, encoded text, and other formats generate their own recurrent forms.

CantorDust’s distinction is less a unique mathematical primitive than an analytic posture: visual fingerprints are embedded directly into reverse-engineering practice. In its Ghidra form, the visualization operates on the program already loaded into the disassembler, reducing the distance between perceptual discovery and semantic examination. A suspicious visual feature can become a target for navigation, disassembly, type recovery, or manual annotation rather than remaining an isolated image.

### Core visual metaphor

**CantorDust is a field microscope.** The analyst learns recurring morphologies, identifies anomalous structures, and immediately moves from appearance to dissection.

---

## 5. Comparative Model

| System         | Preserved invariant                        | Primary representation                   | Strongest capability                                              | Principal weakness                                                       |
| -------------- | ------------------------------------------ | ---------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------ |
| **BinVis**     | Approximate offset locality                | Raster or space-filling byte map         | Finding boundaries and selecting regions                          | Layout and palette can obscure exact byte semantics                      |
| **Veles**      | Adjacent-byte correlation                  | 2D digrams, 3D trigrams, layered digrams | Recognizing statistical grammars and architecture-like signatures | Frequency aggregation can discard exact sequence and offset              |
| **CantorDust** | Learned visual fingerprint plus RE context | Digraph and binary morphology views      | Rapid classification inside a disassembly workflow                | Recognition depends heavily on analyst experience and reference examples |

The systems are therefore complementary rather than substitutable:

[
\text{BinVis: where?}
]

[
\text{Veles: what transition structure?}
]

[
\text{CantorDust: what does this resemble, and what should I inspect next?}
]

---

## 6. Limits and Design Implications

Binary visualization is an evidence-generating interface, not a proof system.

Different binaries may produce similar visual signatures, while semantically equivalent binaries may look radically different after recompilation, packing, encryption, alignment changes, or insertion of padding. High entropy cannot reliably distinguish compression from encryption. N-gram views lose ordering beyond their selected window. Space-filling maps introduce projection artifacts. Sampling can hide narrow structures, and color mappings introduce perceptual and accessibility biases.

The visual layer is also manipulable. An adversary may add inert padding, reorder sections, alter encodings, or perturb non-executed data to change a visualization without meaningfully changing program behavior. Any automated classifier built on these images inherits those attack surfaces.

A modern successor should therefore synchronize several reversible views around a single offset model:

```text
Binary source
    ├── locality map: byte class / entropy / parser overlays
    ├── transition space: digram / trigram / stride-N relations
    ├── semantic layer: sections / instructions / symbols / strings
    └── evidence layer: selections / hypotheses / annotations / exports
```

The invariant should be that every visible feature can be traced back to its contributing byte ranges, transformation parameters, and sampling policy. Machine learning may rank visual anomalies or retrieve similar fingerprints, but it should not replace deterministic provenance.

The enduring contribution of BinVis, Veles, and CantorDust is thus not merely the production of striking images. It is the establishment of a practical intermediate representation between uninterpreted bytes and premature semantics: **a visual search surface on which human perception can decide where formal analysis should begin.**

## Primary project sources

- [binvis.io](https://binvis.io/) and Aldo Cortesi's
  [space-filling curve implementation](https://github.com/cortesi/spacecurve).
- [CodiLime Veles source and documentation](https://github.com/codilime/veles)
  (archived upstream).
- [Battelle CantorDust source](https://github.com/Battelle/cantordust) and
  [release background](https://inside.battelle.org/blog-details/battelle-publishes-open-source-binary-visualization-tool).
