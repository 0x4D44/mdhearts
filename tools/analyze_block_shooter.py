#!/usr/bin/env python3
"""Analyze block-shooter pass outcomes by joining telemetry and deal logs.

Usage:
    python tools/analyze_block_shooter.py pass_v2=bench/out/stage2_pass_moon \
        pass_v1=bench/out/stage2_pass_moon_legacy --csv block_summary.csv \
        --json block_summary.json

Each run directory must contain `telemetry.jsonl` and `deals.jsonl` emitted by
the benchmarking harness.
"""

from __future__ import annotations

import argparse
import json
import math
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from statistics import mean
from typing import Dict, Iterable, List, Optional, Tuple


Key = Tuple[str, int, int]  # (run_id, hand_index, permutation_index)


@dataclass
class DealInfo:
    points: Dict[str, int]
    moon_shooter: Optional[str]
    moon_variant: Optional[str]


@dataclass
class BlockEvent:
    label: str
    run_id: str
    hand_index: int
    permutation_index: int
    seat: str
    probability: float
    total_score: float
    moon_shooter: Optional[str]
    moon_variant: Optional[str]
    seat_points: Optional[int]
    cards: Optional[str]


def parse_run_argument(raw: str) -> Tuple[str, Path]:
    if "=" not in raw:
        raise argparse.ArgumentTypeError(
            f"Run argument must be in label=path form, got '{raw}'"
        )
    label, path = raw.split("=", 1)
    if not label:
        raise argparse.ArgumentTypeError(f"Run label missing in '{raw}'")
    return label, Path(path)


def load_deals(path: Path) -> Tuple[Dict[Key, DealInfo], str, int]:
    deals: Dict[Key, Dict[str, int]] = {}
    run_id_hint: Optional[str] = None
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            payload = json.loads(line)
            run_id = payload["run_id"]
            run_id_hint = run_id_hint or run_id
            key = (run_id, payload["hand_index"], payload["permutation_index"])
            points = deals.setdefault(key, {})
            points[payload["seat"]] = int(payload["points"])

    info_map: Dict[Key, DealInfo] = {}
    for key, points in deals.items():
        shooter, variant = detect_moon(points)
        info_map[key] = DealInfo(points=points, moon_shooter=shooter, moon_variant=variant)

    unique_deals = len(info_map)
    return info_map, run_id_hint or "", unique_deals


def detect_moon(points: Dict[str, int]) -> Tuple[Optional[str], Optional[str]]:
    values = sorted(points.values())
    if values == [0, 26, 26, 26]:
        shooter = next(seat for seat, pts in points.items() if pts == 0)
        return shooter, "classic"
    if values == [0, 0, 0, 26]:
        shooter = next(seat for seat, pts in points.items() if pts == 26)
        return shooter, "inverted"
    return None, None


def load_block_events(
    telemetry_path: Path, deals: Dict[Key, DealInfo], run_id_hint: str, label: str
) -> List[BlockEvent]:
    pending: Dict[str, List[dict]] = defaultdict(list)
    events: List[BlockEvent] = []
    with telemetry_path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            payload = json.loads(line)
            target = payload.get("target", "")
            fields = payload.get("fields", {})

            if target == "hearts_bot::pass_decision":
                if fields.get("moon_objective") != "block_shooter":
                    continue
                seat = normalize_seat(fields.get("seat"))
                pending[seat].append(
                    {
                        "probability": float(fields.get("moon_probability", float("nan"))),
                        "total": float(fields.get("total", float("nan"))),
                        "cards": fields.get("cards"),
                    }
                )
            elif target == "hearts_bench::pass":
                seat = normalize_seat(fields.get("seat"))
                if not pending[seat]:
                    continue
                info = pending[seat].pop(0)
                run_id = fields.get("run_id", run_id_hint)
                key = (
                    run_id,
                    int(fields.get("hand_index", 0)),
                    int(fields.get("permutation_index", 0)),
                )
                deal = deals.get(key)
                seat_points = deal.points.get(seat) if deal else None
                events.append(
                    BlockEvent(
                        label=label,
                        run_id=run_id,
                        hand_index=key[1],
                        permutation_index=key[2],
                        seat=seat,
                        probability=info["probability"],
                        total_score=info["total"],
                        moon_shooter=deal.moon_shooter if deal else None,
                        moon_variant=deal.moon_variant if deal else None,
                        seat_points=seat_points,
                        cards=info.get("cards"),
                    )
                )
    return events


def normalize_seat(raw: Optional[str]) -> str:
    if raw is None:
        return ""
    return raw.strip().lower()


def bucket_probability(value: float) -> str:
    if math.isnan(value):
        return "unknown"
    edges = [0.0, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.01]
    for start, end in zip(edges, edges[1:]):
        if start <= value < end:
            return f"[{start:.1f},{end:.1f})"
    return "[1.0,1.0]"


def summarize_events(
    label: str, events: List[BlockEvent], deals: Dict[Key, DealInfo], unique_deals: int
) -> dict:
    total = len(events)
    successes = sum(1 for event in events if event.moon_shooter is None)
    failure_other = sum(
        1
        for event in events
        if event.moon_shooter is not None and event.moon_shooter != event.seat
    )
    failure_self = sum(
        1
        for event in events
        if event.moon_shooter is not None and event.moon_shooter == event.seat
    )
    probabilities = [event.probability for event in events if not math.isnan(event.probability)]

    bins: Dict[str, Dict[str, int]] = defaultdict(lambda: {"total": 0, "success": 0})
    for event in events:
        bucket = bucket_probability(event.probability)
        bins[bucket]["total"] += 1
        if event.moon_shooter is None:
            bins[bucket]["success"] += 1

    moon_occurrences = sum(1 for info in deals.values() if info.moon_shooter is not None)
    total_hands = unique_deals

    summary = {
        "label": label,
        "events": total,
        "success": successes,
        "failure_other": failure_other,
        "failure_self": failure_self,
        "success_rate": (successes / total) if total else math.nan,
        "moon_occurrences": moon_occurrences,
        "moon_rate_per_hand": (moon_occurrences / total_hands) if total_hands else math.nan,
        "avg_probability": mean(probabilities) if probabilities else math.nan,
        "bins": {
            bucket: {
                "total": data["total"],
                "success": data["success"],
                "success_rate": (data["success"] / data["total"])
                if data["total"]
                else math.nan,
            }
            for bucket, data in sorted(bins.items())
        },
    }
    return summary


def write_csv(rows: List[dict], path: Path) -> None:
    headers = [
        "label",
        "events",
        "success",
        "failure_other",
        "failure_self",
        "success_rate",
        "moon_occurrences",
        "moon_rate_per_hand",
        "avg_probability",
    ]
    with path.open("w", encoding="utf-8") as handle:
        handle.write(",".join(headers) + "\n")
        for row in rows:
            handle.write(
                ",".join(
                    [
                        str(row["label"]),
                        str(row["events"]),
                        str(row["success"]),
                        str(row["failure_other"]),
                        str(row["failure_self"]),
                        format_float(row["success_rate"]),
                        str(row["moon_occurrences"]),
                        format_float(row["moon_rate_per_hand"]),
                        format_float(row["avg_probability"]),
                    ]
                )
                + "\n"
            )


def format_float(value: float) -> str:
    if math.isnan(value):
        return "nan"
    return f"{value:.6f}"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "runs",
        nargs="+",
        help="Run directories in label=path form (e.g., pass_v2=bench/out/stage2_pass_moon)",
    )
    parser.add_argument("--csv", type=Path, help="Optional CSV summary output")
    parser.add_argument("--json", type=Path, help="Optional JSON summary output")
    args = parser.parse_args()

    summaries: List[dict] = []

    for raw in args.runs:
        label, run_dir = parse_run_argument(raw)
        telemetry_path = run_dir / "telemetry.jsonl"
        deals_path = run_dir / "deals.jsonl"
        if not telemetry_path.exists():
            raise FileNotFoundError(f"telemetry file not found: {telemetry_path}")
        if not deals_path.exists():
            raise FileNotFoundError(f"deals file not found: {deals_path}")

        deals, run_id_hint, unique_deals = load_deals(deals_path)
        events = load_block_events(telemetry_path, deals, run_id_hint, label)
        summary = summarize_events(label, events, deals, unique_deals)
        summaries.append(summary)

        print(f"Run: {label}")
        print(f"  Block-shooter pass events: {summary['events']}")
        print(f"  Success (no moon): {summary['success']}")
        print(f"  Failures (other seat shot moon): {summary['failure_other']}")
        print(f"  Failures (same seat shot moon): {summary['failure_self']}")
        print(f"  Success rate: {format_float(summary['success_rate'])}")
        print(f"  Moon occurrences (all deals): {summary['moon_occurrences']}")
        print(f"  Moon rate per hand: {format_float(summary['moon_rate_per_hand'])}")
        print("  Probability bins:")
        for bucket, data in summary["bins"].items():
            rate = format_float(data["success_rate"])
            print(
                f"    {bucket}: {data['success']} / {data['total']} success (rate {rate})"
            )
        print()

    if args.csv:
        write_csv(summaries, args.csv)
    if args.json:
        args.json.write_text(json.dumps(summaries, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
