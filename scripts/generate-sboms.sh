#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-cyclonedx >/dev/null 2>&1; then
    printf '%s\n' \
        'cargo-cyclonedx is required; install it with:' \
        '  cargo install cargo-cyclonedx --locked' >&2
    exit 127
fi

sbom_directory="${STRATA_SBOM_DIRECTORY:-target/sbom}"
mkdir -p "$sbom_directory"

expected_count=$(rg --files crates -g Cargo.toml | wc -l | tr -d '[:space:]')
cargo cyclonedx \
    -qq \
    --format json \
    --spec-version 1.5 \
    --target aarch64-apple-darwin \
    --target-in-filename \
    --license-strict

sbom_count=0
while IFS= read -r generated_file; do
    mv "$generated_file" "$sbom_directory/"
    sbom_count=$((sbom_count + 1))
done < <(
    rg --files crates -g '*_aarch64-apple-darwin.cdx.json' | LC_ALL=C sort
)

if [ "$sbom_count" -ne "$expected_count" ]; then
    printf 'Expected %d workspace SBOMs, moved %d.\n' \
        "$expected_count" "$sbom_count" >&2
    exit 1
fi

printf 'Generated %d CycloneDX SBOM files in %s.\n' \
    "$sbom_count" "$sbom_directory"
