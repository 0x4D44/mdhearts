#!/usr/bin/env bash
set -euo pipefail

# Medium-size Normal vs Hard compare with fast Hard limits.
# Usage: tools/compare_medium.sh [seed_start] [count]

SEED_START=${1:-500}
COUNT=${2:-10}

BIN="target/release/mdhearts"
if [ ! -x "$BIN" ]; then
  echo "Building release mdhearts…" >&2
  cargo build -p hearts-app --bin mdhearts --release >/dev/null
fi

OUT_DIR="designs/tuning/stage1/compare_release"
mkdir -p "$OUT_DIR"

FAST=(--hard-steps 60 --hard-branch-limit 150 --hard-next-branch-limit 80 --hard-time-cap-ms 5)

run() {
  local seat=$1 mode=$2
  local out="${OUT_DIR}/compare_${mode}_${seat}_s${SEED_START}_n${COUNT}.csv"
  echo "compare ${mode} ${seat} -> ${out}" >&2
  if [ "$mode" = "on" ]; then
    MDH_FEATURE_HARD_STAGE12=1 MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 \
      "$BIN" --compare-batch "$seat" "$SEED_START" "$COUNT" --only-disagree --out "$out" "${FAST[@]}" || true
  else
    MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 \
      "$BIN" --compare-batch "$seat" "$SEED_START" "$COUNT" --only-disagree --out "$out" "${FAST[@]}" || true
  fi
}

for seat in west east south north; do
  run "$seat" off
  run "$seat" on
done

echo "Done. Compare CSVs in $OUT_DIR" >&2
