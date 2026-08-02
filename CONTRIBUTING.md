# Contributing to Strata

Strata is pre-alpha software for examining untrusted binary data. Contributions
are welcome, but analytical correctness, bounded resource use, and exact source
provenance take priority over feature count or visual novelty.

## Before opening a change

- Read the [architecture](docs/01-architecture.md),
  [domain model](docs/02-domain-model.md), and
  [implementation status](docs/20-implementation-status.md).
- Discuss broad contract, schema, plugin, security, or rendering changes before
  implementing them. Small fixes and focused tests can go directly to review.
- Never attach proprietary binaries, credentials, personal paths, or live
  malware to an issue or pull request.

## Development setup

The supported development target is Apple Silicon running macOS 15 or newer.
Install Rust and `just`; the checked-in toolchain file selects stable Rust with
`rustfmt`, Clippy, and the `aarch64-apple-darwin` target.

```bash
just check
just lint
just test
```

Fork the repository, create a focused branch, and keep commits small enough to
review:

```bash
git clone https://github.com/YOUR-ACCOUNT/strata.git
cd strata
git switch -c feature/short-description
```

Use [GitHub Discussions](https://github.com/a-Gb/strata/discussions) for broad
projection, format, or architecture ideas. Use
[GitHub Issues](https://github.com/a-Gb/strata/issues) for scoped bugs and
proposals with a reproducible acceptance boundary.

Optional release checks require `cargo-deny`, `cargo-cyclonedx`, and the
`markdownlint` CLI:

```bash
just advisories-update
just release-check
just sbom
```

## Change requirements

- Keep source reads immutable and explicitly bounded.
- Map every rendered or derived datum to exact or explicitly sampled source
  ranges and transformation parameters.
- Keep results deterministic for identical source bytes and configuration.
- Preserve CPU reference behavior for GPU work and add differential tests.
- Return typed errors; workspace policy denies unsafe code, `unwrap`, `expect`,
  and `panic`.
- Keep maintained source and documentation files at or below 1,200 lines.
- Update schemas, fixtures, documentation, and semantic goldens in the same
  change when a public contract changes.

## Fixtures

Use deterministic synthetic data whenever possible. Every committed fixture
must record its digest, origin or generator, license, expected properties, and
test consumers in [fixtures/README.md](fixtures/README.md). Restricted corpora
belong outside the repository and may be referenced only by digest.

## Review-ready changes

A change is ready when it has a focused rationale, tests proportional to risk,
clean formatting and Clippy output, no unrelated generated files, and updated
public documentation. Visible UI changes should include a compact screenshot or
recording generated from a redistributable fixture.

Unless explicitly stated otherwise, contributions intentionally submitted for
inclusion are licensed under the repository's MIT OR Apache-2.0 terms.
