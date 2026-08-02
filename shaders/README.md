# Shader status

`p1_projection.wgsl` is compiled and dispatched by `strata-gpu` for Alignment
and Hamming projection coordinates. Its output is differential-tested against
the deterministic CPU reference before the application reports the GPU path as
available.

The other WGSL files currently describe planned binding shapes and dispatch
responsibilities. Their `TODO` bodies are not wired into the application and
must not be described as implemented kernels.

Rules for promotion:

- keep raw integer statistics whenever possible;
- validate dimensions and checked byte arithmetic on the host;
- record shader/semantics digests in provenance;
- compare every core kernel against a CPU reference;
- use bounded, reusable staging/readback pools;
- never expose backend-passthrough shader creation to external plugins.
