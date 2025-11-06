#!/usr/bin/env bash
set -euo pipefail

# Quick Normal vs Hard compare across all seats with fast Hard limits.
# Usage: tools/compare_small.sh [seed_start] [count]

SEED_START=${1:-150}
COUNT=${2:-5}

BIN="target/release/mdhearts"
if [ ! -x "$BIN" ]; then
  echo "Building release mdhearts…" >&2
  cargo build -p hearts-app --bin mdhearts --release >/dev/null
fi

OUT_DIR="designs/tuning/stage1/compare_release"
mkdir -p "$OUT_DIR"

run_compare() {
  local seat=$1 mode=$2
  local out="${OUT_DIR}/compare_${mode}_${seat}_s${SEED_START}_n${COUNT}.csv"
  echo "compare ${mode} ${seat} -> ${out}" >&2
  if [ "$mode" = "on" ]; then
    MDH_FEATURE_HARD_STAGE12=1 \
    MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 MDH_HARD_BRANCH_LIMIT=150 MDH_HARD_NEXT_BRANCH_LIMIT=80 MDH_HARD_TIME_CAP_MS=5 \
      "$BIN" --compare-batch "$seat" "$SEED_START" "$COUNT" --only-disagree --out "$out" || true
  else
    MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 MDH_HARD_BRANCH_LIMIT=150 MDH_HARD_NEXT_BRANCH_LIMIT=80 MDH_HARD_TIME_CAP_MS=5 \
      "$BIN" --compare-batch "$seat" "$SEED_START" "$COUNT" --only-disagree --out "$out" || true
  fi
}

for seat in west east south north; do
  run_compare "$seat" off
  run_compare "$seat" on
done

echo "Done. Compare CSVs in $OUT_DIR" >&2
