#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

DMG_TO_VERIFY="${1:-${STRATA_DMG_FILE}}"
strata_require_command hdiutil

if [[ ! -f "${DMG_TO_VERIFY}" ]]; then
    printf 'DMG is missing: %s\n' "${DMG_TO_VERIFY}" >&2
    exit 1
fi

DMG_BYTES="$(stat -f '%z' "${DMG_TO_VERIFY}")"
MAX_DMG_BYTES="${STRATA_MAX_DMG_BYTES:-26214400}"
if (( DMG_BYTES > MAX_DMG_BYTES )); then
    printf 'DMG exceeds the %s-byte budget: %s bytes\n' \
        "${MAX_DMG_BYTES}" "${DMG_BYTES}" >&2
    exit 1
fi
strata_verify_disk_image "${DMG_TO_VERIFY}"

MOUNT_ROOT="$(mktemp -d /private/tmp/strata-dmg-mount.XXXXXX)"
MOUNT_DEVICE=""
cleanup() {
    if [[ -n "${MOUNT_DEVICE}" ]]; then
        hdiutil detach "${MOUNT_DEVICE}" >/dev/null || true
    fi
    rmdir "${MOUNT_ROOT}" 2>/dev/null || true
}
trap cleanup EXIT

ATTACH_OUTPUT="$(hdiutil attach \
    -readonly \
    -nobrowse \
    -mountpoint "${MOUNT_ROOT}" \
    "${DMG_TO_VERIFY}")"
MOUNT_DEVICE="$(awk '/^\/dev\// { print $1; exit }' <<<"${ATTACH_OUTPUT}")"
if [[ -z "${MOUNT_DEVICE}" ]]; then
    printf 'Could not determine the mounted DMG device.\n' >&2
    exit 1
fi

MOUNTED_APP="${MOUNT_ROOT}/${STRATA_APP_NAME}.app"
if [[ ! -L "${MOUNT_ROOT}/Applications" ]]; then
    printf 'DMG does not contain the Applications install link.\n' >&2
    exit 1
fi
"${SCRIPT_DIRECTORY}/verify-macos-app.sh" "${MOUNTED_APP}"

if strata_is_developer_id_build || [[ "${STRATA_EXPECT_DEVELOPER_ID:-0}" == "1" ]]; then
    codesign --verify --verbose=2 "${DMG_TO_VERIFY}"
fi
if [[ "${STRATA_EXPECT_NOTARIZED:-0}" == "1" ]]; then
    xcrun stapler validate "${DMG_TO_VERIFY}"
    spctl \
        --assess \
        --type open \
        --context context:primary-signature \
        --verbose=4 \
        "${DMG_TO_VERIFY}"
fi

hdiutil detach "${MOUNT_DEVICE}" >/dev/null
MOUNT_DEVICE=""
printf 'Verified %s\n' "${DMG_TO_VERIFY}"
printf '  image: %s bytes / read-only mount / Applications link\n' "${DMG_BYTES}"
