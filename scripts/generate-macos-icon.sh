#!/usr/bin/env bash

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICON_MASTER="${REPOSITORY_ROOT}/packaging/macos/Strata.icon-master.png"
ICON_OUTPUT="${REPOSITORY_ROOT}/packaging/macos/Strata.icns"

if [[ ! -f "${ICON_MASTER}" ]]; then
    printf 'Icon master is missing: %s\n' "${ICON_MASTER}" >&2
    exit 1
fi

DIMENSIONS="$(sips -g pixelWidth -g pixelHeight "${ICON_MASTER}" 2>/dev/null)"
if ! grep -q 'pixelWidth: 1024' <<<"${DIMENSIONS}" \
    || ! grep -q 'pixelHeight: 1024' <<<"${DIMENSIONS}"; then
    printf 'Icon master must be exactly 1024 x 1024 pixels.\n' >&2
    exit 1
fi

mkdir -p "${REPOSITORY_ROOT}/target"
STAGING_ROOT="$(mktemp -d "${REPOSITORY_ROOT}/target/.strata-icon.XXXXXX")"
ICONSET="${STAGING_ROOT}/Strata.iconset"
cleanup() {
    if [[ "${STRATA_KEEP_ICON_STAGE:-0}" == "1" ]]; then
        printf 'Preserved icon staging at %s\n' "${STAGING_ROOT}"
    else
        rm -rf -- "${STAGING_ROOT}"
    fi
}
trap cleanup EXIT
mkdir -p "${ICONSET}"

write_size() {
    local pixels="$1"
    local filename="$2"
    sips -z "${pixels}" "${pixels}" "${ICON_MASTER}" \
        --out "${ICONSET}/${filename}" >/dev/null
}

write_size 16 icon_16x16.png
write_size 32 icon_16x16@2x.png
write_size 32 icon_32x32.png
write_size 64 icon_32x32@2x.png
write_size 128 icon_128x128.png
write_size 256 icon_128x128@2x.png
write_size 256 icon_256x256.png
write_size 512 icon_256x256@2x.png
write_size 512 icon_512x512.png
write_size 1024 icon_512x512@2x.png

BASE_16_ICNS="${STAGING_ROOT}/base-16.icns"
BASE_32_ICNS="${STAGING_ROOT}/base-32.icns"
sips -s format icns "${ICONSET}/icon_16x16.png" --out "${BASE_16_ICNS}" >/dev/null
sips -s format icns "${ICONSET}/icon_32x32.png" --out "${BASE_32_ICNS}" >/dev/null

# iconutil in Xcode 26.6 rejects even an iconset it just extracted. Assemble the
# documented ICNS chunk container directly, retaining sips' legacy 16/32-bit
# chunks and PNG-compressed modern/Retina representations.
CHUNK_TYPES=(ic07 ic08 ic09 ic10 ic11 ic12 ic13 ic14)
CHUNK_FILES=(
    "${ICONSET}/icon_128x128.png"
    "${ICONSET}/icon_256x256.png"
    "${ICONSET}/icon_512x512.png"
    "${ICONSET}/icon_512x512@2x.png"
    "${ICONSET}/icon_16x16@2x.png"
    "${ICONSET}/icon_32x32@2x.png"
    "${ICONSET}/icon_128x128@2x.png"
    "${ICONSET}/icon_256x256@2x.png"
)

BASE_HEADER_BYTES=32
BASE_16_BYTES="$(stat -f '%z' "${BASE_16_ICNS}")"
BASE_32_BYTES="$(stat -f '%z' "${BASE_32_ICNS}")"
TOTAL_BYTES=$((8 + BASE_16_BYTES - BASE_HEADER_BYTES + BASE_32_BYTES - BASE_HEADER_BYTES))
for chunk_file in "${CHUNK_FILES[@]}"; do
    TOTAL_BYTES=$((TOTAL_BYTES + 8 + $(stat -f '%z' "${chunk_file}")))
done

write_big_endian_u32() {
    printf '%08x' "$1" | xxd -r -p
}

STAGED_ICNS="${STAGING_ROOT}/Strata.icns"
{
    printf 'icns'
    write_big_endian_u32 "${TOTAL_BYTES}"
    dd if="${BASE_16_ICNS}" bs=1 skip="${BASE_HEADER_BYTES}" 2>/dev/null
    dd if="${BASE_32_ICNS}" bs=1 skip="${BASE_HEADER_BYTES}" 2>/dev/null
    for chunk_index in "${!CHUNK_TYPES[@]}"; do
        chunk_file="${CHUNK_FILES[${chunk_index}]}"
        printf '%s' "${CHUNK_TYPES[${chunk_index}]}"
        write_big_endian_u32 "$((8 + $(stat -f '%z' "${chunk_file}")))"
        cat "${chunk_file}"
    done
} >"${STAGED_ICNS}"

VERIFY_ICONSET="${STAGING_ROOT}/verify.iconset"
iconutil -c iconset "${STAGED_ICNS}" -o "${VERIFY_ICONSET}"
for expected_icon in \
    icon_16x16.png \
    icon_32x32.png \
    icon_128x128.png \
    icon_256x256.png \
    icon_512x512.png \
    icon_512x512@2x.png; do
    if [[ ! -f "${VERIFY_ICONSET}/${expected_icon}" ]]; then
        printf 'Generated ICNS is missing %s.\n' "${expected_icon}" >&2
        exit 1
    fi
done

mv -- "${STAGED_ICNS}" "${ICON_OUTPUT}"
printf 'Generated %s\n' "${ICON_OUTPUT}"
