#!/usr/bin/env bash
set -euo pipefail

max_lines="${STRATA_MAX_FILE_LINES:-1200}"
case "$max_lines" in
    ''|*[!0-9]*)
        printf 'STRATA_MAX_FILE_LINES must be a positive integer, got %s\n' "$max_lines" >&2
        exit 2
        ;;
esac

if (( max_lines < 1 )); then
    printf 'STRATA_MAX_FILE_LINES must be greater than zero\n' >&2
    exit 2
fi

violations=0
while IFS= read -r path; do
    line_count=$(wc -l < "$path")
    if (( line_count > max_lines )); then
        printf '%5d  %s\n' "$line_count" "$path" >&2
        violations=$((violations + 1))
    fi
done < <(
    rg --files \
        -g '!target/**' \
        -g '!output/**' \
        -g '!.git/**' \
        -g '*.rs' \
        -g '*.md' \
        -g '*.toml' \
        -g '*.json' \
        -g '*.jsonl' \
        -g '*.wit' \
        -g '*.wgsl' \
        -g '*.mmd' \
        -g '*.txt' \
        -g '*.sh' \
        -g '*.py' \
        -g '*.js' \
        -g '*.ts' \
        -g '*.tsx' \
        -g '*.html' \
        -g '*.css' \
        -g '*.swift' \
        -g '*.c' \
        -g '*.cc' \
        -g '*.cpp' \
        -g '*.h' \
        -g '*.hpp' \
        -g '*.yml' \
        -g '*.yaml' \
        -g '*.plist' \
        -g 'Dockerfile' \
        -g 'Justfile' \
        -g 'justfile' \
        -g '!*.strata-session/**' \
        -g '!*.cdx.json' \
        -g '!*.spdx.json' \
        | LC_ALL=C sort
)

if (( violations > 0 )); then
    printf 'Found %d maintained text file(s) over the %d-line limit.\n' \
        "$violations" "$max_lines" >&2
    exit 1
fi

printf 'Maintained text files satisfy the %d-line limit.\n' "$max_lines"
