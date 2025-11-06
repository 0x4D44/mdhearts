## Summary

Briefly describe the change. Link to relevant design docs if applicable.

## Motivation

Why is this change needed? What problems does it solve?

## Changes

- Key code changes (files/modules)
- Flags/envs introduced/changed

## Tests

- New tests added and what they cover
- Existing tests touched

## Artifacts

- Link to archived CSVs/traces under `designs/tuning/`

## CI

- Ultra-fast smoke job (`ultra-smoke`) runs 1-seed NNHH/HHNN per seat and validates artifact presence.

## Risks & Mitigations

- Behavioral risk and guardrails
- Performance considerations

## Reviewer Checklist (optional)

- [ ] Stage 1 neutrality holds within ±0.01 on deterministc smokes
- [ ] Guard reasons appear as expected in traces
- [ ] Stage 2 avoids clean-trick capture when over cap

