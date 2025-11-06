#!/usr/bin/env bash
set -euo pipefail

# Commit helper for continue-on-main behind feature flags. DRY-RUN by default.
# Usage: RUN=1 tools/commit_plan_on_main.sh

DRY=${RUN:-0}

run() {
  echo "+ $*"
  if [ "$DRY" = "1" ]; then
    eval "$@"
  fi
}

echo "Preparing grouped commits on current branch (DRY_RUN=$((DRY==0)))"

# 1) feature flags + gating
run git add crates/hearts-app/src/bot/play.rs crates/hearts-app/src/cli.rs
run git commit -m "feat(hard): gate Stage1/2 behind runtime flags; add CLI toggles" || true

# 2) tools + CI (smokes, eval, compare, thresholds)
run git add tools/smoke_fast.sh tools/check_smoke_artifacts.sh tools/check_smoke_thresholds.sh \
  tools/compare_small.sh tools/compare_medium.sh tools/index_stage1_smokes.sh tools/index_compare.sh \
  tools/eval_all.sh tools/nightly_eval.sh .github/workflows/ci.yml .github/workflows/eval.yml || true
run git commit -m "tools(ci): add eval + compare wrappers; enable smokes with flags; manual CI eval" || true

# 3) docs (README + helpers + CI eval + Stage1 README + plans)
run git add README.md docs/CLI_TOOLS_SMOKE.md docs/EVAL_WRAPPERS.md docs/CI_EVAL.md \
  designs/2025.10.30\ -\ Stage1+2\ PR\ Description.md designs/2025.10.30\ -\ Stage1+2\ PR\ Summary.md \
  designs/2025.10.31\ -\ Continue-On-Main\ via\ Feature\ Flags.md designs/2025.11.01\ -\ Plan\ -\ Feature-flag\ follow-through.md \
  designs/tuning/eval_summaries/README.md designs/tuning/stage1/README.md || true
run git commit -m "docs: feature-flag flow, eval wrappers, CI eval; refresh Stage1 README" || true

# 4) artifacts + indexes (smokes/compare summaries)
run git add designs/tuning/stage1/smoke_release/*.csv designs/tuning/stage1/smoke_release/INDEX.md \
  designs/tuning/stage1/compare_release/*.csv designs/tuning/stage1/compare_release/INDEX.md \
  designs/tuning/eval_summaries/*.md || true
run git commit -m "artifacts(stage1): ultra-smokes + compare outputs with indexes and summaries" || true

# 5) journals
run git add wrk_journals/*.md || true
run git commit -m "chore(journal): add and update JRN entries for feature-flag path and eval runs" || true

echo "Done. Review with: git log --oneline --decorate --graph"

