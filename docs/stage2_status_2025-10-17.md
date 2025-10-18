# Stage 2 Pass & Moon Telemetry Status — 2025-10-17

## Progress Summary

- **Structured pass telemetry** now records ranked candidates, component scores, moon probability/objective, and candidate counts when `MDH_PASS_DETAILS=1` is set. The Stage 0 harness propagates this via `logging.pass_details` or the `--log-pass-details` flag.
- **Moon-aware play logging** exposes the active objective (PPH vs. Block Shooter) on every decision behind `MDH_MOON_DETAILS=1`; harness support mirrors the pass flag with `logging.moon_details` / `--log-moon-details`.
- **Regression coverage** ensures both flags default to “off” and flips to “on” when the environment variables are present (`pass_logging_enabled_*`, `moon_logging_*` tests).
- **Stage 0 harness smoke hash** updated; newly added config `bench/stage2_pass_moon.yaml` drives 1,024 hands × 4 permutations with telemetry enabled.
- **Telemetry analysis helper** (`tools/analyze_telemetry.py`) summarises pass totals, candidate counts, moon probability distribution, and play-objective frequencies directly from `telemetry.jsonl`.
- **Harness summarisation** now writes `telemetry_summary.json` + `.md` alongside Stage 2 runs; CLI prints headline stats, `summary.md` gains an appended **Telemetry Highlights** section, and `tools/analyze_telemetry.py` can mirror the outputs with `--json/--markdown`.
- **Benchmarks executed:** `bench/stage2_pass_moon.yaml` completed for both the tightened `pass_v045_selective` run (`bench/out/stage2_pass_moon_v045_selective/`) and the legacy control (`bench/out/stage2_pass_moon_legacy/`). The thresholded run logged 10,764 pass decisions (avg score 151.1, moon prob 0.361) with block-shooter objectives on ~18.6 % of passes / ~9.3 % of plays. Legacy still emits no pass telemetry and highlights the Easy bot’s dumping edge. Detailed notes in `docs/benchmarks/stage2_pass_moon_v045_selective.md`, `docs/benchmarks/stage2_pass_moon_2025-10-17.md`, and `docs/benchmarks/stage2_pass_moon_control_2025-10-17.md`.
- **Telemetry analytics**: `tools/analyze_telemetry.py` emits block-shooter ratios and optional CSV rows (see `bench/out/stage2_pass_moon_v045_selective/telemetry_summary_py.csv`). `tools/analyze_block_shooter.py` joins pass telemetry with deal logs (97.1 % success across 2,071 block passes in pass_v045_selective); `tools/telemetry_to_markdown.py` turns CSVs into tables (`docs/benchmarks/stage2_pass_block_ratios.md`), and `tools/compare_pass_runs.py` builds side-by-side PPH summaries (`docs/benchmarks/stage2_pass_comparison.md`).

## Outstanding Work

1. **Benchmark synthesis:** fold the pass_v2 vs. pass_v1 comparison into the Stage 2 acceptance deck (charts + narrative).
2. **Telemetry aggregation pipeline:** plug the refreshed `telemetry_summary.json`/CSV outputs into analytics dashboards (still need notebook ingestion + visualisation wiring).
3. **Block-shooter calibration:** analyse the pass_v045_selective telemetry to confirm moon-defense effectiveness, focusing on the 20 inverted-moon failures ≥ 0.60 probability (West 8, South 5, North 5, East 2) surfaced in `docs/benchmarks/stage2_block_failures.md`; the first directional liability prototype (`bench/out/stage2_pass_moon_v045_directional/`) showed no improvement, so iterate on more targeted heuristics before re-running benchmarks.
4. **Documentation & runbook:** extend Stage 2 docs with the benchmark procedure, example summaries from the analyzer script, and guidance on interpreting moon/pass telemetry.
5. **Analysis playbook:** execute the detailed plan in `docs/stage2_block_shooter_analysis_plan.md` (CSV join, success bins, visuals) to feed the acceptance deck.

## Quick Commands

```bash
# structured Stage 2 run (threshold 0.45)
MDH_PASS_V2=1 MDH_ENABLE_BELIEF=1 \
  cargo run -p hearts-bench -- \
  --config bench/stage2_pass_moon.yaml \
  --run-id stage2_pass_moon_v045_selective \
  --log-pass-details --log-moon-details

# summarise telemetry output
python3 tools/analyze_telemetry.py \
  bench/out/stage2_pass_moon_v045_selective/telemetry.jsonl \
  --json bench/out/stage2_pass_moon_v045_selective/telemetry_summary_py.json \
  --markdown bench/out/stage2_pass_moon_v045_selective/telemetry_summary_py.md \
  --csv bench/out/stage2_pass_moon_v045_selective/telemetry_summary_py.csv \
  --run-id stage2_pass_moon_v045_selective

# join block-shooter passes with deal outcomes
python3 tools/analyze_block_shooter.py \
  pass_v045_selective=bench/out/stage2_pass_moon_v045_selective \
  pass_v1=bench/out/stage2_pass_moon_legacy \
  --csv bench/out/block_shooter_summary_v045_selective.csv \
  --json bench/out/block_shooter_summary_v045_selective.json

# list high-probability failures
python3 tools/list_block_failures.py \
  pass_v045_selective=bench/out/stage2_pass_moon_v045_selective \
  --threshold 0.6 \
  --markdown docs/benchmarks/stage2_block_failures.md
```
