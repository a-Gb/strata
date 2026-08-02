#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

strata_validate_release_settings
strata_require_command cargo
strata_require_command codesign
strata_require_command plutil
strata_require_command /usr/libexec/PlistBuddy

if [[ "${STRATA_TARGET_TRIPLE}" != "aarch64-apple-darwin" ]]; then
    printf 'Only the supported Apple Silicon target may be packaged: %s\n' \
        "${STRATA_TARGET_TRIPLE}" >&2
    exit 1
fi

if [[ ! -f "${STRATA_INFO_PLIST}" ]]; then
    printf 'macOS packaging metadata is incomplete.\n' >&2
    exit 1
fi

mkdir -p "${STRATA_REPO_ROOT}/target" "${STRATA_ARTIFACT_DIR}"

export MACOSX_DEPLOYMENT_TARGET="${STRATA_DEPLOYMENT_TARGET}"
cargo build \
    --manifest-path "${STRATA_REPO_ROOT}/Cargo.toml" \
    --locked \
    --profile "${STRATA_BUILD_PROFILE}" \
    --target "${STRATA_TARGET_TRIPLE}" \
    -p strata-app-macos

BUILT_EXECUTABLE="$(strata_profile_binary)"
if [[ ! -x "${BUILT_EXECUTABLE}" ]]; then
    printf 'Expected executable was not built: %s\n' "${BUILT_EXECUTABLE}" >&2
    exit 1
fi

STAGING_ROOT="$(mktemp -d "${STRATA_REPO_ROOT}/target/.strata-app-stage.XXXXXX")"
STAGED_APP="${STAGING_ROOT}/${STRATA_APP_NAME}.app"
cleanup() {
    rm -rf -- "${STAGING_ROOT}"
}
trap cleanup EXIT

mkdir -p \
    "${STAGED_APP}/Contents/MacOS" \
    "${STAGED_APP}/Contents/Resources/Licenses"
/usr/bin/ditto --noqtn "${BUILT_EXECUTABLE}" \
    "${STAGED_APP}/Contents/MacOS/${STRATA_EXECUTABLE_NAME}"
chmod 0755 "${STAGED_APP}/Contents/MacOS/${STRATA_EXECUTABLE_NAME}"
/usr/bin/ditto --noqtn "${STRATA_INFO_PLIST}" "${STAGED_APP}/Contents/Info.plist"
/usr/bin/ditto --noqtn "${STRATA_REPO_ROOT}/LICENSE-MIT" \
    "${STAGED_APP}/Contents/Resources/Licenses/LICENSE-MIT"
/usr/bin/ditto --noqtn "${STRATA_REPO_ROOT}/LICENSE-APACHE" \
    "${STAGED_APP}/Contents/Resources/Licenses/LICENSE-APACHE"

PLIST_BUDDY=/usr/libexec/PlistBuddy
"${PLIST_BUDDY}" -c "Set :CFBundleIdentifier ${STRATA_BUNDLE_ID}" \
    "${STAGED_APP}/Contents/Info.plist"
"${PLIST_BUDDY}" -c "Set :CFBundleShortVersionString ${STRATA_MARKETING_VERSION}" \
    "${STAGED_APP}/Contents/Info.plist"
"${PLIST_BUDDY}" -c "Set :CFBundleVersion ${STRATA_BUILD_NUMBER}" \
    "${STAGED_APP}/Contents/Info.plist"
"${PLIST_BUDDY}" -c "Set :LSMinimumSystemVersion ${STRATA_DEPLOYMENT_TARGET}" \
    "${STAGED_APP}/Contents/Info.plist"

ICON_SOURCE="${STRATA_REPO_ROOT}/packaging/macos/Strata.icns"
if [[ ! -f "${ICON_SOURCE}" ]]; then
    printf 'Release icon is missing: %s\n' "${ICON_SOURCE}" >&2
    exit 1
fi
/usr/bin/ditto --noqtn "${ICON_SOURCE}" \
    "${STAGED_APP}/Contents/Resources/Strata.icns"
"${PLIST_BUDDY}" -c "Set :CFBundleIconFile Strata.icns" \
    "${STAGED_APP}/Contents/Info.plist"

plutil -lint "${STAGED_APP}/Contents/Info.plist" >/dev/null
if strata_is_developer_id_build; then
    codesign \
        --force \
        --options runtime \
        --timestamp \
        --sign "${STRATA_SIGNING_IDENTITY}" \
        "${STAGED_APP}"
else
    codesign \
        --force \
        --sign - \
        "${STAGED_APP}"
fi
codesign --verify --deep --strict --verbose=2 "${STAGED_APP}"

strata_replace_generated_directory "${STAGED_APP}" "${STRATA_APP_BUNDLE}"
printf 'Packaged %s\n' "${STRATA_APP_BUNDLE}"
printf '  version: %s (%s)\n' "${STRATA_MARKETING_VERSION}" "${STRATA_BUILD_NUMBER}"
printf '  target: %s / macOS %s+\n' \
    "${STRATA_TARGET_TRIPLE}" "${STRATA_DEPLOYMENT_TARGET}"
printf '  signing: %s\n' "${STRATA_SIGNING_IDENTITY}"
