#!/usr/bin/env python3
"""Render Stage 2 benchmark charts from telemetry + block summaries."""

from __future__ import annotations

import argparse
import math
from pathlib import Path
from typing import Iterable, List, Optional

SCRIPT_DIR = Path(__file__).resolve().parent

if str(SCRIPT_DIR) not in __import__("sys").path:
    __import__("sys").path.insert(0, str(SCRIPT_DIR))

from aggregate_stage2_metrics import (  # noqa: E402
    load_block_summary,
    load_runs,
)

import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402


def _ensure_output_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def render_block_ratio_chart(runs, output_dir: Path, fmt: str) -> Path:
    labels: List[str] = []
    ratios: List[float] = []
    counts: List[int] = []
    for run in runs:
        value = run.summary["pass"].get("block_ratio", math.nan)
        if math.isnan(value) or run.summary["pass"]["count"] == 0:
            continue
        labels.append(run.label)
        ratios.append(value)
        counts.append(int(run.summary["pass"]["count"]))

    if not labels:
        return output_dir / f"stage2_pass_block_ratio.{fmt}"

    fig, ax = plt.subplots(figsize=(6, 3 + len(labels) * 0.4))
    positions = np.arange(len(labels))
    bars = ax.barh(positions, ratios, color="#3772FF")
    ax.set_xlim(0, 1.0)
    ax.set_yticks(positions, labels=[label.replace("_", " ") for label in labels])
    ax.set_xlabel("Block-shooter pass ratio")
    ax.set_title("Stage 2 Pass Decisions — Block Ratio")
    ax.grid(axis="x", linestyle="--", alpha=0.3)

    for bar, ratio, count in zip(bars, ratios, counts):
        ax.text(
            ratio + 0.01,
            bar.get_y() + bar.get_height() / 2,
            f"{ratio * 100:.1f}% ({count:,} passes)",
            va="center",
            fontsize=9,
        )

    fig.tight_layout()
    output_path = output_dir / f"stage2_pass_block_ratio.{fmt}"
    fig.savefig(output_path, dpi=150)
    plt.close(fig)
    return output_path


def render_block_success_bins(
    block_summary: dict, output_dir: Path, fmt: str
) -> Optional[Path]:
    entries = [
        entry for entry in block_summary.values() if entry.get("bins") or entry.get("probability_bins")
    ]
    if not entries:
        return None

    for entry in entries:
        bins = entry.get("probability_bins") or entry.get("bins", {})
        if not bins:
            continue
        labels = []
        success_rates = []
        totals = []
        for bucket, payload in sorted(bins.items()):
            labels.append(bucket)
            total = int(payload.get("total", 0))
            success = int(payload.get("success", 0))
            rate = success / total if total else math.nan
            success_rates.append(rate)
            totals.append(total)

        if not labels:
            continue

        fig, ax = plt.subplots(figsize=(7, 4))
        positions = np.arange(len(labels))
        bars = ax.bar(positions, success_rates, color="#F46036")
        ax.set_ylim(0, 1.05)
        ax.set_xticks(positions, labels, rotation=35, ha="right")
        ax.set_ylabel("Success rate")
        ax.set_title(f"Block-shooter Success by Estimator Bucket — {entry['label']}")
        ax.grid(axis="y", linestyle="--", alpha=0.3)

        for bar, rate, total in zip(bars, success_rates, totals):
            label = "N/A" if math.isnan(rate) else f"{rate * 100:.1f}%"
            ax.text(
                bar.get_x() + bar.get_width() / 2,
                bar.get_height() + 0.02,
                f"{label}\n(n={total})",
                ha="center",
                va="bottom",
                fontsize=8,
            )

        fig.tight_layout()
        output_path = output_dir / f"stage2_block_success_bins_{entry['label']}.{fmt}"
        fig.savefig(output_path, dpi=150)
        plt.close(fig)
    return output_dir / f"stage2_block_success_bins_{entries[-1]['label']}.{fmt}"


def render_success_overview(block_summary: dict, output_dir: Path, fmt: str) -> Optional[Path]:
    if not block_summary:
        return None

    labels: List[str] = []
    success_rates: List[float] = []
    events: List[int] = []
    moon_rates: List[float] = []
    for label, entry in block_summary.items():
        events.append(int(entry.get("events", 0)))
        ratio = entry.get("success_rate", math.nan)
        moon_ratio = entry.get("moon_rate_per_hand", math.nan)
        if math.isnan(ratio) or events[-1] == 0:
            continue
        labels.append(label)
        success_rates.append(ratio)
        moon_rates.append(moon_ratio if not math.isnan(moon_ratio) else 0.0)

    if not labels:
        return None

    x = np.arange(len(labels))
    width = 0.35
    fig, ax1 = plt.subplots(figsize=(6, 4))

    bars_success = ax1.bar(
        x - width / 2,
        success_rates,
        width,
        label="Block-pass success rate",
        color="#2A9D8F",
    )
    ax1.set_ylim(0, 1.05)
    ax1.set_ylabel("Success rate")
    ax1.set_xticks(x, [label.replace("_", " ") for label in labels])
    ax1.set_title("Block-shooter Outcomes Overview")

    ax2 = ax1.twinx()
    ax2.plot(
        x + width / 2,
        [rate * 100 for rate in moon_rates],
        color="#E9C46A",
        marker="o",
        label="Moon rate per hand",
    )
    ax2.set_ylabel("Moon rate (%)")

    for idx, bar in enumerate(bars_success):
        ax1.text(
            bar.get_x() + bar.get_width() / 2,
            bar.get_height() + 0.02,
            f"{success_rates[idx] * 100:.1f}%\n(n={events[idx]:,})",
            ha="center",
            va="bottom",
            fontsize=8,
        )

    ax1.grid(axis="y", linestyle="--", alpha=0.3)
    fig.tight_layout()
    lines, labels_ = ax1.get_legend_handles_labels()
    lines2, labels2 = ax2.get_legend_handles_labels()
    ax1.legend(lines + lines2, labels_ + labels2, loc="lower center", bbox_to_anchor=(0.5, -0.25), ncol=2)

    output_path = output_dir / f"stage2_block_success_overview.{fmt}"
    fig.savefig(output_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    return output_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "runs",
        nargs="+",
        help="Benchmark run directories in label=path form (Stage 2 telemetry).",
    )
    parser.add_argument(
        "--block-summary",
        type=Path,
        help="Optional JSON file from tools/analyze_block_shooter.py.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("docs/benchmarks/plots"),
        help="Directory to write chart outputs (default: docs/benchmarks/plots).",
    )
    parser.add_argument(
        "--format",
        type=str,
        default="png",
        choices=("png", "svg"),
        help="Image format for rendered charts.",
    )
    args = parser.parse_args()

    runs = load_runs(args.runs)
    block_summary = load_block_summary(args.block_summary)

    output_dir = args.output_dir
    _ensure_output_dir(output_dir)

    render_block_ratio_chart(runs, output_dir, args.format)
    render_block_success_bins(block_summary, output_dir, args.format)
    render_success_overview(block_summary, output_dir, args.format)


if __name__ == "__main__":
    main()
