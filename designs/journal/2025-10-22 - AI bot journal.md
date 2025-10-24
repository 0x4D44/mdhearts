2025-10-22 — Hard Performance H1 implementation notes

Summary
- Implemented Stage H1 (env-gated) in Hard planner:
  - Added leverage tiers (Narrow/Normal/Wide) via `MDH_HARD_TIERS_ENABLE` and thresholds `MDH_HARD_LEVERAGE_THRESH_NARROW`/`_NORMAL`.
  - Introduced per-tier effective limits with global override:
    - PhaseB top-K (defaults: 4/6/8), next-trick probe M (1/2/3), AB margin (100/150/200).
    - Used in choose/explain paths and next_trick_probe; explain remains deterministic.
  - Early-cutoff uses safe bound: `max(next_base + cont_cap, next_base + early_cutoff_margin)` (cont cap when set).
  - Extended Hard Stats: scanned_phase_a/b/c, leverage_score, tier, limits_in_effect, utilization%.
  - Telemetry collected without changing default behavior (tiers OFF by default).

Files touched
- crates/hearts-app/src/bot/search.rs
  - New: Tier/Limits, effective_limits(ctx), compute_leverage(ctx).
  - choose()/explain(): per-tier topK/AB margin, phase counters, safe early-cutoff, extended Stats.
  - explain_verbose(_parts): tier-aware topK, Stats snapshot.
  - next_trick_probe(): per-tier M for probed leads.
  - Budget: added probe_calls and utilization helper.
  - debug_hard_weights_string(): prints tiers and leverage thresholds.

Knobs
- MDH_HARD_TIERS_ENABLE=1 to enable tiered limits.
- MDH_HARD_LEVERAGE_THRESH_NARROW (default 20), MDH_HARD_LEVERAGE_THRESH_NORMAL (default 50).
- Existing overrides still honored: MDH_HARD_PHASEB_TOPK, MDH_HARD_NEXT_BRANCH_LIMIT, MDH_HARD_AB_MARGIN.

Validation
- cargo test --all: all tests green.
- Behavior parity preserved with tiers disabled (default). CLI last_stats now carries richer telemetry; existing prints unaffected.

Next
- H1 docs: README/docs/CLI_TOOLS to describe new knobs and Stats fields.
- Begin H2 (guarded planner nudges + adaptive continuation) behind env flags; keep defaults OFF.

Mixed-seat baseline (quick)
- Command: `--match-mixed west 1000 60 nnhh --hard-deterministic --hard-steps 120`
- Output:
  - tiers OFF: designs/tuning/mixed_nnhh_west_1000_60_baseline.csv (61 lines incl header)
  - tiers ON:  designs/tuning/mixed_nnhh_west_1000_60_tiers.csv (61 lines incl header)
- Early spot check shows same first few rows (expected; tiering adjusts limits but defaults conservative). Will analyze aggregates after H2.

H2 start (planner nudges; env-gated)
- Added tiny planner-level nudges in Normal base_score (defaults off):
  - Leader-feed nudge per penalty when feeding leader and base per-penalty feed is small (guarded by `MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE`).
  - Self-capture nudge per penalty when we’d capture and our score ≥ 85.
- Knobs: `MDH_HARD_PLANNER_NUDGES=1`, `MDH_HARD_PLANNER_LEADER_FEED_NUDGE`, `MDH_HARD_PLANNER_SELF_CAPTURE_NUDGE`, `MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE`.
- Tests remain green with defaults off.

H2 adaptive continuation scaling (env-gated)
- Added per-mille per-penalty scaling for continuation parts in Hard rollout (defaults 0):
  - MDH_HARD_CONT_SCALE_FEED_PERMIL, MDH_HARD_CONT_SCALE_SELFCAP_PERMIL.
- Scaling applied proportionally to penalties captured/fed in current trick; respects cont cap; mirrored in verbose parts output.
- Tests remain green.

Phase A scaffolding (env-gated; defaults unchanged)
- Added `MDH_HARD_PROMOTE_DEFAULTS`: when set, slightly increases Hard continuation weights (feed/self) and applies a small continuation cap (~250) if not overridden, and auto-enables tiering for Hard.
- Added `MDH_HARD_TIERS_DEFAULT_ON_HARD`: auto-enable tiering only for Hard without turning on global tiers.
- Debug weights string prints tiers_auto/promoted flags.

Phase A defaults (Hard)
- Promoted Hard continuation defaults (feed 70, self 95) and cont cap (~250) as true defaults (still overridable by env).
- Tiering now defaults ON for Hard (FutureHard) while Normal remains unchanged.
- Quick deterministic NNHH (west, 200 seeds) shows unchanged averages (expected due to modest changes), but infrastructure is now in place for deeper H2/C adjustments.

Phase B defaults (Hard-only base-score nudges)
- Added tiny Hard-only nudges directly in base_score (leader-feed +30/pen when feeding leader on penalty tricks; self-capture −30/pen when near 100), guarded by simple conditions and kept conservative.
- All tests remain green.

Phase C default tuning & evaluations
- Enabled choose-only next-probe widening (+1) in Wide tier; added small default Wide-tier continuation boosts (+10% feed, +6% self-capture). Explain path remains deterministic.
- Added more disagreement goldens (W/N/E) derived from compare-batch.
- Evaluations (deterministic):
  - Mixed-seat (all seats, n=200): no aggregate delta vs baseline in 1000..1199.
  - Match-batch (Normal vs Hard): n=300/seat and n=1000/seat both show equal averages in sampled ranges.
- Takeaway: With conservative tuning and mixed tables, Hard’s improvements do not move seat-level averages materially in these ranges.

Next proposals
- Add 1–2 more targeted goldens (North/East) to lock continuation/deepening outcomes.
- Consider a slightly larger Wide-tier-only default bump (feed +15%, self +10%) and/or widen Wide next-probe by one more in choose-only, then re-run n≥1000 evaluations.
- Alternatively, explore Hard-only base-score leader-target weighting in current-trick scoring (very small) under tier=Wide to amplify intended behavior while keeping Normal unchanged.
