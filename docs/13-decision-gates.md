# Decision gates and open questions

Recommended defaults allow pre-alpha implementation to proceed. The rows marked
for public release remain maintainer decisions and are repeated in the
[release checklist](RELEASING.md).

| Decision | Recommended default | Revisit when |
|---|---|---|
| Product name | `Strata` as working codename | Trademark/repository/package check before public release |
| Minimum macOS | macOS 15+; validate current macOS separately | A required API or support burden changes the floor |
| Distribution | Direct notarized app first | A sandboxed/App Store edition has clear demand |
| Window/UI stack | AppKit/`objc2` host + `wgpu`; evaluate `egui` for chrome | Spike fails accessibility, IME, docking, or native behavior |
| Rendering backend | `wgpu` Metal | Measured bottleneck proves a quarantined Metal fast path |
| Source mutation | Immutable/read-only only | A separate editor product or copy-on-write mode is designed |
| Session storage | SQLite + append-only journal + content artifacts | Simpler files prove equally recoverable and queryable |
| Plugin runtime | Wasmtime Component Model/WIT | Runtime footprint or compatibility fails measured needs |
| Native plugins | Not supported externally | A critical integration cannot fit WASM/helper process |
| Third-party shaders | Disabled | Validated declarative shader package has a strong use case |
| Machine learning | Optional local reranking only | Deterministic baseline and adversarial evaluation exist |
| Parser architecture | WASM/helper isolation where possible | Trusted native parser is demonstrably necessary |
| Ghidra integration | Local bridge, not embedded Ghidra | User workflow proves tighter integration is required |
| 3D default | Opt-in | Task studies show it improves discovery over 2D |
| Telemetry | None by default | Explicit privacy-preserving opt-in is justified |
| Cross-platform | Core stays portable; UI macOS-first | macOS product reaches architectural stability |

## Product questions for later user validation

1. Should the first credible release prioritize firmware/disk images, executable RE, or arbitrary creative data exploration?
2. Is direct process-memory capture in scope, or should Strata consume snapshots produced by other tools?
3. Which external handoff is most important first: Ghidra, rizin, LLDB, a command template, or a generic local API?
4. Should sessions be designed for formal forensic chain-of-custody, or for technically reproducible analyst notes without legal-evidence claims?
5. Is corpus indexing a primary workflow or a later research mode?
6. Should expressive outputs—3D, animation, sonification—ship in the main app or as first-party plugins?
7. What is the minimum hardware floor: 8 GB M1, 16 GB M1/M2, or a higher professional baseline?

## Architecture kill criteria

A choice should be replaced if it violates one of these:

- cannot map visible evidence back to exact or explicitly aggregate ranges;
- blocks main-thread interaction on large sources;
- requires loading the whole source for an overview;
- makes CPU/GPU semantics impossible to compare;
- grants third-party code ambient source/filesystem/network access;
- makes sessions dependent on transient GPU/OS handles;
- prevents source-free reproducibility;
- silently changes analytical output under memory pressure;
- prevents a failed analyzer/plugin/GPU device from degrading locally;
- forces Metal-specific code into domain or analyzer contracts.
