#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

strata_validate_release_settings
strata_require_command codesign
strata_require_command hdiutil

if [[ ! -d "${STRATA_APP_BUNDLE}" ]]; then
    printf 'Package and verify the app before creating a DMG: %s\n' \
        "${STRATA_APP_BUNDLE}" >&2
    exit 1
fi

mkdir -p "${STRATA_REPO_ROOT}/target" "${STRATA_ARTIFACT_DIR}"
STAGING_ROOT="$(mktemp -d "${STRATA_REPO_ROOT}/target/.strata-dmg-stage.XXXXXX")"
DMG_ROOT="$(mktemp -d "${STRATA_REPO_ROOT}/target/.strata-dmg-output.XXXXXX")"
STAGED_DMG="${DMG_ROOT}/${STRATA_APP_NAME}.dmg"
cleanup() {
    rm -rf -- "${STAGING_ROOT}" "${DMG_ROOT}"
}
trap cleanup EXIT

/usr/bin/ditto --noqtn "${STRATA_APP_BUNDLE}" \
    "${STAGING_ROOT}/${STRATA_APP_NAME}.app"
ln -s /Applications "${STAGING_ROOT}/Applications"

hdiutil create \
    -volname "${STRATA_APP_NAME}" \
    -srcfolder "${STAGING_ROOT}" \
    -format UDZO \
    -imagekey zlib-level=9 \
    -ov \
    "${STAGED_DMG}"

if strata_is_developer_id_build; then
    codesign --force --timestamp --sign "${STRATA_SIGNING_IDENTITY}" "${STAGED_DMG}"
    codesign --verify --verbose=2 "${STAGED_DMG}"
fi
strata_verify_disk_image "${STAGED_DMG}"

strata_replace_generated_file "${STAGED_DMG}" "${STRATA_DMG_FILE}"
CHECKSUM_FILE="${STRATA_DMG_FILE}.sha256"
strata_assert_generated_path "${CHECKSUM_FILE}"
printf '%s  %s\n' \
    "$(strata_sha256 "${STRATA_DMG_FILE}")" \
    "$(basename "${STRATA_DMG_FILE}")" >"${CHECKSUM_FILE}"

printf 'Packaged %s\n' "${STRATA_DMG_FILE}"
printf 'Checksum %s\n' "${CHECKSUM_FILE}"
