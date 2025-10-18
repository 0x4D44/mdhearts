#!/usr/bin/env python3
"""Summarise Stage 2 telemetry logs.

Parses `telemetry.jsonl` emitted by the benchmarking harness when
`--log-pass-details` / `--log-moon-details` are enabled and reports
aggregate statistics for pass decisions and moon-aware play objectives.

Optionally writes the summary to JSON and/or Markdown for downstream
analytics pipelines.
"""

from __future__ import annotations

import argparse
import json
import math
from collections import Counter
from pathlib import Path
from statistics import mean
from typing import Dict, Iterable, List, Optional, Tuple


def _parse_float_list(raw: Optional[str]) -> List[float]:
    if not raw:
        return []
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return []
    if isinstance(data, list):
        return [float(item) for item in data if isinstance(item, (int, float))]
    return []


def summarise_pass_events(events: Iterable[dict]) -> dict:
    totals = []
    candidate_counts = []
    moon_probs = []
    objective_counter: Counter[str] = Counter()
    margin_deltas = []

    for event in events:
        fields = event.get("fields", {})
        totals.append(float(fields.get("total", 0.0)))
        candidate_counts.append(int(fields.get("candidate_count", 0)))
        moon_probs.append(float(fields.get("moon_probability", 0.0)))
        objective_counter[fields.get("moon_objective", "")] += 1

        top_scores = _parse_float_list(fields.get("top_scores"))
        if len(top_scores) >= 2:
            margin_deltas.append(top_scores[0] - top_scores[1])

    summary = {
        "count": len(totals),
        "avg_total": mean(totals) if totals else math.nan,
        "avg_candidates": mean(candidate_counts) if candidate_counts else math.nan,
        "avg_moon_probability": mean(moon_probs) if moon_probs else math.nan,
        "objective_counts": dict(objective_counter),
        "avg_best_margin": mean(margin_deltas) if margin_deltas else math.nan,
    }
    block_count = objective_counter.get("block_shooter", 0)
    summary["block_ratio"] = (block_count / len(totals)) if totals else math.nan
    return summary


def summarise_play_events(events: Iterable[dict]) -> dict:
    objective_counter: Counter[str] = Counter()
    for event in events:
        fields = event.get("fields", {})
        objective_counter[fields.get("objective", "")] += 1
    total = sum(objective_counter.values())
    block = objective_counter.get("BlockShooter", 0)
    return {
        "objective_counts": dict(objective_counter),
        "block_ratio": (block / total) if total else math.nan,
        "count": total,
    }


def analyze(path: Path) -> Dict[str, object]:
    pass_events = []
    play_events = []

    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                payload = json.loads(line)
            except json.JSONDecodeError:
                continue

            target = payload.get("target", "")
            if target == "hearts_bot::pass_decision":
                pass_events.append(payload)
            elif target == "hearts_bot::play":
                play_events.append(payload)

    pass_summary = summarise_pass_events(pass_events)
    play_summary = summarise_play_events(play_events)

    return {"pass": pass_summary, "play": play_summary}


def render_markdown(summary: Dict[str, object], telemetry_path: Path) -> str:
    pass_summary = summary["pass"]
    play_summary = summary["play"]
    lines = [
        "# Telemetry Summary",
        "",
        f"- Source: `{telemetry_path}`",
        "",
        "## Pass Decisions",
        f"- Events: {pass_summary['count']}",
    ]
    if not math.isnan(pass_summary["avg_total"]):
        lines.append(f"- Avg total score: {pass_summary['avg_total']:.2f}")
    if not math.isnan(pass_summary["avg_candidates"]):
        lines.append(f"- Avg candidates: {pass_summary['avg_candidates']:.2f}")
    if not math.isnan(pass_summary["avg_moon_probability"]):
        lines.append(
            f"- Avg moon probability: {pass_summary['avg_moon_probability']:.3f}"
        )
    if not math.isnan(pass_summary["avg_best_margin"]):
        lines.append(
            f"- Avg best vs next margin: {pass_summary['avg_best_margin']:.2f}"
        )

    objectives = pass_summary["objective_counts"]
    if objectives:
        lines.append("- Objectives:")
        for objective, count in sorted(objectives.items()):
            label = objective or "<unset>"
            lines.append(f"  - {label}: {count}")
    block_ratio = pass_summary.get("block_ratio", math.nan)
    if not math.isnan(block_ratio):
        lines.append(f"- Block-shooter ratio: {block_ratio:.3f}")
    lines.extend(
        [
            "",
            "## Play Objectives",
        ]
    )
    play_objectives = play_summary["objective_counts"]
    if play_objectives:
        for objective, count in sorted(play_objectives.items()):
            label = objective or "<unset>"
            lines.append(f"- {label}: {count}")
    else:
        lines.append("- <none>")
    play_block_ratio = play_summary.get("block_ratio", math.nan)
    if not math.isnan(play_block_ratio):
        lines.append(f"- Block-shooter play ratio: {play_block_ratio:.3f}")
    lines.append("")
    return "\n".join(lines)


def print_summary(summary: Dict[str, object]) -> None:
    pass_summary = summary["pass"]
    play_summary = summary["play"]

    print("Pass decisions: {count}".format(count=pass_summary["count"]))
    if pass_summary["count"]:
        print(
            "  avg total score: {0:.2f}".format(pass_summary["avg_total"])
        )
        print(
            "  avg candidate count: {0:.2f}".format(pass_summary["avg_candidates"])
        )
        print(
            "  avg moon probability: {0:.3f}".format(
                pass_summary["avg_moon_probability"]
            )
        )
        print(
            "  avg best-vs-next margin: {0:.2f}".format(
                pass_summary["avg_best_margin"]
            )
        )
        print(
            "  block-shooter ratio: {0:.3f}".format(
                pass_summary["block_ratio"]
            )
        )
        print("  objective counts:")
        for objective, count in sorted(pass_summary["objective_counts"].items()):
            print(f"    {objective or '<unset>'}: {count}")

    print("Play objective counts:")
    objectives = play_summary["objective_counts"]
    if objectives:
        for objective, count in sorted(objectives.items()):
            print(f"  {objective or '<unset>'}: {count}")
    else:
        print("  <none>")
    print(
        "  block-shooter ratio: {0:.3f}".format(
            play_summary["block_ratio"]
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "telemetry",
        type=Path,
        help="Path to telemetry.jsonl emitted by the Stage 2 benchmarking harness",
    )
    parser.add_argument(
        "--json",
        type=Path,
        help="Optional path to write the summary as JSON",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        help="Optional path to write the summary in Markdown",
    )
    parser.add_argument(
        "--csv",
        type=Path,
        help="Optional path to append a CSV row with aggregated metrics",
    )
    parser.add_argument(
        "--run-id",
        type=str,
        help="Optional run identifier to include in CSV output",
    )
    args = parser.parse_args()

    if not args.telemetry.exists():
        parser.error(f"telemetry file not found: {args.telemetry}")

    summary = analyze(args.telemetry)
    print_summary(summary)

    if args.json:
        args.json.write_text(
            json.dumps(summary, indent=2),
            encoding="utf-8",
        )

    if args.markdown:
        args.markdown.write_text(
            render_markdown(summary, args.telemetry),
            encoding="utf-8",
        )

    if args.csv:
        headers = [
            "run_id",
            "telemetry_path",
            "pass_count",
            "pass_block_ratio",
            "pass_avg_score",
            "pass_avg_moon_probability",
            "play_count",
            "play_block_ratio",
        ]
        row = [
            args.run_id or args.telemetry.parent.name,
            str(args.telemetry),
            str(summary["pass"]["count"]),
            f"{summary['pass']['block_ratio']:.6f}"
            if not math.isnan(summary["pass"]["block_ratio"])
            else "",
            f"{summary['pass']['avg_total']:.6f}"
            if not math.isnan(summary["pass"]["avg_total"])
            else "",
            f"{summary['pass']['avg_moon_probability']:.6f}"
            if not math.isnan(summary["pass"]["avg_moon_probability"])
            else "",
            str(summary["play"]["count"]),
            f"{summary['play']['block_ratio']:.6f}"
            if not math.isnan(summary["play"]["block_ratio"])
            else "",
        ]
        write_csv(args.csv, headers, row)


def write_csv(path: Path, headers: List[str], row: List[str]) -> None:
    exists = path.exists()
    with path.open("a", encoding="utf-8") as handle:
        if not exists:
            handle.write(",".join(headers) + "\n")
        handle.write(",".join(row) + "\n")


if __name__ == "__main__":
    main()
