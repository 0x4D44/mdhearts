#!/usr/bin/env bash
set -euo pipefail

# Validate that smoke CSVs have at least a minimal number of lines (header + 1 row).
# Usage: tools/check_smoke_thresholds.sh <dir> [min_lines]

DIR=${1:-designs/tuning/stage1/smoke_release}
MIN=${2:-2}

shopt -s nullglob
fail=0
for f in "$DIR"/*.csv; do
  lines=$(wc -l < "$f" | tr -d '[:space:]')
  printf '%s: %s lines\n' "$f" "$lines"
  if [ "$lines" -lt "$MIN" ]; then
    echo "FAIL: $f has fewer than $MIN lines" >&2
    fail=1
  fi
done

exit "$fail"
