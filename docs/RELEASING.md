# Releasing

This checklist covers a public source snapshot and macOS preview release. It
does not by itself authorize publishing, signing with a maintainer identity, or
rewriting already-public repository history.

## Maintainer decisions

- [x] Use `Strata` and `https://github.com/a-Gb/strata` publicly.
- [x] Use GitHub Issues and Discussions for public collaboration.
- [x] Publish GitHub private vulnerability reporting as the security channel.
- [x] Use pre-alpha version `0.1.0` across package and bundle metadata.
- [x] Use bundle identifier `dev.strata.workbench` and signing team
      `2NK7ZR2DY7` for direct distribution.
- [x] Keep workspace crates `publish = false` for `0.1.0`; extension is through
      source contributions and forks.

## Source release gate

- [x] The tree contains no source binaries, private paths, credentials, local
      project locators, sessions, generated videos, or unlicensed fixtures.
- [x] If prototype history has never been published, generated media has been
      removed from history or the public branch has been rebuilt as a clean
      root commit. Never rewrite already-public history casually.
- [x] `README.md`, `CHANGELOG.md`, `SECURITY.md`, and implementation status
      describe the same capabilities and limitations.
- [x] Every schema and example JSON document parses.
- [x] Maintained Markdown passes the repository policy.
- [x] Every committed fixture has a digest, license, generator or source, and
      expected properties.
- [x] No maintained source or documentation file exceeds 1,200 lines.
- [x] Dependency advisories, licenses, bans, and sources pass policy.

The dependency policy is filtered to the supported `aarch64-apple-darwin`
target. The all-target lock graph currently includes the two `quick-xml 0.38.4`
denial-of-service advisories recorded in the implementation status through a
Linux-only desktop dependency path. Upgrade or remove that path, then run a
separate all-target audit before expanding platform support.

Run:

```bash
just advisories-update
just release-check
just validate-video-gallery
just sbom
git diff --check
```

The advisory update is an explicit network step. `just release-check` then uses
the repository-local cache under ignored `target/` without changing it during
the acceptance run.

The SBOM recipe emits one target-specific CycloneDX document per workspace
crate under ignored `target/sbom/`. Inspect those documents outside the release
commit unless the release process explicitly adopts a stable artifact location.

## macOS binary gate

- [ ] Build from a clean checkout at the intended tag.
- [x] Run the CPU/GPU differential on an Apple Silicon Mac using the packaged
      application. Minimum-OS hardware remains part of the clean-machine gate.
- [x] Build `0.1.0` with the hardened runtime and Developer ID Application team
      `2NK7ZR2DY7`.
- [x] Notarize, staple, and Gatekeeper-verify the distributed archive.
- [ ] Verify installation and first launch from a quarantined clean machine.
- [ ] Confirm Gatekeeper launch, file-open, large-file, session reattachment,
      signature-pack, export, and offline behavior.
- [x] Publish checksums, SBOM, source revision, toolchain, SDK, deployment
      target, and known limitations beside the artifact.

Run the full local artifact chain with:

```bash
just dmg
just verify-macos-gpu
just smoke-macos-gui
```

The first command builds the optimized app, verifies its bundle and Mach-O
metadata, creates a compressed DMG plus SHA-256 file, mounts the image
read-only, and verifies the mounted copy. The hardware and GUI gates are
deliberately separate because restricted build agents may not expose Metal or
WindowServer. Default output is ad-hoc signed and remains unsuitable for
public distribution.

The scripts under `scripts/` also implement credential-gated hardened-runtime
Developer ID signing, notarization, stapling, and Gatekeeper verification. See
[macOS packaging](../packaging/macos/README.md) for the generic environment
contract. A public binary release is not accepted until that path has run with
the final identity, version, bundle identifier, and a clean-machine install.

### Current `0.1.0` candidate

- Developer ID signing, secure timestamping, exact team verification, mounted
  DMG verification, Metal differential, and a five-second GUI smoke pass.
- Build `2` declares `Strata.icns` explicitly and avoids the LaunchServices
  identity previously shared with an obsolete iconless build.
- Apple submission `f72d64fd-9db3-4201-b228-aec12874a3be` was accepted on
  August 2, 2026. The ticket is stapled and Gatekeeper reports `Notarized
  Developer ID`.
- Quarantined local copies of the DMG and extracted app passed Gatekeeper, and
  the copied app passed a five-second first-launch smoke. A separate clean Mac
  remains the final installation-environment check.
- The post-staple SHA-256 digest is
  `1254d1c2ff27e32ac78596e878afcfcce605c7d3c8f7d65722b0ed995febdc39`.
- The current Developer ID certificate expires on February 1, 2027 and should
  be replaced with a G2-issued certificate before that date.

## After publication

- [ ] Tag the exact reviewed commit and avoid rebuilding the same version.
- [ ] Verify that public archives contain both license texts.
- [ ] Re-run the install and first-launch smoke test from the published artifact.
- [ ] Record release notes and any security-relevant dependency exceptions.
- [ ] Keep generated demonstrations reproducible from checked programs and
      synthetic fixtures instead of committing renderer output.
