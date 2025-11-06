#!/usr/bin/env bash
set -euo pipefail

# Ultra-fast deterministic smokes for Stage 1 sanity.
# Usage: tools/smoke_fast.sh [seed_start]

SEED_START=${1:-100}
COUNT=${SMOKE_COUNT:-2}
OUT_DIR="designs/tuning/stage1/smoke_release"
mkdir -p "$OUT_DIR"

BIN="target/release/mdhearts"
if [ ! -x "$BIN" ]; then
  echo "Building release mdhearts…" >&2
  cargo build -p hearts-app --bin mdhearts --release >/dev/null
fi

run_smoke() {
  local seat=$1 mix=$2
  local out="tmp/stage1_ci_${seat}_${mix}_smoke_fast1.csv"
  MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 MDH_FEATURE_HARD_STAGE12=1 \
    "$BIN" --match-mixed "$seat" "$SEED_START" "$COUNT" "$mix" \
    --hard-steps 60 --hard-branch-limit 150 --hard-next-branch-limit 80 --hard-time-cap-ms 5 \
    --stats --out "$out" || true
  if [ -f "$out" ]; then
    cp -f "$out" "$OUT_DIR/"/
    echo "saved: $OUT_DIR/$(basename "$out")"
  else
    echo "FAILED: $seat $mix"
  fi
}

for seat in west east south north; do
  run_smoke "$seat" nnhh
  run_smoke "$seat" hhnn
done

echo "Done. Outputs in $OUT_DIR" >&2
