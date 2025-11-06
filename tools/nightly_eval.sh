#!/usr/bin/env bash
set -euo pipefail

# Nightly wrapper for feature-flag evals. Intended to be run manually or via scheduler.
# Uses moderate counts to stay quick while providing signal.

SEED_START=${1:-300}
export SMOKE_COUNT=${SMOKE_COUNT:-2}
export SMALL_COUNT=${SMALL_COUNT:-5}
export MED_COUNT=${MED_COUNT:-10}

echo "[nightly] seed_start=${SEED_START} SMOKE_COUNT=${SMOKE_COUNT} SMALL_COUNT=${SMALL_COUNT} MED_COUNT=${MED_COUNT}" >&2
"$(dirname "$0")/eval_all.sh" "$SEED_START"

