#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/macos-release-lib.sh
source "${SCRIPT_DIRECTORY}/macos-release-lib.sh"

APP_TO_SMOKE="${1:-${STRATA_APP_BUNDLE}}"
APP_EXECUTABLE="${APP_TO_SMOKE}/Contents/MacOS/${STRATA_EXECUTABLE_NAME}"
SMOKE_SECONDS="${STRATA_GUI_SMOKE_SECONDS:-5}"
if [[ ! -x "${APP_EXECUTABLE}" ]]; then
    printf 'Packaged executable is missing: %s\n' "${APP_EXECUTABLE}" >&2
    exit 1
fi
if [[ ! "${SMOKE_SECONDS}" =~ ^[0-9]+$ ]] \
    || (( SMOKE_SECONDS < 1 || SMOKE_SECONDS > 30 )); then
    printf 'STRATA_GUI_SMOKE_SECONDS must be between 1 and 30.\n' >&2
    exit 1
fi

mkdir -p "${STRATA_ARTIFACT_DIR}"
SMOKE_LOG="${STRATA_ARTIFACT_DIR}/gui-smoke.log"
: >"${SMOKE_LOG}"
"${APP_EXECUTABLE}" >"${SMOKE_LOG}" 2>&1 &
GUI_PID=$!

cleanup() {
    if kill -0 "${GUI_PID}" 2>/dev/null; then
        kill -TERM "${GUI_PID}" 2>/dev/null || true
        wait "${GUI_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT

for (( second = 1; second <= SMOKE_SECONDS; second += 1 )); do
    sleep 1
    if ! kill -0 "${GUI_PID}" 2>/dev/null; then
        set +e
        wait "${GUI_PID}"
        EXIT_STATUS=$?
        set -e
        printf 'GUI exited during smoke test with status %s.\n' "${EXIT_STATUS}" >&2
        sed -n '1,120p' "${SMOKE_LOG}" >&2
        exit 1
    fi
done

kill -TERM "${GUI_PID}"
set +e
wait "${GUI_PID}"
EXIT_STATUS=$?
set -e
GUI_PID=""
if [[ "${EXIT_STATUS}" != "0" && "${EXIT_STATUS}" != "143" ]]; then
    printf 'GUI shutdown returned unexpected status %s.\n' "${EXIT_STATUS}" >&2
    sed -n '1,120p' "${SMOKE_LOG}" >&2
    exit 1
fi

printf 'Verified packaged GUI remained alive for %s seconds.\n' "${SMOKE_SECONDS}"
