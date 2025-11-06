#!/usr/bin/env bash
set -euo pipefail

SRC_DIR=${1:-designs/tuning/stage1/smoke_release}
OUT="$SRC_DIR/INDEX.md"

if [ ! -d "$SRC_DIR" ]; then
  echo "Source dir not found: $SRC_DIR" >&2
  exit 1
fi

gen_row() {
  local f=$1
  # Extract 2nd line fields: seed,seat,mix,pen,...
  if [ ! -s "$f" ]; then
    echo "| $(basename "$f") | (empty) |  |  |  |"; return
  fi
  local line
  line=$(awk 'NR==2{print; exit}' "$f")
  # Expect CSV: seed, seat, mix, pen, ...
  local seed seat mix pen
  IFS=',' read -r seed seat mix pen _rest <<<"$line"
  seed=$(echo "$seed" | xargs)
  seat=$(echo "$seat" | xargs)
  mix=$(echo "$mix" | xargs)
  pen=$(echo "$pen" | xargs)
  echo "| $(basename "$f") | $seat | $mix | $seed | $pen |"
}

{
  echo "# Stage 1 Ultra-Fast Smokes"
  echo
  echo "Auto-generated index of NNHH/HHNN smokes (SMOKE_COUNT-configurable)."
  echo
  echo "| File | Seat | Mix | Seed | Penalties |"
  echo "|------|------|-----|------|-----------|"
  for f in "$SRC_DIR"/*.csv; do
    gen_row "$f"
  done | sort
} > "$OUT"

echo "WROTE $OUT"
