#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

APP_TO_VERIFY="${1:-${STRATA_APP_BUNDLE}}"
INFO_PLIST="${APP_TO_VERIFY}/Contents/Info.plist"

strata_validate_release_settings
strata_require_command codesign
strata_require_command lipo
strata_require_command plutil
strata_require_command strings
strata_require_command xcrun

if [[ ! -d "${APP_TO_VERIFY}" || ! -f "${INFO_PLIST}" ]]; then
    printf 'App bundle is missing or incomplete: %s\n' "${APP_TO_VERIFY}" >&2
    exit 1
fi

plutil -lint "${INFO_PLIST}" >/dev/null
ACTUAL_EXECUTABLE="$(strata_plist_value "${INFO_PLIST}" CFBundleExecutable)"
APP_EXECUTABLE="${APP_TO_VERIFY}/Contents/MacOS/${ACTUAL_EXECUTABLE}"
if [[ ! -x "${APP_EXECUTABLE}" ]]; then
    printf 'Bundle executable is missing: %s\n' "${APP_EXECUTABLE}" >&2
    exit 1
fi

assert_equal() {
    local label="$1"
    local actual_value="$2"
    local expected_value="$3"
    if [[ "${actual_value}" != "${expected_value}" ]]; then
        printf '%s mismatch: expected %s, found %s\n' \
            "${label}" "${expected_value}" "${actual_value}" >&2
        exit 1
    fi
}

assert_equal "bundle executable" "${ACTUAL_EXECUTABLE}" "${STRATA_EXECUTABLE_NAME}"
assert_equal \
    "bundle identifier" \
    "$(strata_plist_value "${INFO_PLIST}" CFBundleIdentifier)" \
    "${STRATA_BUNDLE_ID}"
assert_equal \
    "marketing version" \
    "$(strata_plist_value "${INFO_PLIST}" CFBundleShortVersionString)" \
    "${STRATA_MARKETING_VERSION}"
assert_equal \
    "build number" \
    "$(strata_plist_value "${INFO_PLIST}" CFBundleVersion)" \
    "${STRATA_BUILD_NUMBER}"
assert_equal \
    "minimum system version" \
    "$(strata_plist_value "${INFO_PLIST}" LSMinimumSystemVersion)" \
    "${STRATA_DEPLOYMENT_TARGET}"
assert_equal \
    "bundle icon" \
    "$(strata_plist_value "${INFO_PLIST}" CFBundleIconFile)" \
    "Strata.icns"
if [[ ! -f "${APP_TO_VERIFY}/Contents/Resources/Strata.icns" ]]; then
    printf 'Bundle icon resource is missing.\n' >&2
    exit 1
fi

codesign --verify --deep --strict --verbose=2 "${APP_TO_VERIFY}"

ARCHITECTURES="$(lipo -archs "${APP_EXECUTABLE}")"
assert_equal "executable architecture" "${ARCHITECTURES}" "arm64"

MACHO_MINIMUM="$(xcrun vtool -show-build "${APP_EXECUTABLE}" | awk '$1 == "minos" { print $2; exit }')"
if [[ -z "${MACHO_MINIMUM}" ]]; then
    printf 'Could not read the Mach-O deployment target.\n' >&2
    exit 1
fi
assert_equal "Mach-O deployment target" "${MACHO_MINIMUM}" "${STRATA_DEPLOYMENT_TARGET}"

EXECUTABLE_BYTES="$(stat -f '%z' "${APP_EXECUTABLE}")"
MAX_EXECUTABLE_BYTES="${STRATA_MAX_EXECUTABLE_BYTES:-20971520}"
if (( EXECUTABLE_BYTES > MAX_EXECUTABLE_BYTES )); then
    printf 'Executable exceeds the %s-byte budget: %s bytes\n' \
        "${MAX_EXECUTABLE_BYTES}" "${EXECUTABLE_BYTES}" >&2
    exit 1
fi

if strings -a "${APP_EXECUTABLE}" | grep -E -m 1 '/Users/[^/[:space:]]+|/var/folders/' >/dev/null; then
    printf 'Packaged executable leaks a local absolute path.\n' >&2
    exit 1
fi

"${APP_EXECUTABLE}" --help >/dev/null
"${APP_EXECUTABLE}" \
    --validate-program \
    "${STRATA_REPO_ROOT}/examples/video/firmware-stratigraphy.json" >/dev/null

GPU_STATUS="not requested"
if [[ "${STRATA_RUN_GPU_SELF_TEST:-0}" == "1" ]]; then
    "${APP_EXECUTABLE}" --gpu-self-test
    GPU_STATUS="passed"
fi

if strata_is_developer_id_build || [[ "${STRATA_EXPECT_DEVELOPER_ID:-0}" == "1" ]]; then
    SIGNATURE_DETAILS="$(codesign --display --verbose=4 "${APP_TO_VERIFY}" 2>&1)"
    if grep -q '^Signature=adhoc$' <<<"${SIGNATURE_DETAILS}"; then
        printf 'Expected a Developer ID signature, found an ad-hoc signature.\n' >&2
        exit 1
    fi
    if ! grep -q 'flags=.*runtime' <<<"${SIGNATURE_DETAILS}"; then
        printf 'Hardened runtime is missing from the app signature.\n' >&2
        exit 1
    fi
    if ! grep -q "^TeamIdentifier=${STRATA_TEAM_ID}$" <<<"${SIGNATURE_DETAILS}"; then
        printf 'App signature does not belong to team %s.\n' "${STRATA_TEAM_ID}" >&2
        exit 1
    fi
    if codesign --display --entitlements - "${APP_TO_VERIFY}" 2>&1 \
        | grep -q 'com.apple.security.get-task-allow'; then
        printf 'Release signature contains the debug get-task-allow entitlement.\n' >&2
        exit 1
    fi
fi

if [[ "${STRATA_EXPECT_NOTARIZED:-0}" == "1" ]]; then
    spctl --assess --type execute --verbose=4 "${APP_TO_VERIFY}"
fi

printf 'Verified %s\n' "${APP_TO_VERIFY}"
printf '  executable: %s bytes / arm64 / macOS %s+\n' \
    "${EXECUTABLE_BYTES}" "${MACHO_MINIMUM}"
printf '  headless: help and program validation passed\n'
printf '  GPU differential: %s\n' "${GPU_STATUS}"
