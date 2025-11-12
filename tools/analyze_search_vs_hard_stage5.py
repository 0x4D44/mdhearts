#!/usr/bin/env python3
"""
Utility script for Stage 5 of the search-vs-hard think-limit study.

Scans the timestamped artifact folders produced by `tools/run_search_vs_hard.ps1`,
computes aggregate metrics (per-seat averages, sigma, disagreement percentage,
telemetry timeout counts), and emits a JSON blob for reuse in reports.
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
from collections import Counter, defaultdict
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional


@dataclass
class SeatStats:
    seat: str
    count: int
    avg_search: float
    avg_hard: float
    avg_delta: float
    std_delta: float
    disagreement_count: int
    disagreement_pct: float


def parse_match_csv(path: Path) -> SeatStats:
    seat = path.stem.split("_")[1]
    deltas: List[float] = []
    search_pen: List[float] = []
    hard_pen: List[float] = []

    with path.open(newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            try:
                a_pen = float(row["a_pen"])
                b_pen = float(row["b_pen"])
                delta = float(row["delta"])
            except KeyError as exc:
                raise RuntimeError(f"Match CSV missing column {exc} ({path})") from exc
            search_pen.append(a_pen)
            hard_pen.append(b_pen)
            deltas.append(delta)

    if not deltas:
        raise RuntimeError(f"Match CSV {path} contains no rows")

    count = len(deltas)
    avg_search = sum(search_pen) / count
    avg_hard = sum(hard_pen) / count
    avg_delta = sum(deltas) / count
    std_delta = statistics.pstdev(deltas)

    # disagreement stats filled later
    return SeatStats(
        seat=seat,
        count=count,
        avg_search=round(avg_search, 3),
        avg_hard=round(avg_hard, 3),
        avg_delta=round(avg_delta, 3),
        std_delta=round(std_delta, 3),
        disagreement_count=0,
        disagreement_pct=0.0,
    )


def count_disagreements(path: Path) -> int:
    if not path.exists():
        return 0
    with path.open(newline="") as handle:
        reader = csv.reader(handle)
        # subtract header if file has >0 lines
        row_count = sum(1 for _ in reader)
    return max(row_count - 1, 0)


def parse_telemetry(path: Path) -> Dict[str, object]:
    if not path.exists():
        return {"records": 0, "post_records": 0, "timed_out": 0, "fallback_counts": {}}

    records = 0
    post_records = 0
    timed_out = 0
    fallback_counter: Counter[str] = Counter()

    with path.open() as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            records += 1
            payload = json.loads(line)
            if payload.get("phase") != "post":
                continue
            post_records += 1
            if payload.get("timed_out"):
                timed_out += 1
                fallback = payload.get("fallback") or "unknown"
                fallback_counter[fallback] += 1

    return {
        "records": records,
        "post_records": post_records,
        "timed_out": timed_out,
        "fallback_counts": dict(fallback_counter),
    }


def analyze_limit(limit_dir: Path) -> Dict[str, object]:
    seats: Dict[str, SeatStats] = {}
    for match_csv in limit_dir.glob("match_*.csv"):
        seat_stats = parse_match_csv(match_csv)
        seats[seat_stats.seat] = seat_stats

    for compare_csv in limit_dir.glob("compare_*.csv"):
        seat = compare_csv.stem.split("_")[1]
        disagreements = count_disagreements(compare_csv)
        stats = seats.get(seat)
        if stats:
            stats.disagreement_count = disagreements
            stats.disagreement_pct = round(
                (disagreements / stats.count) * 100.0 if stats.count else 0.0, 3
            )

    telemetry_info = parse_telemetry(limit_dir / "telemetry_smoke.jsonl")

    return {
        "seats": {seat: asdict(stats) for seat, stats in sorted(seats.items())},
        "telemetry": telemetry_info,
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Analyze search vs hard think-limit artifacts"
    )
    parser.add_argument(
        "--root",
        type=Path,
        required=True,
        help="Folder produced by run_search_vs_hard.ps1 (e.g., designs/tuning/search_vs_hard/<timestamp>)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        required=True,
        help="Path to write aggregate JSON results",
    )
    args = parser.parse_args()

    root = args.root
    if not root.exists():
        raise SystemExit(f"Root folder {root} does not exist")

    limits: Dict[str, object] = {}
    for entry in sorted(root.iterdir()):
        if not entry.is_dir():
            continue
        limits[entry.name] = analyze_limit(entry)

    payload = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "root": str(root),
        "limits": limits,
    }

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2))
    print(f"Wrote analysis to {args.out}")


if __name__ == "__main__":
    main()
