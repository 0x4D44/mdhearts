# Stage 5 — Tuning Pass 1 Summary (deterministic)

Ranges
- west 1000..1149 (150)
- south 1080..1229 (150)
- east 2000..2149 (150)
- north 1100..1299 (200)

Disagreement counts (lines include header; lines-1 ~= disagreements)
- west_1000_150: baseline_lines=5 tuned_lines=5
- south_1080_150: baseline_lines=5 tuned_lines=5
- east_2000_150: baseline_lines=6 tuned_lines=6
- north_1100_200: baseline_lines=7 tuned_lines=7

Observation
- +5 feed/self_capture continuation weights (env-only) did not change disagreement counts in these ranges. Leave defaults unchanged.

Next
- If we pursue deeper tweaks, prefer targeted continuation caps or minor probe/pruning adjustments behind env flags; validate via goldens and benches.