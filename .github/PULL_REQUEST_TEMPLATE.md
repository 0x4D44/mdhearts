## Summary
Briefly describe the change (AI tuning, tests, docs, tools) and motivation.

## Validation
- [ ] `cargo test --all` passes locally
- [ ] Deterministic eval run (small ranges) completed with `tools/run_eval.(ps1|sh)`
- [ ] Artifacts saved under `designs/tuning/` (CSV + summary)

## Tests
- [ ] Added/updated constructed goldens (if behavior change)
- [ ] Tests are deterministic and clean up env vars

## Defaults
- [ ] Changes are env-only (defaults unchanged), or
- [ ] Defaults changed with strong evidence (stable goldens + improved match summaries)

## Docs
- [ ] README/docs updated if new knobs/scripts were added

