#!/usr/bin/env bash
set -euo pipefail

# All-in-one eval helper: smokes, compares, thresholds, summary.
# Usage: tools/eval_all.sh [seed_start]

SEED_START=${1:-180}
SMOKE_COUNT=${SMOKE_COUNT:-2}
SMALL_COUNT=${SMALL_COUNT:-3}
MED_COUNT=${MED_COUNT:-6}

BIN="target/release/mdhearts"
if [ ! -x "$BIN" ]; then
  echo "Building release mdhearts…" >&2
  cargo build -p hearts-app --bin mdhearts --release >/dev/null
fi

STAMP=$(date +%Y-%m-%d_%H%M%S)
SUM_DIR="designs/tuning/eval_summaries"
OUT_MD="$SUM_DIR/eval_${STAMP}.md"
mkdir -p "$SUM_DIR"

echo "Running ultra-fast smokes (SMOKE_COUNT=${SMOKE_COUNT})…" >&2
SMOKE_COUNT=$SMOKE_COUNT ./tools/smoke_fast.sh "$SEED_START" >/dev/null || true
./tools/index_stage1_smokes.sh designs/tuning/stage1/smoke_release >/dev/null || true

echo "Checking smoke thresholds (>=2 lines)…" >&2
SMOKE_OK=0
if ./tools/check_smoke_thresholds.sh designs/tuning/stage1/smoke_release >/dev/null; then
  SMOKE_OK=1
fi

echo "Running small compares (OFF/ON, ${SMALL_COUNT} seeds)…" >&2
./tools/compare_small.sh "$SEED_START" "$SMALL_COUNT" >/dev/null || true

echo "Running medium compares (OFF/ON, ${MED_COUNT} seeds)…" >&2
./tools/compare_medium.sh "$SEED_START" "$MED_COUNT" >/dev/null || true

count_rows() {
  local pattern=$1
  local total=0
  for f in designs/tuning/stage1/compare_release/${pattern}; do
    local n=$(wc -l < "$f" | tr -d '[:space:]')
    # subtract header
    if [ "$n" -gt 1 ]; then
      total=$(( total + (n - 1) ))
    fi
  done
  echo "$total"
}

SMALL_DISAG=$(count_rows 'compare_*_s'*)

{
  echo "# Eval Summary (${STAMP})"
  echo
  echo "## Smokes"
  echo "- Seed start: ${SEED_START}"
  echo "- Count per run (SMOKE_COUNT): ${SMOKE_COUNT}"
  echo "- Threshold OK: ${SMOKE_OK}"
  echo
  echo "## Compares (disagreements only)"
  echo "- Small set rows: ${SMALL_DISAG}"
  echo "- Medium outputs written to designs/tuning/stage1/compare_release/"
} > "$OUT_MD"

echo "WROTE ${OUT_MD}"

