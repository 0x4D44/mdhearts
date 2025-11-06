#!/usr/bin/env bash
set -euo pipefail

DIR=${1:-designs/tuning/stage1/smoke_release}
if [ ! -d "$DIR" ]; then
  echo "Artifact directory not found: $DIR" >&2
  exit 1
fi

missing=0
bad=0
check_file() {
  local path=$1
  if [ ! -f "$path" ]; then
    echo "MISSING: $path" >&2
    missing=$((missing+1))
    return
  fi
  # Count data rows regardless of trailing newline
  local data
  data=$(awk 'NR>1{c++} END{print (c?c:0)}' "$path")
  if [ "${data:-0}" -lt 1 ]; then
    echo "NO DATA ROWS: $path" >&2
    bad=$((bad+1))
  else
    echo "OK (data rows=$data): $path"
  fi
}

for seat in west east south north; do
  for mix in nnhh hhnn; do
    check_file "$DIR/stage1_ci_${seat}_${mix}_smoke_fast1.csv"
  done
done

if [ $missing -gt 0 ] || [ $bad -gt 0 ]; then
  echo "Artifact validation failed: missing=$missing bad=$bad" >&2
  exit 2
fi

echo "All smoke artifacts present and non-empty."
