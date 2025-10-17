#!/usr/bin/env python3
"""
Cross-check the Rust Wilcoxon implementation against SciPy for a benchmark run.

Usage:
  python tools/validate_wilcoxon.py bench/out/stage0_smoke/summary.md

The script parses the Markdown summary table emitted by `hearts-bench`,
computes Wilcoxon signed-rank p-values with SciPy when available, and reports
any material divergence. If SciPy is missing, the script exits gracefully with
instructions to install it.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Tuple

try:
    from scipy import stats  # type: ignore
except ImportError:
    stats = None


ROW_PATTERN = re.compile(
    r"\|\s*(?P<agent>[^|]+)\|\s*[^|]+\|\s*(?P<hands>\d+)\|\s*[^|]+\|\s*[^|]+\|\s*\[[^]]+\]\|\s*[^|]+\|\s*[^|]+\|\s*[^|]+\|\s*[^|]+\|\s*(?P<pvalue>[0-9.]+)\s*\|"
)


@dataclass
class AgentRow:
    name: str
    hands: int
    p_value: float


def parse_summary_table(path: Path) -> Dict[str, AgentRow]:
    rows: Dict[str, AgentRow] = {}
    content = path.read_text(encoding="utf-8")
    for line in content.splitlines():
        match = ROW_PATTERN.match(line)
        if not match:
            continue
        agent = match.group("agent").strip()
        hands = int(match.group("hands"))
        p_value = float(match.group("pvalue"))
        rows[agent] = AgentRow(agent, hands, p_value)
    if not rows:
        raise ValueError(f"No rows parsed from {path}")
    return rows


def load_diffs(jsonl_path: Path, baseline: str) -> Dict[str, List[float]]:
    import json

    diffs: Dict[str, List[float]] = {}
    with jsonl_path.open("r", encoding="utf-8") as handle:
        for raw in handle:
            payload = json.loads(raw)
            if payload["bot"] == baseline:
                baseline_points = payload["pph"]
                continue
            agent_name = payload["bot"]
            diff = payload["pph"] - baseline_points
            diffs.setdefault(agent_name, []).append(diff)
    return diffs


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Wilcoxon p-values using SciPy")
    parser.add_argument("summary", type=Path, help="Path to hearts-bench summary.md")
    parser.add_argument(
        "--jsonl",
        type=Path,
        default=None,
        help="Optional path to deals.jsonl (auto-infers from summary when omitted)",
    )
    parser.add_argument(
        "--baseline",
        required=False,
        help="Baseline agent name (inferred from summary header when omitted)",
    )
    args = parser.parse_args()

    if stats is None:
        print("SciPy not installed. Run `pip install -r python/requirements.txt`.", file=sys.stderr)
        return 2

    rows = parse_summary_table(args.summary)
    if not args.jsonl:
        jsonl_path = args.summary.with_name("deals.jsonl")
    else:
        jsonl_path = args.jsonl

    baseline = args.baseline or next(iter(rows.keys()))
    diffs = load_diffs(jsonl_path, baseline)

    failures: List[Tuple[str, float, float]] = []
    for agent, row in rows.items():
        if agent == baseline:
            continue
        sample = diffs.get(agent, [])
        if not sample:
            continue
        _, scipy_p = stats.wilcoxon(sample, zero_method="wilcox", correction=True, alternative="two-sided")
        if abs(scipy_p - row.p_value) > 0.01:
            failures.append((agent, row.p_value, float(scipy_p)))

    if failures:
        print("Mismatch detected between Rust and SciPy Wilcoxon results:")
        for agent, rust_p, scipy_p in failures:
            print(f"  {agent}: Rust={rust_p:.4f}, SciPy={scipy_p:.4f}")
        return 1

    print("Wilcoxon cross-check passed (Rust ≈ SciPy).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
