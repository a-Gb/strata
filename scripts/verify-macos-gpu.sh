#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

APP_TO_VERIFY="${1:-${STRATA_APP_BUNDLE}}"
APP_EXECUTABLE="${APP_TO_VERIFY}/Contents/MacOS/${STRATA_EXECUTABLE_NAME}"
if [[ ! -x "${APP_EXECUTABLE}" ]]; then
    printf 'Packaged executable is missing: %s\n' "${APP_EXECUTABLE}" >&2
    exit 1
fi

"${APP_EXECUTABLE}" --gpu-self-test
printf 'Verified packaged Metal compute path.\n'
