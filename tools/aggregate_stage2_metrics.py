#!/usr/bin/env python3
"""Aggregate Stage 2 telemetry and block-shooter results.

Given one or more benchmark run directories (each containing a
`telemetry_summary.json` produced by `tools/analyze_telemetry.py`) and an
optional block-shooter summary JSON, emit a consolidated Markdown digest
that can be pasted directly into the Stage 2 deck/docs.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    # Re-use the existing telemetry parser to avoid duplicating logic.
    from analyze_telemetry import analyze as analyze_telemetry  # type: ignore
except ImportError as exc:  # pragma: no cover - should not happen in repo
    raise SystemExit(f"Failed to import analyze_telemetry: {exc}") from exc


@dataclass
class TelemetryRun:
    label: str
    path: Path
    summary: Dict[str, Dict[str, float]]


def parse_run_argument(raw: str) -> Tuple[str, Path]:
    if "=" not in raw:
        raise argparse.ArgumentTypeError(
            f"Run argument must be in label=path form, got '{raw}'"
        )
    label, path = raw.split("=", 1)
    if not label:
        raise argparse.ArgumentTypeError(f"Run label missing in '{raw}'")
    candidate = Path(path)
    if not candidate.exists():
        raise argparse.ArgumentTypeError(f"Run path does not exist: {candidate}")
    return label, candidate


def load_telemetry_summary(path: Path) -> Dict[str, Dict[str, float]]:
    summary_path = path / "telemetry_summary.json"
    if summary_path.exists():
        with summary_path.open("r", encoding="utf-8") as handle:
            return json.load(handle)

    telemetry_path = path / "telemetry.jsonl"
    if telemetry_path.exists():
        return analyze_telemetry(telemetry_path)

    raise FileNotFoundError(
        f"No telemetry summary found in {path}. "
        "Expected telemetry_summary.json or telemetry.jsonl."
    )


def load_runs(arguments: Iterable[str]) -> List[TelemetryRun]:
    runs: List[TelemetryRun] = []
    for raw in arguments:
        label, path = parse_run_argument(raw)
        summary = load_telemetry_summary(path)
        runs.append(TelemetryRun(label=label, path=path, summary=summary))
    if not runs:
        raise ValueError("At least one run must be provided.")
    return runs


def load_block_summary(path: Optional[Path]) -> Dict[str, dict]:
    if path is None:
        return {}
    if not path.exists():
        raise FileNotFoundError(f"Block summary JSON not found: {path}")
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    output: Dict[str, dict] = {}
    for entry in data:
        label = entry.get("label")
        if not label:
            continue
        output[label] = entry
    return output


def ratio_bar(value: float, width: int = 20) -> str:
    if math.isnan(value):
        return "N/A"
    clamped = max(0.0, min(1.0, value))
    filled = int(round(clamped * width))
    return f"{'█' * filled}{'░' * (width - filled)} {clamped * 100:5.1f}%"


def render_markdown(runs: List[TelemetryRun], block_summary: Dict[str, dict]) -> str:
    lines: List[str] = [
        "# Stage 2 Metrics Digest",
        "",
        "## Pass Telemetry Overview",
        "",
        "| Run | Pass Events | Pass Block Ratio | Avg Pass Score | "
        "Avg Moon Probability | Play Events | Play Block Ratio |",
        "| --- | ---: | --- | ---: | ---: | ---: | --- |",
    ]
    for run in runs:
        pass_summary = run.summary["pass"]
        play_summary = run.summary["play"]
        pass_count = int(pass_summary.get("count", 0))
        objective_counts = pass_summary.get("objective_counts", {})
        pass_block_ratio = pass_summary.get("block_ratio", math.nan)
        if math.isnan(pass_block_ratio):
            pass_block_events = float(objective_counts.get("block_shooter", 0))
            pass_block_ratio = (pass_block_events / pass_count) if pass_count else math.nan
        pass_avg_total = pass_summary.get("avg_total", math.nan)
        pass_avg_moon = pass_summary.get("avg_moon_probability", math.nan)

        play_objectives = play_summary.get("objective_counts", {})
        play_count = int(
            play_summary.get("count", sum(int(v) for v in play_objectives.values()))
        )
        play_block_ratio = play_summary.get("block_ratio", math.nan)
        if math.isnan(play_block_ratio):
            play_block_events = float(play_objectives.get("BlockShooter", 0))
            play_block_ratio = (play_block_events / play_count) if play_count else math.nan

        lines.append(
            "| {label} | {pass_count} | {pass_block} | {pass_score} | {moon_prob} | "
            "{play_count} | {play_block} |".format(
                label=run.label,
                pass_count=pass_count,
                pass_block=ratio_bar(pass_block_ratio),
                pass_score=(
                    f"{pass_avg_total:.2f}" if not math.isnan(pass_avg_total) else "N/A"
                ),
                moon_prob=(
                    f"{pass_avg_moon:.3f}" if not math.isnan(pass_avg_moon) else "N/A"
                ),
                play_count=play_count,
                play_block=ratio_bar(play_block_ratio),
            )
        )

    if block_summary:
        lines.extend(
            [
                "",
                "## Block-Shooter Outcomes",
                "",
                "| Run | Events | Success Rate | Other Shoots | Self Shoots | "
                "Moon Rate / Hand | Avg Block Probability |",
                "| --- | ---: | --- | ---: | ---: | --- | ---: |",
            ]
        )
        for run in runs:
            entry = block_summary.get(run.label)
            if not entry:
                continue
            success_rate = entry.get("success_rate", math.nan)
            moon_rate = entry.get("moon_rate_per_hand", math.nan)
            avg_prob = entry.get("avg_probability", math.nan)
            lines.append(
                "| {label} | {events} | {success} | {other} | {self} | {moon} | {avg_prob} |".format(
                    label=entry["label"],
                    events=int(entry.get("events", 0)),
                    success=ratio_bar(success_rate) if not math.isnan(success_rate) else "N/A",
                    other=int(entry.get("failure_other", 0)),
                    self=int(entry.get("failure_self", 0)),
                    moon=(
                        f"{moon_rate * 100:5.1f}%"
                        if not math.isnan(moon_rate)
                        else "N/A"
                    ),
                    avg_prob=(
                        f"{avg_prob:.3f}" if not math.isnan(avg_prob) else "N/A"
                    ),
                )
            )

        lines.append("")
        lines.append("### Block Probability Bins")
        lines.append("")
        lines.append("| Run | Probability Bin | Attempts | Successes | Success Rate |")
        lines.append("| --- | --- | ---: | ---: | --- |")
        rendered = False
        for run in runs:
            entry = block_summary.get(run.label)
            if not entry:
                continue
            bins = entry.get("probability_bins") or entry.get("bins", {})
            if not bins:
                continue
            for bucket, payload in sorted(bins.items()):
                total = payload.get("total", 0)
                success = payload.get("success", 0)
                rate = success / total if total else math.nan
                lines.append(
                    "| {label} | {bucket} | {total} | {success} | {rate} |".format(
                        label=entry["label"],
                        bucket=bucket,
                        total=total,
                        success=success,
                        rate=f"{rate * 100:5.1f}%"
                        if not math.isnan(rate)
                        else "N/A",
                    )
                )
                rendered = True
        if not rendered:
            lines.append("| — | — | 0 | 0 | N/A |")

    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "runs",
        nargs="+",
        help="Benchmark run directories in label=path form "
        "(expects telemetry_summary.json inside the path).",
    )
    parser.add_argument(
        "--block-summary",
        type=Path,
        help="Optional JSON file produced by tools/analyze_block_shooter.py",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        help="Optional path to write the combined Markdown digest",
    )
    args = parser.parse_args()

    runs = load_runs(args.runs)
    block_summary = load_block_summary(args.block_summary)
    markdown = render_markdown(runs, block_summary)

    print(markdown)
    if args.markdown:
        args.markdown.write_text(markdown, encoding="utf-8")


if __name__ == "__main__":
    main()
