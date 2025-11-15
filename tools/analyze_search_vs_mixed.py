#!/usr/bin/env python3
"""
Aggregate metrics for mixed-seat Search vs Hard experiments produced by
tools/run_search_vs_mixed.ps1.
"""

from __future__ import annotations

import argparse
import csv
import json
from collections import defaultdict
from dataclasses import dataclass, asdict
from pathlib import Path
import re
from statistics import pstdev
from typing import Dict, List, Tuple


@dataclass
class SeatSummary:
    seat: str
    count: int
    avg_pen: float
    std_pen: float


@dataclass
class TelemetrySummary:
    records: int
    post_records: int
    timed_out: int
    search_stats: Dict[str, float]
    depth2_samples: int
    mix_hint_bias: Dict[str, float]
    controller_bias_avg: float


def parse_match_csv(path: Path) -> SeatSummary:
    seat = path.stem.split("_")[2]
    penalties: List[float] = []
    with path.open(newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            value = row.get("pen") or row.get("penalty") or row.get("a_pen")
            if value is None:
                raise RuntimeError(f"pen column missing in {path}")
            penalties.append(float(value))
    if not penalties:
        raise RuntimeError(f"no rows parsed from {path}")
    count = len(penalties)
    avg_pen = sum(penalties) / count
    std_pen = pstdev(penalties)
    return SeatSummary(seat=seat, count=count, avg_pen=round(avg_pen, 3), std_pen=round(std_pen, 3))


def parse_telemetry(path: Path) -> TelemetrySummary:
    if not path.exists():
        return TelemetrySummary(
            records=0,
            post_records=0,
            timed_out=0,
            search_stats={},
            depth2_samples=0,
            mix_hint_bias={},
            controller_bias_avg=0.0,
        )
    records = 0
    post_records = 0
    timed_out = 0
    search_count = 0
    search_totals = {
        "scanned": 0,
        "scanned_phase_a": 0,
        "scanned_phase_b": 0,
        "scanned_phase_c": 0,
        "utilization": 0,
        "continuation_scale_permil": 0,
        "controller_bias_delta": 0,
    }
    depth2_total = 0
    bias_totals: Dict[str, int] = {}
    controller_bias_total = 0
    controller_bias_count = 0
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
            if "controller_bias_delta" in payload and payload["controller_bias_delta"] is not None:
                controller_bias_total += int(payload["controller_bias_delta"])
                controller_bias_count += 1
            stats = payload.get("search_stats")
            if stats:
                search_count += 1
                for key in (
                    "scanned",
                    "scanned_phase_a",
                    "scanned_phase_b",
                    "scanned_phase_c",
                    "continuation_scale_permil",
                ):
                    search_totals[key] += int(stats.get(key, 0))
                search_totals["utilization"] += int(stats.get("utilization", 0))
                depth2_total += int(stats.get("depth2_samples", 0))
                bias = stats.get("mix_hint_bias")
                if isinstance(bias, dict):
                    for key, value in bias.items():
                        bias_totals[key] = bias_totals.get(key, 0) + int(value)
                search_totals["controller_bias_delta"] += int(stats.get("controller_bias_delta", 0))
    averages = {}
    if search_count:
        averages = {
            key: round(value / search_count, 2)
            for key, value in search_totals.items()
        }
        averages["samples"] = search_count
    avg_depth2 = 0
    if search_count:
        avg_depth2 = round(depth2_total / search_count, 2)
        averages["depth2_samples"] = avg_depth2
    bias_averages: Dict[str, float] = {}
    if search_count:
        bias_averages = {
            key: round(value / search_count, 3) for key, value in bias_totals.items()
        }
    controller_bias_avg = (
        round(controller_bias_total / controller_bias_count, 2)
        if controller_bias_count
        else 0.0
    )
    return TelemetrySummary(
        records=records,
        post_records=post_records,
        timed_out=timed_out,
        search_stats=averages,
        depth2_samples=depth2_total,
        mix_hint_bias=bias_averages,
        controller_bias_avg=controller_bias_avg,
    )


def analyze_limit(limit_dir: Path) -> Dict[str, Dict[str, object]]:
    summaries: Dict[str, SeatSummary] = {}
    for match_csv in limit_dir.glob("match_*.csv"):
        summary = parse_match_csv(match_csv)
        summaries[summary.seat] = summary

    telemetry: Dict[str, TelemetrySummary] = {}
    for telem in limit_dir.glob("telemetry_*.jsonl"):
        seat = telem.stem.split("_")[-1]
        telemetry[seat] = parse_telemetry(telem)

    output: Dict[str, Dict[str, object]] = {}
    for seat, summary in summaries.items():
        telemetry_summary = telemetry.get(
            seat,
            TelemetrySummary(
                records=0,
                post_records=0,
                timed_out=0,
                search_stats={},
                depth2_samples=0,
                mix_hint_bias={},
                controller_bias_avg=0.0,
            ),
        )
        output[seat] = {
            "penalties": asdict(summary),
            "telemetry": asdict(telemetry_summary),
        }
    return output


def limit_name_to_ms(name: str) -> int:
    match = re.match(r"(?:smoke_)?limit_(\d+)ms", name)
    if not match:
        raise RuntimeError(f"Unexpected limit directory name: {name}")
    return int(match.group(1))


def summarize_seat_trends(
    mix_payload: Dict[str, Dict[str, Dict[str, object]]]
) -> Dict[str, Dict[str, object]]:
    bucketed: Dict[str, List[Tuple[int, float, float, float]]] = defaultdict(list)
    for limit_name, seats in mix_payload.items():
        if limit_name == "seat_trends":
            continue
        limit_ms = limit_name_to_ms(limit_name)
        for seat, data in seats.items():
            penalties = data["penalties"]["avg_pen"]
            cont_scale = data["telemetry"].get("search_stats", {}).get("continuation_scale_permil", 0.0)
            depth2_avg = data["telemetry"].get("search_stats", {}).get("depth2_samples", 0.0)
            bucketed[seat].append((limit_ms, penalties, cont_scale, depth2_avg))
    summaries: Dict[str, Dict[str, object]] = {}
    for seat, entries in bucketed.items():
        entries.sort(key=lambda item: item[0])
        limits = [item[0] for item in entries]
        penalties = [item[1] for item in entries]
        cont_scales = [item[2] for item in entries]
        depth2_samples = [item[3] for item in entries]
        first_limit = limits[0]
        last_limit = limits[-1]
        base_delta_ms = max(last_limit - first_limit, 1)
        penalty_delta = penalties[-1] - penalties[0]
        penalty_slope_per_10s = round(penalty_delta / (base_delta_ms / 10_000), 4)
        cont_delta = cont_scales[-1] - cont_scales[0]
        cont_eff = round(penalty_delta / cont_delta, 4) if cont_delta not in (0, 0.0) else None
        per_limit_deltas = {
            f"{limits[i-1]}->{limits[i]}": round(penalties[i] - penalties[i - 1], 4)
            for i in range(1, len(limits))
        }
        avg_depth2 = (
            round(sum(depth2_samples) / len(depth2_samples), 3) if depth2_samples else 0.0
        )
        depth2_threshold = 0.4
        slope_threshold = -0.15
        depth2_triggered = avg_depth2 >= depth2_threshold
        fails_goals = depth2_triggered and penalty_slope_per_10s >= slope_threshold
        summaries[seat] = {
            "limits_ms": limits,
            "avg_penalties": penalties,
            "penalty_delta": round(penalty_delta, 4),
            "penalty_slope_per_10s": penalty_slope_per_10s,
            "continuation_scale_delta": round(cont_delta, 4),
            "penalty_per_cont_delta": cont_eff,
            "per_limit_deltas": per_limit_deltas,
            "avg_depth2_samples": avg_depth2,
            "fails_goals": fails_goals,
            "goal_checks": {
                "depth2_triggered": depth2_triggered,
                "depth2_threshold": depth2_threshold,
                "slope_threshold": slope_threshold,
                "observed_depth2_avg": avg_depth2,
                "observed_slope": penalty_slope_per_10s,
            },
        }
    return summaries


def main() -> None:
    parser = argparse.ArgumentParser(description="Analyze search_vs_mixed artifacts")
    parser.add_argument("--root", type=Path, required=True, help="Path to search_vs_mixed/<timestamp> folder")
    parser.add_argument("--out", type=Path, required=True, help="Output JSON path")
    args = parser.parse_args()

    root = args.root
    if not root.exists():
        raise SystemExit(f"{root} does not exist")

    payload: Dict[str, Dict[str, Dict[str, object]]] = {}
    for mix_dir in sorted([p for p in root.iterdir() if p.is_dir()]):
        mix_name = mix_dir.name
        payload[mix_name] = {}
        limit_dirs = sorted([p for p in mix_dir.iterdir() if p.is_dir()])
        for limit_dir in limit_dirs:
            payload[mix_name][limit_dir.name] = analyze_limit(limit_dir)
        payload[mix_name]["seat_trends"] = summarize_seat_trends(payload[mix_name])

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2))
    print(f"Wrote analysis to {args.out}")


if __name__ == "__main__":
    main()
