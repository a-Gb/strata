#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

APP_TO_BENCHMARK="${1:-${STRATA_APP_BUNDLE}}"
APP_EXECUTABLE="${APP_TO_BENCHMARK}/Contents/MacOS/${STRATA_EXECUTABLE_NAME}"
if [[ ! -x "${APP_EXECUTABLE}" ]]; then
    printf 'Packaged executable is missing: %s\n' "${APP_EXECUTABLE}" >&2
    exit 1
fi

EXECUTABLE_BYTES="$(stat -f '%z' "${APP_EXECUTABLE}")"
printf 'Executable: %s bytes\n' "${EXECUTABLE_BYTES}"

if command -v hyperfine >/dev/null 2>&1; then
    mkdir -p "${STRATA_ARTIFACT_DIR}"
    hyperfine \
        --shell=none \
        --warmup 3 \
        --runs 20 \
        --export-json "${STRATA_ARTIFACT_DIR}/startup-benchmark.json" \
        "'${APP_EXECUTABLE}' --help" \
        "'${APP_EXECUTABLE}' --validate-program '${STRATA_REPO_ROOT}/examples/video/firmware-stratigraphy.json'"
else
    printf 'hyperfine not installed; skipped startup timing.\n'
fi

if command -v cargo-bloat >/dev/null 2>&1; then
    cargo bloat \
        --manifest-path "${STRATA_REPO_ROOT}/Cargo.toml" \
        --profile "${STRATA_BUILD_PROFILE}" \
        --target "${STRATA_TARGET_TRIPLE}" \
        --target-dir "${STRATA_REPO_ROOT}/target/bloat" \
        -p strata-app-macos \
        --crates \
        -n 15
else
    printf 'cargo-bloat not installed; skipped size attribution.\n'
fi
