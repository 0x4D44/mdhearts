#!/usr/bin/env python3
"""Estimate block-shooter ratios under different probability thresholds.

Reuses telemetry/deal parsing from `tools/analyze_block_shooter.py` to compute
how many block passes would fire if we raised or lowered the estimator
threshold, along with projected success rates.
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path
from typing import Dict, Iterable, List, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent

if str(SCRIPT_DIR) not in __import__("sys").path:
    __import__("sys").path.insert(0, str(SCRIPT_DIR))

from analyze_block_shooter import (  # noqa: E402
    BlockEvent,
    load_block_events,
    load_deals,
    parse_run_argument,
)


def summarize_threshold(events: List[BlockEvent], threshold: float) -> Dict[str, float]:
    filtered = [event for event in events if not math.isnan(event.probability) and event.probability >= threshold]
    total = len(filtered)
    success = sum(1 for event in filtered if event.moon_shooter is None)
    other_shooter = sum(
        1
        for event in filtered
        if event.moon_shooter is not None and event.moon_shooter != event.seat
    )
    self_shooter = sum(
        1
        for event in filtered
        if event.moon_shooter is not None and event.moon_shooter == event.seat
    )
    return {
        "threshold": threshold,
        "events": total,
        "success": success,
        "success_rate": success / total if total else math.nan,
        "failure_other": other_shooter,
        "failure_self": self_shooter,
    }


def sweep_thresholds(events: List[BlockEvent], thresholds: Iterable[float]) -> List[Dict[str, float]]:
    return [summarize_threshold(events, threshold) for threshold in thresholds]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "runs",
        nargs="+",
        help="Run directories in label=path form (must contain telemetry.jsonl & deals.jsonl).",
    )
    parser.add_argument(
        "--thresholds",
        type=float,
        nargs="+",
        default=[0.32, 0.4, 0.45, 0.5],
        help="Probability thresholds to evaluate (default: 0.32 0.4 0.45 0.5).",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        help="Optional path to write the sweep as a Markdown table.",
    )
    args = parser.parse_args()

    rows: List[str] = []
    markdown_lines: List[str] = [
        "| Run | Threshold | Events | Success | Success Rate | Other Shoots | Self Shoots |",
        "| --- | --- | ---: | ---: | --- | ---: | ---: |",
    ]

    for raw in args.runs:
        label, path = parse_run_argument(raw)
        deals, run_id_hint, _ = load_deals(path / "deals.jsonl")
        events = load_block_events(path / "telemetry.jsonl", deals, run_id_hint, label)
        summaries = sweep_thresholds(events, args.thresholds)

        for summary in summaries:
            rows.append(
                "{label} threshold={threshold:.2f} events={events} success={success} rate={rate}".format(
                    label=label,
                    threshold=summary["threshold"],
                    events=summary["events"],
                    success=summary["success"],
                    rate="nan" if math.isnan(summary["success_rate"]) else f"{summary['success_rate']:.4f}",
                )
            )
            markdown_lines.append(
                "| {label} | {threshold:.2f} | {events} | {success} | {rate} | {other} | {self} |".format(
                    label=label,
                    threshold=summary["threshold"],
                    events=summary["events"],
                    success=summary["success"],
                    rate="N/A" if math.isnan(summary["success_rate"]) else f"{summary['success_rate'] * 100:5.1f}%",
                    other=summary["failure_other"],
                    self=summary["failure_self"],
                )
            )

    print("\n".join(rows))
    if args.markdown:
        args.markdown.write_text("\n".join(markdown_lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
