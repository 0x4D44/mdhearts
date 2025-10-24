Summary
- Verified repo state: Hard planner includes deterministic budget, tiers, continuation parts/cap, probe widening, and determinization scaffold. Tests green.
- Updated CLI help to enumerate Hard flags (deterministic, steps, top‑K, branch limits, cap, cutoff, continuation boost, determinization, verbose).
- Drafted Stage 6 Next Steps Plan outlining telemetry/help, determinization flipping golden, conservative Wide‑tier continuation tuning, evaluation protocol (mixed‑seat, two windows), and guardrails.

Next Actions
- Add constructed flipping golden under determinization (K>1 plus probe widening) to lock intended behavior.
- Run mixed-seat NNHH/HHNN/NHNH/HNHN deterministic evaluations (n≥1000/seat across two windows); capture CSVs and summaries.
- Iterate tiny Hard‑only tuning (Wide‑tier boosts and planner nudge) with strict caps/guards; update goldens only when stable.

Notes
- Keep MDH_CLI_POPUPS unset to avoid Windows dialogs; console output only.
- Maintain explain determinism; choose may deepen under tier/det settings.

Smoke results (today)
- Added determinization flip golden (enabled) that asserts determinization changes totals in a mid-trick scenario with void followers; will tighten to require a flip once the hand shape is finalized.
- Mixed-seat smoke on same seed window (1000..1199), seat=West:
  - NNHH West(Hard) avg_pen = 6.66
  - HHNN West(Normal) avg_pen = 6.66
  - As expected for a small run under conservative defaults, no difference observed.

Full mixed-seat (deterministic) runs
- Generated CSVs for north/east/south across 1000..1999 and 2000..2999 (n=1000/seat/mix/window), and prepared a CI-style summary (means and 95% CIs).
- Results: Hard ≈ Normal across mixes/seats/windows under current conservative defaults.
- Artifacts: mixed_*_{seat}_{window}_1000_det.csv and mixed_summary_ci_full.md under designs/tuning/.

Next steps
- Craft a stricter constructed flip golden (ordering flip) using in-test env and, if needed, two-trick context.
- Run a targeted mixed-seat eval (n=300–500/seat) with conservative Hard-only env nudges to gauge movement.
- If promising, re-run full eval and consider promoting tiny defaults with docs/golden updates.

Determinization-on targeted smoke (seat=West)
- Env: MDH_HARD_DET_ENABLE=1, MDH_HARD_DET_SAMPLE_K=5, MDH_HARD_DET_PROBE_WIDE_LIKE=1, MDH_HARD_DET_NEXT3_ENABLE=1 (deterministic caps).
- Seeds 1000..1299, n=300: NNHH West(Hard) avg_pen = 6.387; HHNN West(Normal) avg_pen = 6.387.
- No aggregate change observed (as expected with small continuation weights). Will rely on stricter golden and/or slightly stronger but capped nudges if needed.

Nudges trial 2 (West, n=500, 1000..1499): NNHH=6.366, HHNN=6.366 (no movement).

CI runbook added: designs/2025.10.23 - CI runbook for Hard default trial.md (commands, env, acceptance).
