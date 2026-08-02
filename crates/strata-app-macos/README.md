# strata-app-macos

This crate is the thin production composition root for the reusable workbench
implemented by the `strata-poc` package's `strata_workbench` library. It owns
the production process identity while source, tiling, digest, comparison, and
analysis orchestration remain behind `strata-runtime`.

Run
`cargo run -p strata-app-macos -- [SOURCE | PROJECT.strata-project | SESSION_DIRECTORY]`.
Local projects restore the source-free session checkpoint, pinned signature
knowledge, page, exact ranges, analytical controls, and camera after digest-
verified source reattachment. The legacy `strata-poc` binary remains available
as a compatibility and experiment host.
