#!/usr/bin/env python3
"""
Derive seat-aware continuation schedule hints from mixed-seat analyzer JSON.

Usage:
  python tools/fit_continuation_schedule.py --inputs tmp/search_vs_mixed/*.json --out tmp/continuation_fit.json
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Dict, List, Tuple


@dataclass
class SeatPoint:
    limit_ms: int
    penalty: float
    continuation_scale: float
    depth2_samples: float


@dataclass
class SeatFit:
    mix: str
    seat: str
    limits_ms: List[int]
    penalties: List[float]
    observed_continuation_scale: List[float]
    continuation_scale: List[int]
    depth2_samples: List[float]
    penalty_delta: float
    penalty_slope_per_10s: float
    continuation_delta: float
    avg_depth2: float
    needs_more_width: bool
    suggested_phaseb_topk: int
    suggested_ab_margin: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Fit continuation schedules from analyzer JSONs.")
    parser.add_argument(
        "--inputs",
        nargs="+",
        required=True,
        help="Paths to analyzer JSON files (e.g., tmp/search_vs_mixed/*.json)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        required=True,
        help="Output JSON path for fitted schedule data",
    )
    return parser.parse_args()


def load_points(path: Path) -> Dict[Tuple[str, str], List[SeatPoint]]:
    payload = json.loads(path.read_text())
    buckets: Dict[Tuple[str, str], List[SeatPoint]] = {}
    for mix_name, mix_values in payload.items():
        for limit_name, seats in mix_values.items():
            if limit_name == "seat_trends":
                continue
            try:
                limit_ms = int(limit_name.split("_")[1].replace("ms", ""))
            except (IndexError, ValueError):
                continue
            for seat_name, data in seats.items():
                penalties = data.get("penalties", {})
                telemetry = data.get("telemetry", {})
                stats = telemetry.get("search_stats", {})
                penalty = penalties.get("avg_pen")
                if penalty is None:
                    continue
                point = SeatPoint(
                    limit_ms=limit_ms,
                    penalty=float(penalty),
                    continuation_scale=float(stats.get("continuation_scale_permil", 0.0)),
                    depth2_samples=float(stats.get("depth2_samples", 0.0)),
                )
                buckets.setdefault((mix_name, seat_name), []).append(point)
    return buckets


def compute_fit(mix: str, seat: str, points: List[SeatPoint]) -> SeatFit:
    points = sorted(points, key=lambda p: p.limit_ms)
    limits = [p.limit_ms for p in points]
    penalties = [p.penalty for p in points]
    conts = [p.continuation_scale for p in points]
    depth2 = [p.depth2_samples for p in points]

    total_span = max(limits[-1] - limits[0], 1)
    penalty_delta = penalties[-1] - penalties[0]
    penalty_slope = penalty_delta / (total_span / 10_000)
    continuation_delta = conts[-1] - conts[0]
    avg_depth2 = sum(depth2) / len(depth2)

    needs_more_width = penalty_slope >= -0.1 or penalty_delta >= -0.25
    suggested_topk = 6
    if avg_depth2 >= 0.5:
        suggested_topk += 1
    if continuation_delta >= 100:
        suggested_topk += 1
    suggested_topk = min(suggested_topk, 10)

    suggested_ab = 150
    if penalty_slope >= 0:
        suggested_ab = max(100, int(150 * 0.85))
    if avg_depth2 > 1.0:
        suggested_ab = max(75, int(suggested_ab * 0.9))

    fitted_scales = derive_fitted_scales(limits, conts, penalty_slope, avg_depth2)

    return SeatFit(
        mix=mix,
        seat=seat,
        limits_ms=limits,
        penalties=penalties,
        observed_continuation_scale=conts,
        continuation_scale=fitted_scales,
        depth2_samples=depth2,
        penalty_delta=round(penalty_delta, 4),
        penalty_slope_per_10s=round(penalty_slope, 4),
        continuation_delta=round(continuation_delta, 2),
        avg_depth2=round(avg_depth2, 3),
        needs_more_width=needs_more_width,
        suggested_phaseb_topk=suggested_topk,
        suggested_ab_margin=suggested_ab,
    )


def derive_fitted_scales(
    limits: List[int],
    observed: List[float],
    penalty_slope: float,
    avg_depth2: float,
) -> List[int]:
    base_scales = [max(1000, int(round(value))) for value in observed]
    if avg_depth2 < 0.4:
        return base_scales
    extra = 0
    if penalty_slope >= -0.05:
        extra = 250
    elif penalty_slope >= -0.1:
        extra = 150
    elif penalty_slope >= -0.15:
        extra = 75
    if extra == 0:
        return base_scales
    fitted: List[int] = []
    for limit, observed_scale in zip(limits, base_scales):
        factor = 0.0
        if limit >= 20_000:
            factor = 1.0
        elif limit >= 15_000:
            factor = 0.75
        elif limit >= 10_000:
            factor = 0.5
        boost = int(round(extra * factor))
        fitted.append(min(1850, observed_scale + boost))
    for i in range(1, len(fitted)):
        if fitted[i] < fitted[i - 1]:
            fitted[i] = fitted[i - 1]
    return fitted


def main() -> None:
    args = parse_args()
    all_points: Dict[Tuple[str, str], List[SeatPoint]] = {}
    for input_path in args.inputs:
        path = Path(input_path)
        if not path.exists():
            raise SystemExit(f"{path} does not exist")
        for key, pts in load_points(path).items():
            all_points.setdefault(key, []).extend(pts)

    fits: Dict[str, Dict[str, Dict[str, object]]] = {}
    for (mix, seat), pts in all_points.items():
        if len(pts) < 2:
            continue
        fit = compute_fit(mix, seat, pts)
        fits.setdefault(mix, {})[seat] = asdict(fit)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(fits, indent=2))
    print(f"Wrote continuation fit to {args.out}")


if __name__ == "__main__":
    main()
