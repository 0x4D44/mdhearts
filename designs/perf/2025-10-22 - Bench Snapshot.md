Bench Snapshot — 2025-10-22

Method
- CLI `--bench-check <difficulty> <seat> <seed_start> <count>` with small ranges.
- Hard runs deterministic: `--hard-deterministic --hard-steps 80`.

Normal (avg_us, p95_us)
- West 1000..1029: avg 42, p95 111
- North 1100..1129: avg 48, p95 114
- East 2000..2029: avg 51, p95 119
- South 1080..1109: avg 65, p95 161

Hard (deterministic; avg_us, p95_us)
- West 1000..1029: avg 703, p95 2420
- North 1100..1129: avg 465, p95 1557
- East 2000..2029: avg 649, p95 1634
- South 1080..1109: avg 770, p95 2488

Notes
- Typical Hard decisions (deterministic, step-capped) are sub‑3ms p95 in this snapshot.
- If widening caps in future, keep p95 well under 20–30ms.
