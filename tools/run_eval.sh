#!/usr/bin/env bash
set -euo pipefail

# Deterministic settings
export MDH_DEBUG_LOGS=0
export MDH_HARD_DETERMINISTIC=1
export MDH_HARD_TEST_STEPS=${MDH_HARD_TEST_STEPS:-120}

# Ranges (override via env if desired)
SEAT_START_WEST=${SEAT_START_WEST:-1000}
SEAT_START_SOUTH=${SEAT_START_SOUTH:-1080}
SEAT_START_EAST=${SEAT_START_EAST:-2000}
SEAT_START_NORTH=${SEAT_START_NORTH:-1100}
COUNT_WEST=${COUNT_WEST:-150}
COUNT_SOUTH=${COUNT_SOUTH:-150}
COUNT_EAST=${COUNT_EAST:-150}
COUNT_NORTH=${COUNT_NORTH:-200}

timestamp=$(date +%Y-%m-%d_%H%M%S)

cmp_west="designs/tuning/compare_west_${SEAT_START_WEST}_${COUNT_WEST}_det_${timestamp}.csv"
cmp_south="designs/tuning/compare_south_${SEAT_START_SOUTH}_${COUNT_SOUTH}_det_${timestamp}.csv"
cmp_east="designs/tuning/compare_east_${SEAT_START_EAST}_${COUNT_EAST}_det_${timestamp}.csv"
cmp_north="designs/tuning/compare_north_${SEAT_START_NORTH}_${COUNT_NORTH}_det_${timestamp}.csv"

match_west="designs/tuning/match_west_${SEAT_START_WEST}_${COUNT_WEST}_det_${timestamp}.csv"
match_south="designs/tuning/match_south_${SEAT_START_SOUTH}_${COUNT_SOUTH}_det_${timestamp}.csv"
match_east="designs/tuning/match_east_${SEAT_START_EAST}_${COUNT_EAST}_det_${timestamp}.csv"
match_north="designs/tuning/match_north_${SEAT_START_NORTH}_${COUNT_NORTH}_det_${timestamp}.csv"

mkdir -p designs/tuning

HARD_FEATURE_FLAG=${MDH_FEATURE_HARD_STAGE12:-""}
HARD_FLAG_ARGS=()
case "${HARD_FEATURE_FLAG,,}" in
  1|on|true|yes) HARD_FLAG_ARGS+=("--hard-stage12");;
  *) ;;
esac

echo "Running compare-batch (only-disagree)… ${HARD_FLAG_ARGS[*]:-}"
cargo run -q -p hearts-app -- --compare-batch west  "$SEAT_START_WEST"  "$COUNT_WEST"  --only-disagree --out "$cmp_west"  ${HARD_FLAG_ARGS[@]:-}
cargo run -q -p hearts-app -- --compare-batch south "$SEAT_START_SOUTH" "$COUNT_SOUTH" --only-disagree --out "$cmp_south" ${HARD_FLAG_ARGS[@]:-}
cargo run -q -p hearts-app -- --compare-batch east  "$SEAT_START_EAST"  "$COUNT_EAST"  --only-disagree --out "$cmp_east"  ${HARD_FLAG_ARGS[@]:-}
cargo run -q -p hearts-app -- --compare-batch north "$SEAT_START_NORTH" "$COUNT_NORTH" --only-disagree --out "$cmp_north" ${HARD_FLAG_ARGS[@]:-}

echo "Running match-batch (Normal vs Hard)… ${HARD_FLAG_ARGS[*]:-}"
cargo run -q -p hearts-app -- --match-batch west  "$SEAT_START_WEST"  "$COUNT_WEST"  normal hard --out "$match_west"  ${HARD_FLAG_ARGS[@]:-}
cargo run -q -p hearts-app -- --match-batch south "$SEAT_START_SOUTH" "$COUNT_SOUTH" normal hard --out "$match_south" ${HARD_FLAG_ARGS[@]:-}
cargo run -q -p hearts-app -- --match-batch east  "$SEAT_START_EAST"  "$COUNT_EAST"  normal hard --out "$match_east"  ${HARD_FLAG_ARGS[@]:-}
cargo run -q -p hearts-app -- --match-batch north "$SEAT_START_NORTH" "$COUNT_NORTH" normal hard --out "$match_north" ${HARD_FLAG_ARGS[@]:-}

lines() { wc -l < "$1" | tr -d '[:space:]'; }
summarize_match() {
  awk -F, 'NR>1 && NF>=7 { a+=$5; b+=$6; n++ } END { if(n>0) printf("n=%d avg_a=%.2f avg_b=%.2f avg_delta=%.2f\n", n, a/n, b/n, (b-a)/n) }' "$1"
}

out_md="designs/tuning/eval_summary_${timestamp}.md"
{
  echo "# Eval Summary (${timestamp})"
  echo
  echo "Compare (only-disagree) line counts:"
  echo "- ${cmp_west}  lines=$(lines "$cmp_west")"
  echo "- ${cmp_south} lines=$(lines "$cmp_south")"
  echo "- ${cmp_east}  lines=$(lines "$cmp_east")"
  echo "- ${cmp_north} lines=$(lines "$cmp_north")"
  echo
  echo "Match (Normal vs Hard) averages:"
  echo -n "- ${match_west}: "; summarize_match "$match_west"
  echo -n "- ${match_south}: "; summarize_match "$match_south"
  echo -n "- ${match_east}: "; summarize_match "$match_east"
  echo -n "- ${match_north}: "; summarize_match "$match_north"
} > "$out_md"

echo "WROTE $out_md"
