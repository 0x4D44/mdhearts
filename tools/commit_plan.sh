#!/usr/bin/env bash
set -euo pipefail

# Commit helper for feature/hard-stage1-2. DRY-RUN by default.
# Usage: RUN=1 tools/commit_plan.sh

DRY=${RUN:-0}
branch=feature/hard-stage1-2

run() {
  echo "+ $*"
  if [ "$DRY" = "1" ]; then
    eval "$@"
  fi
}

echo "Preparing commits to branch: $branch (DRY_RUN=$((DRY==0)))"

if ! git rev-parse --verify "$branch" >/dev/null 2>&1; then
  run git checkout -b "$branch"
else
  run git checkout "$branch"
fi

# 1) docs: recovery plan + stage1 archives README + curated CSVs
run git add \
  "designs/2025.10.30 - Post-Power-Cut WIP and Plan.md" \
  designs/tuning/stage1/README.md \
  designs/tuning/stage1/*.csv
run git commit -m "docs(stage1): add recovery plan, archives README, and curated CSVs" || true

# 2–4) tests: stage1/2 coverage
run git add crates/hearts-app/tests/todo_stage1_stage2.rs
run git commit -m "tests(stage1+2): add guard and stage2 coverage (incl. flat-scores, over-cap)" || true

# 5) tools/docs: fast smokes + recipes
run git add tools/smoke_fast.sh docs/CLI_TOOLS_SMOKE.md README.md
run git commit -m "tools,docs: add ultra-fast smoke script and recipes; README quick smoke" || true

# 6) artifacts: 1-seed smokes NNHH/HHNN
run git add designs/tuning/stage1/smoke_release/*.csv
run git commit -m "artifacts(stage1): archive 1-seed NNHH/HHNN smokes (all seats)" || true

# 7) ci: ultra-smoke + validation
run git add .github/workflows/ci.yml tools/check_smoke_artifacts.sh
run git commit -m "ci: add PR ultra-smoke job with artifact validation" || true

# 8) PR summary + notes
run git add "designs/2025.10.30 - Stage1+2 PR Summary.md" designs/notes/
run git commit -m "docs: add Stage1+2 PR summary; archive stage1 notes" || true

echo "Done. Review with: git log --oneline --decorate --graph"

