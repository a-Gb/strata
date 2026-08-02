# Security policy

Strata analyzes attacker-controlled bytes and is pre-alpha. It should be used in
an isolated environment appropriate to the material being examined.

## Supported versions

Strata is pre-alpha and does not yet carry a production security-support
commitment. Security fixes are applied on a best-effort basis to the current
preview and development branch.

| Version | Support |
|---|---|
| `0.1.x` | Current pre-alpha preview; best-effort fixes |
| Earlier snapshots | Unsupported |

## Reporting a vulnerability

Do not publish exploit details, credentials, private paths, proprietary samples,
or live malware in a public issue. Use
[GitHub private vulnerability reporting](https://github.com/a-Gb/strata/security/advisories/new).
If that channel is unavailable, open a minimal non-sensitive issue asking a
maintainer to establish contact.

A useful report includes the affected revision, platform, minimal synthetic
reproducer, expected boundary, observed behavior, and whether source bytes or
local paths may have escaped. Do not include a sensitive source file merely to
make the report reproducible.

## Current guarantees

- Local sources are opened read-only and are never modified in place.
- Source requests are generation checked and bounded by explicit byte budgets.
- Large-file overviews use bounded resident tiles instead of whole-file
  allocation.
- Portable session bundles exclude source bytes and local paths and require an
  exact length plus SHA-256 match before reattachment.
- Signature packs are parsed under fixed size and pattern limits; unsupported
  records are rejected and counted.
- WGPU work is resource bounded, differential checked against CPU semantics, and
  has an explicit fallback.
- The application has no telemetry or cloud-analysis dependency.

## Important non-guarantees

- The downloadable macOS preview is hardened, Developer ID signed, and Apple
  notarized. The application is not sandboxed, fuzz-certified, or independently
  audited.
- The current UI host does not yet provide complete native document
  authorization or device-loss recovery.
- Dependency policy currently covers the supported Apple Silicon target. The
  complete cross-platform lock graph includes a Linux-only `quick-xml 0.38.4`
  path affected by RUSTSEC-2026-0194 and RUSTSEC-2026-0195; Linux is not a
  supported target until that path is upgraded or removed.
- Session integrity hashes do not provide authenticity against an adversary who
  can rewrite both the bundle and its hashes.
- Third-party plugin execution is not enabled. The WASM capability model in the
  architecture documents is a planned boundary, not a deployed sandbox.
- Heuristics, entropy, signatures, and visual similarity are evidence cues, not
  file-type, encryption, compression, or malware verdicts.
- Video export launches a local `ffmpeg` process when requested.

## Handling sensitive inputs

- Prefer deterministic synthetic or legally redistributable fixtures for bug
  reports and tests.
- Keep restricted corpora outside the repository and refer to them by digest.
- Review `.strata-project` files before sharing; they may contain local paths.
- Prefer source-free `.strata-session` bundles for collaboration, but inspect
  their manifest and journal before disclosure.
- Assume screenshots and exports may reveal source-derived structure even when
  they contain no literal source bytes.

## Security work required before a stable, security-supported release

- Fuzz source, schema, session, signature, and parser boundaries.
- Complete native sandbox/authorization and GPU device-loss acceptance.
- Generate and review an SBOM, advisory report, and license report.
- Complete clean-machine and hostile-input acceptance for distributed
  artifacts.
- Establish a documented response-time and supported-version commitment.
- Establish update provenance and rollback behavior.
