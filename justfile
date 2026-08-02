# Repository acceptance recipes. Strict documentation and Clippy policy is workspace-clean.

check-lines:
    bash scripts/check-file-lines.sh

shell-check:
    bash -n scripts/*.sh
    if command -v shellcheck >/dev/null 2>&1; then shellcheck scripts/*.sh; fi

check: check-lines
    cargo check --workspace --all-targets

lint: check-lines
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

lint-poc: check-lines
    cargo fmt --all -- --check
    cargo clippy -p strata-poc --all-targets --no-deps -- -D warnings

test: check-lines
    cargo test --workspace

docs-check:
    markdownlint --ignore target --ignore output --ignore .git "**/*.md"

rust-docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

release-check: check lint test docs-check rust-docs shell-check
    cargo metadata --locked --no-deps --format-version 1 > /dev/null
    cargo deny check --disable-fetch --hide-inclusion-graph
    git diff --check

advisories-update:
    cargo deny fetch

sbom:
    bash scripts/generate-sboms.sh

macos-icon:
    bash scripts/generate-macos-icon.sh

package-macos: check-lines
    bash scripts/package-macos-app.sh

verify-macos-app: package-macos
    bash scripts/verify-macos-app.sh

verify-macos-gpu: package-macos
    bash scripts/verify-macos-gpu.sh

smoke-macos-gui: package-macos
    bash scripts/smoke-macos-gui.sh

package-dmg: verify-macos-app
    bash scripts/package-macos-dmg.sh

verify-dmg: package-dmg
    bash scripts/verify-macos-dmg.sh

dmg: verify-dmg

benchmark-macos: package-macos
    bash scripts/benchmark-macos-release.sh

notarize-dmg: verify-dmg
    bash scripts/notarize-macos-dmg.sh

release-macos-local: release-check validate-video-gallery sbom verify-dmg verify-macos-gpu smoke-macos-gui

package-poc: check-lines
    cargo build --release -p strata-poc
    mkdir -p "target/Strata POC.app/Contents/MacOS"
    cp target/release/strata-poc "target/Strata POC.app/Contents/MacOS/strata-poc"
    cp packaging/macos/StrataPoc-Info.plist "target/Strata POC.app/Contents/Info.plist"
    codesign --force --deep --sign - "target/Strata POC.app"

render-poc-video program:
    cargo run -p strata-poc -- --render-program "{{program}}"

video-fixtures:
    cargo run -p strata-test-support --example write_video_fixtures

list-video-presets:
    cargo run -p strata-poc -- --list-video-presets

validate-video-gallery:
    cargo run -p strata-poc -- --validate-program examples/video/firmware-stratigraphy.json
    cargo run -p strata-poc -- --validate-program examples/video/xor-correlation-reveal.json
    cargo run -p strata-poc -- --validate-program examples/video/interleave-lattice.json
    cargo run -p strata-poc -- --validate-program examples/video/bitplane-blueprint.json

render-video-gallery:
    cargo run -p strata-poc -- --render-program examples/video/firmware-stratigraphy.json
    cargo run -p strata-poc -- --render-program examples/video/xor-correlation-reveal.json
    cargo run -p strata-poc -- --render-program examples/video/interleave-lattice.json
    cargo run -p strata-poc -- --render-program examples/video/bitplane-blueprint.json
