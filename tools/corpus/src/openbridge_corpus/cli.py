from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .corpuslib import (
    CorpusError,
    dump_json,
    generate_variants,
    lint_corpus,
    pack_corpus,
    write_report,
)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="corpus",
        description="Validate and build the standalone OpenBridge protocol corpus.",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path("testdata"),
        help="Corpus root (default: ./testdata).",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("lint", help="Validate schemas, cases, artifacts, and provenance.")

    generate = subparsers.add_parser(
        "generate", help="Generate deterministic SSE byte-fragmentation variants."
    )
    generate.add_argument("--seed", type=int)
    generate.add_argument("--output", type=Path)

    report = subparsers.add_parser("report", help="Build corpus coverage metadata.")
    report.add_argument("--output", type=Path)

    pack = subparsers.add_parser("pack", help="Build a deterministic corpus ZIP.")
    pack.add_argument("--output", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    root = args.root.resolve()
    try:
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
    except CorpusError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
