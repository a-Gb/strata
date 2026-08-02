#!/usr/bin/env bash

set -euo pipefail

STRATA_REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STRATA_APP_NAME="Strata"
STRATA_EXECUTABLE_NAME="strata-app-macos"
STRATA_TARGET_TRIPLE="${STRATA_TARGET_TRIPLE:-aarch64-apple-darwin}"
STRATA_BUILD_PROFILE="${STRATA_BUILD_PROFILE:-dist}"
STRATA_DEPLOYMENT_TARGET="${STRATA_DEPLOYMENT_TARGET:-15.0}"
STRATA_BUNDLE_ID="${STRATA_BUNDLE_ID:-dev.strata.workbench}"
STRATA_TEAM_ID="${STRATA_TEAM_ID:-2NK7ZR2DY7}"
STRATA_BUILD_NUMBER="${STRATA_BUILD_NUMBER:-2}"
STRATA_SIGNING_IDENTITY="${STRATA_SIGNING_IDENTITY:--}"
STRATA_ARTIFACT_DIR="${STRATA_REPO_ROOT}/target/artifacts"
# shellcheck disable=SC2034 # consumed by scripts that source this library
STRATA_APP_BUNDLE="${STRATA_ARTIFACT_DIR}/${STRATA_APP_NAME}.app"
STRATA_INFO_PLIST="${STRATA_INFO_PLIST:-${STRATA_REPO_ROOT}/packaging/macos/Strata-Info.plist}"

strata_workspace_version() {
    awk '
        $0 == "[workspace.package]" { in_package = 1; next }
        in_package && /^\[/ { exit }
        in_package && $1 == "version" {
            gsub(/\"/, "", $3)
            print $3
            exit
        }
    ' "${STRATA_REPO_ROOT}/Cargo.toml"
}

STRATA_MARKETING_VERSION="${STRATA_MARKETING_VERSION:-$(strata_workspace_version)}"
# shellcheck disable=SC2034 # consumed by scripts that source this library
STRATA_DMG_FILE="${STRATA_ARTIFACT_DIR}/${STRATA_APP_NAME}-${STRATA_MARKETING_VERSION}-arm64.dmg"

strata_require_command() {
    local command_name="$1"
    if ! command -v "${command_name}" >/dev/null 2>&1; then
        printf 'Required command not found: %s\n' "${command_name}" >&2
        return 1
    fi
}

strata_validate_release_settings() {
    if [[ ! "${STRATA_MARKETING_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        printf 'STRATA_MARKETING_VERSION must use x.y.z form: %s\n' \
            "${STRATA_MARKETING_VERSION}" >&2
        return 1
    fi
    if [[ ! "${STRATA_BUILD_NUMBER}" =~ ^[0-9]+$ ]]; then
        printf 'STRATA_BUILD_NUMBER must be numeric: %s\n' "${STRATA_BUILD_NUMBER}" >&2
        return 1
    fi
    if [[ ! "${STRATA_DEPLOYMENT_TARGET}" =~ ^[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
        printf 'STRATA_DEPLOYMENT_TARGET must be a macOS version: %s\n' \
            "${STRATA_DEPLOYMENT_TARGET}" >&2
        return 1
    fi
    if [[ ! "${STRATA_BUNDLE_ID}" =~ ^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$ ]]; then
        printf 'STRATA_BUNDLE_ID is invalid: %s\n' "${STRATA_BUNDLE_ID}" >&2
        return 1
    fi
    if [[ ! "${STRATA_TEAM_ID}" =~ ^[A-Z0-9]{10}$ ]]; then
        printf 'STRATA_TEAM_ID is invalid: %s\n' "${STRATA_TEAM_ID}" >&2
        return 1
    fi
}

strata_target_directory() {
    printf '%s\n' "${CARGO_TARGET_DIR:-${STRATA_REPO_ROOT}/target}"
}

strata_profile_binary() {
    printf '%s/%s/%s/%s\n' \
        "$(strata_target_directory)" \
        "${STRATA_TARGET_TRIPLE}" \
        "${STRATA_BUILD_PROFILE}" \
        "${STRATA_EXECUTABLE_NAME}"
}

strata_assert_generated_path() {
    local generated_item="$1"
    local generated_parent
    local target_root
    target_root="$(cd "${STRATA_REPO_ROOT}/target" && pwd -P)"
    if ! generated_parent="$(cd "$(dirname "${generated_item}")" && pwd -P)"; then
        printf 'Generated destination parent does not exist: %s\n' \
            "${generated_item}" >&2
        return 1
    fi
    case "${generated_parent}/$(basename "${generated_item}")" in
        "${target_root}"/*) ;;
        *)
            printf 'Refusing to replace a path outside repository target/: %s\n' \
                "${generated_item}" >&2
            return 1
            ;;
    esac
}

strata_replace_generated_directory() {
    local staged_directory="$1"
    local destination_directory="$2"
    strata_assert_generated_path "${destination_directory}"
    rm -rf -- "${destination_directory}"
    mv -- "${staged_directory}" "${destination_directory}"
}

strata_replace_generated_file() {
    local staged_file="$1"
    local destination_file="$2"
    strata_assert_generated_path "${destination_file}"
    rm -f -- "${destination_file}"
    mv -- "${staged_file}" "${destination_file}"
}

strata_plist_value() {
    local plist_file="$1"
    local plist_key="$2"
    /usr/libexec/PlistBuddy -c "Print :${plist_key}" "${plist_file}"
}

strata_sha256() {
    shasum -a 256 "$1" | awk '{ print $1 }'
}

strata_verify_disk_image() {
    local disk_image="$1"
    local attempt
    for attempt in 1 2 3 4 5; do
        if hdiutil verify "${disk_image}"; then
            return 0
        fi
        if (( attempt < 5 )); then
            printf 'Disk image not ready; retrying verification (%s/5).\n' \
                "$((attempt + 1))" >&2
            sleep 1
        fi
    done
    printf 'Disk image verification did not stabilize: %s\n' "${disk_image}" >&2
    return 1
}

strata_is_developer_id_build() {
    [[ "${STRATA_SIGNING_IDENTITY}" != "-" ]]
}
