#!/usr/bin/env bash
set -euo pipefail

SRC_DIR=${1:-designs/tuning/stage1/compare_release}
OUT="$SRC_DIR/INDEX.md"

if [ ! -d "$SRC_DIR" ]; then
  echo "Source dir not found: $SRC_DIR" >&2
  exit 1
fi

count_rows() {
  local f=$1
  local n=$(wc -l < "$f" | tr -d '[:space:]')
  if [ "$n" -gt 1 ]; then echo $((n-1)); else echo 0; fi
}

total=0
{
  echo "# Stage1/2 Compare Artifacts (disagreements only)"
  echo
  echo "| File | Rows |"
  echo "|------|------|"
  for f in "$SRC_DIR"/compare_*.csv; do
    rows=$(count_rows "$f")
    total=$(( total + rows ))
    echo "| $(basename "$f") | ${rows} |"
  done | sort
  echo
  echo "Total disagreement rows: ${total}"
} > "$OUT"

echo "WROTE $OUT"

