#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

NOTARY_PROFILE="${STRATA_NOTARY_PROFILE:-strata-notary}"
if ! strata_is_developer_id_build; then
    printf 'Refusing to notarize an ad-hoc signed image. Set STRATA_SIGNING_IDENTITY.\n' >&2
    exit 1
fi
if [[ ! -f "${STRATA_DMG_FILE}" ]]; then
    printf 'DMG is missing: %s\n' "${STRATA_DMG_FILE}" >&2
    exit 1
fi

STRATA_EXPECT_DEVELOPER_ID=1 \
    "${SCRIPT_DIRECTORY}/verify-macos-dmg.sh" "${STRATA_DMG_FILE}"
xcrun notarytool submit \
    "${STRATA_DMG_FILE}" \
    --keychain-profile "${NOTARY_PROFILE}" \
    --wait
xcrun stapler staple "${STRATA_DMG_FILE}"
xcrun stapler validate "${STRATA_DMG_FILE}"
STRATA_EXPECT_DEVELOPER_ID=1 \
STRATA_EXPECT_NOTARIZED=1 \
    "${SCRIPT_DIRECTORY}/verify-macos-dmg.sh" "${STRATA_DMG_FILE}"

printf 'Notarized and stapled %s\n' "${STRATA_DMG_FILE}"
