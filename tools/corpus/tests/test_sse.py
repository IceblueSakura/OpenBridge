"""验证独立 SSE parser 对分片、冲突和未终止 event 的处理。"""

from __future__ import annotations

import base64
import json
import shutil
from pathlib import Path

from openbridge_corpus.corpuslib import generate_variants
from openbridge_corpus.sse import IncrementalSseParser, parse_sse


CORPUS_ROOT = Path(__file__).parents[3] / "testdata"


def test_incremental_parser_handles_every_generated_wire_variant(
    tmp_path: Path,
) -> None:
    root = tmp_path / "testdata"
    shutil.copytree(
        CORPUS_ROOT,
        root,
        ignore=shutil.ignore_patterns("generated", "reports", "dist", "runtime"),
    )
    generate_variants(root, seed=20260726)
    manifest = json.loads(
        (root / "generated" / "manifest.json").read_text(encoding="utf-8")
    )
    assert len(manifest["files"]) == 306
    for entry in manifest["files"]:
        payload = json.loads(
            (root / "generated" / entry["path"]).read_text(encoding="utf-8")
        )
        parser = IncrementalSseParser()
        events = []
        for encoded in payload["chunks_base64"]:
            events.extend(parser.feed(base64.b64decode(encoded)))
        parser.close()
        assert events, entry["path"]


def test_parser_preserves_event_and_payload_type_conflict() -> None:
    events = parse_sse(
        b"event: response.completed\n"
        b'data: {"type":"response.failed"}\n\n'
    )
    assert len(events) == 1
    assert events[0].event_field == "response.completed"
    assert events[0].payload_type == "response.failed"
    assert events[0].type_conflict is True
    assert events[0].terminal == "response_failed"


def test_eof_does_not_dispatch_unterminated_event() -> None:
    parser = IncrementalSseParser()
    assert parser.feed(b"data: {\"value\":1}\n") == []
    parser.close()
