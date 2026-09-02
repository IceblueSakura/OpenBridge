#!/usr/bin/env python3
"""Orchestrate bounded openbridge-probe generation runs and collect redacted reports.

The probe binary executes exactly one unit case per call; this script only loops
target x protocol x case selections, runs the binary per call, saves one redacted
JSON report per call, and renders one summary table. It never constructs request
bodies and never touches credentials (the probe reads config/upstream-credentials.toml
itself through the same bootstrap path as the service).

Runtime artifacts land under testdata/runtime/ (gitignored); canonical testdata is
never written.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PROBE_BIN = REPO_ROOT / "target" / "release" / "openbridge-probe"

# Closed probe case names, mirroring src/probe.rs ProbeGenerationCase::from_wire.
CHAT_CASES = [
    "text",
    "reasoning-none",
    "reasoning-minimal",
    "reasoning-low",
    "reasoning-medium",
    "reasoning-high",
    "reasoning-xhigh",
    "reasoning-max",
    "json-object",
    "json-schema",
    "json-schema-strict",
    "image-input-inline-png",
    "tool-auto",
    "tool-none",
    "tool-required",
    "tool-named",
    "tool-strict",
    "tool-parallel-false",
    "tool-parallel-true",
]
# Responses-only single-field differential cases (added on top of the Chat set).
RESPONSES_ONLY_CASES = ["reasoning-summary", "include-encrypted-content", "prompt-cache-key"]
# Both delivery modes are probed per case, matching the JSON/SSE evidence convention.
DELIVERIES = ["non-streaming", "streaming"]


@dataclass(frozen=True)
class Target:
    label: str
    provider: str
    target_id: str
    upstream_model: str


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run a bounded openbridge-probe matrix.")
    parser.add_argument("--out", required=True, help="output directory (created if missing)")
    parser.add_argument("--delay-seconds", type=float, default=2.0,
                        help="sleep between requests to stay polite to rate limits")
    parser.add_argument("--timeout-seconds", type=int, default=600,
                        help="per-request subprocess timeout")
    parser.add_argument("--case", action="append", default=None,
                        help="restrict to these case names (repeatable)")
    parser.add_argument("--protocol", choices=["chat", "responses", "both"], default="both")
    parser.add_argument("--delivery", choices=["non-streaming", "streaming", "both"],
                        default="both")
    parser.add_argument("--target-label", action="append", default=None,
                        help="restrict to these target labels (repeatable)")
    parser.add_argument("--dry-run", action="store_true", help="print the planned calls only")
    return parser


def planned_calls(targets: list[Target], protocols: list[str], deliveries: list[str],
                  cases: list[str] | None):
    calls = []
    for target in targets:
        for protocol in protocols:
            protocol_cases = list(cases or CHAT_CASES)
            if protocol == "responses":
                protocol_cases += [c for c in RESPONSES_ONLY_CASES if not cases or c in cases]
            if cases and protocol == "chat":
                protocol_cases = [c for c in protocol_cases if c in cases]
            for case in protocol_cases:
                if protocol == "chat" and case in RESPONSES_ONLY_CASES:
                    continue
                for delivery in deliveries:
                    calls.append((target, protocol, case, delivery))
    return calls


def run_probe(target: Target, protocol: str, case: str, delivery: str, report_path: Path,
              timeout_seconds: int) -> dict:
    command = [
        str(PROBE_BIN), "generation",
        "--provider", target.provider,
        "--target", target.target_id,
        "--model", target.upstream_model,
        "--protocol", protocol,
        "--delivery", delivery,
        "--case", case,
    ]
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command, cwd=REPO_ROOT, capture_output=True, text=True, timeout=timeout_seconds,
        )
        elapsed = time.monotonic() - started
        stdout = completed.stdout.strip()
        report = json.loads(stdout) if stdout else None
        if report is not None:
            report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        return {
            "exit_code": completed.returncode,
            "elapsed_seconds": round(elapsed, 1),
            "report": report,
            "stderr_tail": completed.stderr.strip().splitlines()[-1] if completed.stderr.strip() else None,
        }
    except subprocess.TimeoutExpired:
        return {"exit_code": None, "elapsed_seconds": timeout_seconds, "report": None,
                "stderr_tail": "subprocess timeout"}
    except json.JSONDecodeError:
        return {"exit_code": 0, "elapsed_seconds": round(time.monotonic() - started, 1),
                "report": None, "stderr_tail": "unparseable stdout"}


def outcome_of(report: dict | None) -> str:
    if report is None:
        return "no-report"
    generation = report.get("generation") or {}
    outcome = generation.get("outcome") or {}
    state = outcome.get("state", "unknown")
    failure = outcome.get("failure")
    return f"{state}" if not failure else f"{state}/{failure}"


def verdict_of(report: dict | None) -> str:
    if report is None:
        return "-"
    evidence = (report.get("generation") or {}).get("capability_evidence")
    if evidence is None:
        return "-"
    return evidence.get("verdict", "-")


def main() -> int:
    args = build_parser().parse_args()

    # The 2026-09-02 focus matrix: four dual-protocol registered targets.
    targets = [
        Target("deepseek-v4-flash-vision-exp", "deepseek", "deepseek-v4-flash-vision-exp",
               "deepseek-v4-flash-vision-exp"),
        Target("mimo-v2.5", "mimo", "mimo-v2-5", "mimo-v2.5"),
        Target("glm-5.3-flash", "zhipu-cn", "zhipu-cn/glm-5-3-flash", "glm-5.3-flash"),
        Target("qwen3.8-max", "bailian", "bailian/qwen3-8-max", "qwen3.8-max"),
    ]
    if args.target_label:
        targets = [t for t in targets if t.label in args.target_label]
    protocols = ["chat", "responses"] if args.protocol == "both" else [args.protocol]
    deliveries = (DELIVERIES if args.delivery == "both" else [args.delivery])

    calls = planned_calls(targets, protocols, deliveries, args.case)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    plan_path = out / "plan.json"
    plan_path.write_text(json.dumps(
        [{"label": t.label, "provider": t.provider, "target": t.target_id,
          "upstream_model": t.upstream_model, "protocol": p, "case": c, "delivery": d}
         for (t, p, c, d) in calls], ensure_ascii=False, indent=2) + "\n")
    print(f"planned {len(calls)} probe calls -> {out}")

    if args.dry_run:
        for (t, p, c, d) in calls:
            print(f"  {t.label:32s} {p:9s} {d:14s} {c}")
        return 0

    rows = []
    for index, (target, protocol, case, delivery) in enumerate(calls, 1):
        delivery_tag = "sse" if delivery == "streaming" else "json"
        report_path = out / f"{target.label}_{protocol}_{case}_{delivery_tag}.json"
        print(f"[{index}/{len(calls)}] {target.label} {protocol}/{case}/{delivery_tag} ...",
              flush=True)
        result = run_probe(target, protocol, case, delivery, report_path,
                           args.timeout_seconds)
        report = result["report"]
        row = {
            "label": target.label, "protocol": protocol, "case": case,
            "delivery": delivery_tag,
            "outcome": outcome_of(report), "verdict": verdict_of(report),
            "elapsed_seconds": result["elapsed_seconds"],
            "reasoning_observed": ((report or {}).get("generation") or {}).get("evidence", {}).get("reasoning_observed") if report else None,
            "reasoning_summary_observed": ((report or {}).get("generation") or {}).get("evidence", {}).get("reasoning_summary_observed") if report else None,
            "report_file": report_path.name if report else None,
            "error": result["stderr_tail"],
        }
        rows.append(row)
        # Append incrementally so an interrupted run keeps its progress.
        (out / "results.jsonl").open("a").write(json.dumps(row, ensure_ascii=False) + "\n")
        print(f"    outcome={row['outcome']} verdict={row['verdict']} "
              f"elapsed={row['elapsed_seconds']}s")
        if index < len(calls):
            time.sleep(args.delay_seconds)

    (out / "summary.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")
    failures = [r for r in rows if not r["outcome"].startswith("accepted")]
    print(f"\ndone: {len(rows)} calls, {len(failures)} non-accepted")
    for row in failures:
        print(f"  {row['label']} {row['protocol']}/{row['case']}: {row['outcome']} {row['error'] or ''}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
