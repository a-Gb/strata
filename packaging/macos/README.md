# macOS packaging

The packaging scripts produce an Apple Silicon app and compressed DMG under
ignored `target/artifacts/`. Local artifacts are ad-hoc signed by default; they
are suitable for development smoke tests, not public distribution.

## Local image

```bash
just dmg
open target/artifacts/Strata-0.1.0-arm64.dmg
```

The `dmg` recipe builds with the `dist` Cargo profile, enforces a macOS 15.0
Mach-O deployment target, verifies the app, creates the image, mounts it
read-only, and verifies the mounted copy. It also writes a sibling SHA-256
file. `just benchmark-macos` records headless startup timing when `hyperfine`
is installed. `just verify-macos-gpu` is a separate hardware gate because
restricted or headless build sessions may not expose a Metal adapter.
`just smoke-macos-gui` launches the packaged executable for five seconds and
fails if it exits early; it likewise requires an active macOS GUI session.

The image contains `Strata.app` and an `/Applications` link. The checked-in
`Strata.icns` is generated from the reviewed 1024-pixel master; see
[icon provenance](ICON.md).

## Developer ID image

The selected `0.1.0` identity must be installed in the login keychain:

```bash
STRATA_SIGNING_IDENTITY="Developer ID Application: Grant Hodgeon (2NK7ZR2DY7)" \
  just dmg
```

A non-ad-hoc identity enables the hardened runtime, a secure timestamp, and
Developer ID checks. The release currently grants no entitlements; any future
addition requires a security review.

## Notarization

Apple's developer agreements must be current. Only the Apple Developer Account
Holder can accept them. Check the
[developer account landing page](https://developer.apple.com/account/) first
and accept any updated Apple Developer Program License Agreement. Apple pauses
access to the Mac notary service while that agreement is outstanding. If the
account directs you to App Store Connect, also check **Business -> Agreements**
and resolve every required action:

<https://developer.apple.com/help/app-store-connect/manage-agreements/view-agreements-status>

Store an App Store Connect API key in the default keychain profile without
committing the key or identifiers:

```bash
xcrun notarytool store-credentials strata-notary \
  --key /secure/path/AuthKey_KEYID.p8 \
  --key-id KEYID \
  --issuer ISSUER_UUID
```

Then run the explicit submission recipe with the same signing identity:

```bash
STRATA_SIGNING_IDENTITY="Developer ID Application: Grant Hodgeon (2NK7ZR2DY7)" \
  just notarize-dmg
```

This command submits to Apple, waits for acceptance, staples the ticket, and
re-runs Gatekeeper checks. It is intentionally separate from local packaging
because it changes external state and requires maintainer credentials.

The local `strata-notary` profile is configured for this repository. An HTTP
403 mentioning an agreement is not conclusive: compare the profile with a
direct Team API key request. If the direct request succeeds, overwrite the
stale profile using `store-credentials --validate`. Individual API keys cannot
use `notarytool`; use a Team Key and never commit its private key.

The currently installed Developer ID certificate expires on February 1, 2027.
Replace it with a G2-issued certificate before that deadline.

## Overrides

The scripts accept these environment variables:

| Variable | Default |
|---|---|
| `STRATA_MARKETING_VERSION` | Workspace version |
| `STRATA_BUILD_NUMBER` | `2` |
| `STRATA_BUNDLE_ID` | `dev.strata.workbench` |
| `STRATA_TEAM_ID` | `2NK7ZR2DY7` |
| `STRATA_DEPLOYMENT_TARGET` | `15.0` |
| `STRATA_SIGNING_IDENTITY` | `-` (ad-hoc) |
| `STRATA_NOTARY_PROFILE` | `strata-notary` |

Generated destinations are restricted to this repository's `target/`
directory so a malformed override cannot replace an unrelated path.
