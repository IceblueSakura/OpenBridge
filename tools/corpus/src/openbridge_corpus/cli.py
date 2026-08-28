"""Command-line entry point for the OpenBridge protocol corpus and independent mock testkit."""

from __future__ import annotations

import argparse
import asyncio
import sys
from pathlib import Path

from .corpuslib import (
    CorpusError,
    dump_json,
    generate_variants,
    lint_corpus,
    load_json,
    pack_corpus,
    write_report,
)
from .mockclient import run_mock_client
from .mockserver import MockServer
from .plans import (
    build_client_plan,
    build_server_scenario,
    build_server_suite,
    validate_runtime_document,
)
from .semantic import verify_semantic_trace
from .semantic_plan import build_semantic_plan
from .verifier import verify_case_observations


def _parser() -> argparse.ArgumentParser:
    """Build parsers for corpus lint/generate/report/pack and mock subcommands."""
    # Build the shared corpus root and required subcommand entry points.
    parser = argparse.ArgumentParser(
        description=(
            "Validate and build the standalone protocol corpus, or run its "
            "HTTP/SSE mock tools."
        ),
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path("testdata"),
        help="Corpus root (default: ./testdata).",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # Register canonical corpus validation and derived-output commands.
    subparsers.add_parser("lint", help="Validate schemas, cases, artifacts, and provenance.")

    generate = subparsers.add_parser(
        "generate", help="Generate deterministic SSE wire variants."
    )
    generate.add_argument("--seed", type=int)
    generate.add_argument("--output", type=Path)

    report = subparsers.add_parser("report", help="Build corpus coverage metadata.")
    report.add_argument("--output", type=Path)

    pack = subparsers.add_parser("pack", help="Build a deterministic corpus ZIP.")
    pack.add_argument("--output", type=Path)

    # Register scenario, suite, and client-plan compilation commands.
    server_plan = subparsers.add_parser(
        "build-server-scenario",
        help="Compile a self-contained Mock Server scenario from one corpus case.",
    )
    server_plan.add_argument("--case", required=True)
    server_plan.add_argument("--variant", default="canonical")
    server_plan.add_argument("--chunk-delay-ms", type=int, default=0)
    server_plan.add_argument("--abort-delay-ms", type=int, default=10)
    server_plan.add_argument("--output", type=Path)

    server_suite = subparsers.add_parser(
        "build-server-suite",
        help="Compile an ordered multi-exchange Mock Server suite.",
    )
    server_suite.add_argument("--case", action="append", required=True)
    server_suite.add_argument("--suite-id", default="server-suite")
    server_suite.add_argument("--variant", default="canonical")
    server_suite.add_argument("--chunk-delay-ms", type=int, default=0)
    server_suite.add_argument("--abort-delay-ms", type=int, default=10)
    server_suite.add_argument("--output", type=Path)

    client_plan = subparsers.add_parser(
        "build-client-plan",
        help="Compile a self-contained Mock Client plan from one corpus case.",
    )
    client_plan.add_argument("--case", required=True)
    client_plan.add_argument("--base-url", required=True)
    client_plan.add_argument("--timeout-ms", type=int, default=5000)
    client_plan.add_argument("--output", type=Path)

    # Register mock-process execution and single-case observation verification commands.
    server = subparsers.add_parser(
        "mock-server", help="Run one precompiled Mock Server scenario."
    )
    server.add_argument("--scenario", type=Path, required=True)
    server.add_argument("--host", default="127.0.0.1")
    server.add_argument("--port", type=int, default=0)
    server.add_argument("--ready-file", type=Path)
    server.add_argument("--observation", type=Path)
    server.add_argument("--timeout-seconds", type=float, default=30)

    client = subparsers.add_parser(
        "mock-client", help="Run one precompiled Mock Client plan."
    )
    client.add_argument("--plan", type=Path, required=True)
    client.add_argument("--observation", type=Path)

    verify = subparsers.add_parser(
        "verify-observations",
        help="Compare one case's Mock Client/Server observations with its oracles.",
    )
    verify.add_argument("--case", required=True)
    verify.add_argument("--client-observation", type=Path, required=True)
    verify.add_argument("--server-observation", type=Path)

    semantic_verify = subparsers.add_parser(
        "verify-semantic-trace",
        help="Compare one normalized function-tool trace with its semantic oracle.",
    )
    semantic_verify.add_argument("--case", required=True)
    semantic_verify.add_argument("--trace", type=Path, required=True)

    semantic_plan = subparsers.add_parser(
        "build-semantic-plan",
        help="Compile a credential-free semantic execution plan.",
    )
    semantic_plan.add_argument("--case", required=True)
    semantic_plan.add_argument("--target-bytes", type=int)
    semantic_plan.add_argument(
        "--placement", choices=["start", "middle", "end"]
    )
    semantic_plan.add_argument("--output", type=Path)
    return parser


def _runtime_output(root: Path, output: Path | None, default_name: str) -> Path:
    """Resolve and constrain a derived runtime output path within the corpus runtime boundary."""
    runtime_root = (root / "runtime").resolve()
    candidate = (
        output.resolve() if output is not None else runtime_root / default_name
    )
    if candidate != runtime_root and runtime_root not in candidate.parents:
        raise CorpusError(f"runtime output must stay inside {runtime_root}")
    candidate.parent.mkdir(parents=True, exist_ok=True)
    return candidate


def _write_runtime(
    root: Path, output: Path | None, default_name: str, document: dict
) -> Path:
    """Write a runtime-derived document in stable JSON format."""
    path = _runtime_output(root, output, default_name)
    path.write_text(dump_json(document), encoding="utf-8", newline="\n")
    return path


async def _run_server(args: argparse.Namespace, root: Path) -> dict:
    """Start the mock server, wait for one scenario or suite, and return observations."""
    document = load_json(args.scenario)
    is_suite = "exchanges" in document
    validate_runtime_document(
        root, "server-suite" if is_suite else "server-scenario", document
    )
    if is_suite:
        for exchange in document["exchanges"]:
            validate_runtime_document(root, "server-scenario", exchange)
    server = MockServer(document, host=args.host, port=args.port)
    try:
        port = await server.start()
        ready = {
            "base_url": f"http://{args.host}:{port}",
            "exchange_count": len(server.scenarios),
            "health_url": f"http://{args.host}:{port}/healthz",
            "host": args.host,
            "port": port,
            "schema_version": "0.1",
        }
        if args.ready_file is not None:
            _write_runtime(root, args.ready_file, "server-ready.json", ready)
        print(dump_json(ready), end="", flush=True)
        observations = await server.wait_all(timeout=args.timeout_seconds)
        if not is_suite:
            return observations[0]
        return {
            "observations": observations,
            "role": "mock_server_run",
            "schema_version": "0.1",
            "suite_id": document["suite_id"],
        }
    finally:
        await server.close()


def main(argv: list[str] | None = None) -> int:
    """Run one corpus or mock testkit subcommand and return its process exit code."""
    # Parse the command and fix the corpus root for this invocation.
    args = _parser().parse_args(argv)
    root = args.root.resolve()
    try:
        # Validate the canonical corpus and build derived outputs.
        if args.command == "lint":
            errors = lint_corpus(root)
            if errors:
                for error in errors:
                    print(f"ERROR: {error}", file=sys.stderr)
                print(f"corpus lint failed with {len(errors)} error(s)", file=sys.stderr)
                return 1
            print("corpus lint passed")
            return 0
        if args.command == "generate":
            manifest = generate_variants(root, seed=args.seed, output=args.output)
            print(
                f"generated {len(manifest['files'])} variant file(s) "
                f"with seed {manifest['seed']}"
            )
            return 0
        if args.command == "report":
            report = write_report(root, output=args.output)
            print(dump_json(report), end="")
            return 0
        if args.command == "pack":
            output, digest = pack_corpus(root, output=args.output)
            print(f"packed {output} sha256={digest}")
            return 0

        # Compile reproducible Mock Server/Client runtime documents.
        if args.command == "build-server-scenario":
            scenario = build_server_scenario(
                root,
                args.case,
                variant=args.variant,
                chunk_delay_ms=args.chunk_delay_ms,
                abort_delay_ms=args.abort_delay_ms,
            )
            output = _write_runtime(
                root,
                args.output,
                f"{args.case}.server-scenario.json",
                scenario,
            )
            print(f"wrote {output}")
            return 0
        if args.command == "build-client-plan":
            plan = build_client_plan(
                root,
                args.case,
                base_url=args.base_url,
                timeout_ms=args.timeout_ms,
            )
            output = _write_runtime(
                root, args.output, f"{args.case}.client-plan.json", plan
            )
            print(f"wrote {output}")
            return 0
        if args.command == "build-server-suite":
            suite = build_server_suite(
                root,
                args.case,
                variant=args.variant,
                chunk_delay_ms=args.chunk_delay_ms,
                abort_delay_ms=args.abort_delay_ms,
                suite_id=args.suite_id,
            )
            output = _write_runtime(
                root, args.output, f"{args.suite_id}.server-suite.json", suite
            )
            print(f"wrote {output}")
            return 0
        if args.command == "build-semantic-plan":
            plan = build_semantic_plan(
                root,
                args.case,
                target_bytes=args.target_bytes,
                placement=args.placement,
            )
            validate_runtime_document(root, "semantic-plan", plan)
            output = _write_runtime(
                root, args.output, f"{args.case}.semantic-plan.json", plan
            )
            print(f"wrote {output}")
            return 0

        # Run the independent Mock Server/Client and write redacted observations.
        if args.command == "mock-server":
            observation = asyncio.run(_run_server(args, root))
            validate_runtime_document(
                root,
                (
                    "server-run-observation"
                    if observation["role"] == "mock_server_run"
                    else "observation"
                ),
                observation,
            )
            output = _write_runtime(
                root, args.observation, "server-observation.json", observation
            )
            print(f"wrote {output}")
            return 0
        if args.command == "mock-client":
            plan = load_json(args.plan)
            validate_runtime_document(root, "client-plan", plan)
            observation = asyncio.run(run_mock_client(plan))
            validate_runtime_document(root, "observation", observation)
            output = _write_runtime(
                root, args.observation, "client-observation.json", observation
            )
            print(f"wrote {output}")
            return 0

        # Compare generated single-case observations with canonical oracles.
        if args.command == "verify-observations":
            client_observation = load_json(args.client_observation)
            server_observation = (
                load_json(args.server_observation)
                if args.server_observation is not None
                else None
            )
            errors = verify_case_observations(
                root,
                args.case,
                client_observation=client_observation,
                server_observation=server_observation,
            )
            if errors:
                for error in errors:
                    print(f"ERROR: {error}", file=sys.stderr)
                print(
                    f"{args.case}: observations failed with {len(errors)} error(s)",
                    file=sys.stderr,
                )
                return 1
            print(f"{args.case}: observations passed")
            return 0
        if args.command == "verify-semantic-trace":
            trace = load_json(args.trace)
            errors = verify_semantic_trace(root, args.case, trace)
            if errors:
                for error in errors:
                    print(f"ERROR: {error}", file=sys.stderr)
                print(
                    f"{args.case}: semantic trace failed with {len(errors)} error(s)",
                    file=sys.stderr,
                )
                return 1
            print(f"{args.case}: semantic trace passed")
            return 0
    except CorpusError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
