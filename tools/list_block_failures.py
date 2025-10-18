#!/usr/bin/env python3
"""List block-shooter failure cases above a probability threshold."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Iterable, List

SCRIPT_DIR = Path(__file__).resolve().parent

if str(SCRIPT_DIR) not in __import__("sys").path:
    __import__("sys").path.insert(0, str(SCRIPT_DIR))

from analyze_block_shooter import (  # noqa: E402
    BlockEvent,
    load_block_events,
    load_deals,
    parse_run_argument,
)


def gather_failures(events: List[BlockEvent], threshold: float) -> List[BlockEvent]:
    failures = []
    for event in events:
        if event.moon_shooter is None:
            continue
        if event.probability != event.probability:  # NaN check
            continue
        if event.probability >= threshold:
            failures.append(event)
    return failures


def render_markdown(failures: List[BlockEvent]) -> str:
    lines = [
        "| Run | Hand | Perm | Seat | Probability | Total Score | Moon Shooter | Variant | Seat Points |",
        "| --- | ---: | ---: | --- | --- | --- | --- | --- | ---: |",
    ]
    for event in failures:
        lines.append(
            "| {label} | {hand} | {perm} | {seat} | {prob:.3f} | {total:.1f} | {shooter} | {variant} | {points} |".format(
                label=event.label,
                hand=event.hand_index,
                perm=event.permutation_index,
                seat=event.seat,
                prob=event.probability,
                total=event.total_score,
                shooter=event.moon_shooter or "<unknown>",
                variant=event.moon_variant or "<unknown>",
                points=event.seat_points if event.seat_points is not None else 0,
            )
        )
    if len(lines) == 2:
        lines.append("| — | — | — | — | — | — | — | — | — |")
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "runs",
        nargs="+",
        help="Run directories in label=path form (requires telemetry.jsonl and deals.jsonl).",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.6,
        help="Minimum moon probability to include (default: 0.6).",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        help="Optional output path for Markdown table.",
    )
    args = parser.parse_args()

    failures: List[BlockEvent] = []
    for raw in args.runs:
        label, path = parse_run_argument(raw)
        deals, run_id_hint, _ = load_deals(path / "deals.jsonl")
        events = load_block_events(path / "telemetry.jsonl", deals, run_id_hint, label)
        failures.extend(gather_failures(events, args.threshold))

    failures.sort(key=lambda evt: (evt.label, evt.hand_index, evt.permutation_index))

    for event in failures:
        print(
            f"{event.label} hand={event.hand_index} perm={event.permutation_index} "
            f"seat={event.seat} prob={event.probability:.3f} total={event.total_score:.1f} "
            f"shooter={event.moon_shooter or '<unknown>'} variant={event.moon_variant or '<unknown>'}"
        )

    if args.markdown:
        args.markdown.write_text(render_markdown(failures), encoding="utf-8")


if __name__ == "__main__":
    main()
