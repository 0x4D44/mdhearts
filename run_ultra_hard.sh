#!/bin/bash
# Ultra-Hard AI Configuration
# This script runs mdhearts with ALL AI improvements enabled and tuned for maximum difficulty
#
# Features enabled:
# - Phase 1: Belief-state sampling (20 samples for high accuracy)
# - Phase 2: Deep search (depth 4, 200ms thinking time)
# - Phase 3: Endgame perfect play (≤8 cards)
# - Aggressive search parameters

export MDH_BELIEF_SAMPLING_ENABLED=1
export MDH_BELIEF_SAMPLE_COUNT=20

export MDH_SEARCH_DEEPER_ENABLED=1
export MDH_SEARCH_MAX_DEPTH=4
export MDH_SEARCH_TIME_MS=200
export MDH_SEARCH_TT_SIZE=1000000

export MDH_ENDGAME_SOLVER_ENABLED=1
export MDH_ENDGAME_MAX_CARDS=8

# Run the game with ultra-hard AI
exec ./target/release/mdhearts "$@"
