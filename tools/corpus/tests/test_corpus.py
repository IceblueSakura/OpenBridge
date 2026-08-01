"""验证 canonical corpus 的 schema、语义、生成、报告和打包边界。"""

from __future__ import annotations

import base64
import json
import shutil
import zipfile
from pathlib import Path

from openbridge_corpus.corpuslib import (
    CorpusError,
    build_report,
    generate_variants,
    lint_corpus,
    pack_corpus,
    terminal_kinds,
    write_report,
)


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CORPUS_ROOT = REPOSITORY_ROOT / "testdata"


def _copy_corpus(destination: Path) -> Path:
    target = destination / "testdata"
    shutil.copytree(
        CORPUS_ROOT,
        target,
        ignore=shutil.ignore_patterns("generated", "reports", "dist", "runtime"),
    )
    return target


def test_repository_corpus_lints() -> None:
    assert lint_corpus(CORPUS_ROOT) == []


def test_lint_reports_catalog_mismatch_and_suspected_secret(tmp_path: Path) -> None:
    root = _copy_corpus(tmp_path)
    catalog_path = root / "catalog.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
    catalog["case_ids"] = catalog["case_ids"][1:]
    catalog_path.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    case = next((root / "cases").rglob("client-request.json"))
    case.write_text(
        '{"authorization":"Bearer sk-proj-abcdefghijklmnop"}\n',
        encoding="utf-8",
    )
    errors = lint_corpus(root)
    assert any("catalog/case mismatch" in error for error in errors)
    assert any("suspected secret" in error for error in errors)


def test_lint_rejects_duplicate_keys_and_undeclared_case_files(
    tmp_path: Path,
) -> None:
    root = _copy_corpus(tmp_path)
    case_directory = next((root / "cases").rglob("*.text.non_stream"))
    request = case_directory / "client-request.json"
    request.write_text('{"model":"one","model":"two"}\n', encoding="utf-8")
    (case_directory / "untracked-oracle.json").write_text("{}\n", encoding="utf-8")
    errors = lint_corpus(root)
    assert any("duplicate object key 'model'" in error for error in errors)
    assert any("undeclared case file untracked-oracle.json" in error for error in errors)


def test_lint_rejects_artifact_escape_and_inconsistent_stream_contract(
    tmp_path: Path,
) -> None:
    root = _copy_corpus(tmp_path)
    case_path = next((root / "cases").rglob("*.text.stream/case.json"))
    case = json.loads(case_path.read_text(encoding="utf-8"))
    case["artifacts"]["expected_client_response"] = "../outside.json"
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    errors = lint_corpus(root)
    assert any("path escapes allowed root" in error for error in errors)
    assert any("streaming case has non-stream artifacts" in error for error in errors)


def test_lint_returns_error_for_malformed_case_manifest(tmp_path: Path) -> None:
    root = _copy_corpus(tmp_path)
    case_path = next((root / "cases").rglob("case.json"))
    case_path.write_text("{", encoding="utf-8")
    errors = lint_corpus(root)
    assert any("cannot read JSON" in error for error in errors)


def test_terminal_parser_distinguishes_chat_and_responses() -> None:
    chat = b'data: {"choices":[{"delta":{"content":"ok"}}]}\n\ndata: [DONE]\n\n'
    responses = (
        b'event: response.completed\n'
        b'data: {"type":"response.completed","response":{"id":"resp_1"}}\n\n'
    )
    assert terminal_kinds(chat) == ["chat_done"]
    assert terminal_kinds(responses) == ["response_completed"]


def test_generation_is_deterministic_and_reconstructs_sources(tmp_path: Path) -> None:
    root = _copy_corpus(tmp_path)
    first = generate_variants(root, seed=1234)
    first_manifest = (root / "generated" / "manifest.json").read_bytes()
    second = generate_variants(root, seed=1234)
    second_manifest = (root / "generated" / "manifest.json").read_bytes()
    assert first == second
    assert first_manifest == second_manifest
    kinds: set[str] = set()
    for entry in first["files"]:
        payload = json.loads((root / "generated" / entry["path"]).read_text())
        kinds.add(payload["kind"].split("_", 1)[0])
        assert payload["wire_sha256"] == payload["reconstructed_sha256"]
        chunks = [base64.b64decode(chunk) for chunk in payload["chunks_base64"]]
        if payload["transformation"] == "none":
            assert payload["canonical_sha256"] == payload["wire_sha256"]
        else:
            assert payload["kind"] == "crlf"
            wire = b"".join(chunks)
            assert b"\r\n" in wire
            assert b"\r" not in wire.replace(b"\r\n", b"")
            assert b"\n" not in wire.replace(b"\r\n", b"")
            terminal_kinds(wire)
        if payload["kind"] == "all_in_one":
            assert len(chunks) == 1
    assert {
        "one",
        "line",
        "utf8",
        "all",
        "event",
        "crlf",
        "seeded",
    } <= kinds


def test_report_confirms_p0_feature_and_generation_coverage() -> None:
    report = build_report(CORPUS_ROOT)
    assert report["case_count"] == 45
    assert report["missing_required_features"] == []
    assert report["missing_required_generation_kinds"] == []


def test_lint_rejects_inconsistent_transport_failure_phase(tmp_path: Path) -> None:
    root = _copy_corpus(tmp_path)
    case_path = next(
        (root / "cases").rglob(
            "responses_native.transport_error.before_output/case.json"
        )
    )
    case = json.loads(case_path.read_text(encoding="utf-8"))
    case["transport"]["downstream_output_observed"] = True
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    errors = lint_corpus(root)
    assert any(
        "before-output failure cannot observe downstream output" in error
        for error in errors
    )


def test_lint_rejects_unmarked_sse_type_conflicts_and_post_terminal_events(
    tmp_path: Path,
) -> None:
    root = _copy_corpus(tmp_path)
    case_directory = next(
        (root / "cases").rglob("chat_to_responses.text.stream")
    )
    upstream = case_directory / "upstream-stream.sse"
    raw = upstream.read_text(encoding="utf-8")
    raw = raw.replace(
        "event: response.created",
        "event: response.output_text.delta",
        1,
    )
    raw += (
        "event: response.output_text.delta\n"
        'data: {"type":"response.output_text.delta","delta":"late"}\n\n'
    )
    upstream.write_text(raw, encoding="utf-8")
    errors = lint_corpus(root)
    assert any("SSE event/data type conflicts" in error for error in errors)
    assert any("event(s) occur after terminal" in error for error in errors)


def test_derived_outputs_cannot_overwrite_canonical_corpus(tmp_path: Path) -> None:
    root = _copy_corpus(tmp_path)
    for operation in (
        lambda: generate_variants(root, output=root / "cases"),
        lambda: write_report(root, output=root / "catalog.json"),
        lambda: pack_corpus(root, output=root / "VERSION"),
    ):
        try:
            operation()
        except CorpusError as error:
            assert "output must stay inside" in str(error)
        else:
            raise AssertionError("unsafe derived output was accepted")
    assert (root / "catalog.json").is_file()
    assert (root / "VERSION").is_file()
    assert any((root / "cases").rglob("case.json"))


def test_pack_is_deterministic_and_excludes_derived_directories(
    tmp_path: Path,
) -> None:
    root = _copy_corpus(tmp_path)
    generate_variants(root, seed=1234)
    (root / "reports").mkdir()
    (root / "reports" / "coverage.json").write_text("{}\n", encoding="utf-8")
    (root / "runtime").mkdir()
    (root / "runtime" / "observation.json").write_text("{}\n", encoding="utf-8")
    first, first_digest = pack_corpus(root, root / "dist" / "first.zip")
    second, second_digest = pack_corpus(root, root / "dist" / "second.zip")
    assert first_digest == second_digest
    assert (root / "dist" / "first.zip.sha256").read_text(
        encoding="ascii"
    ).startswith(first_digest)
    with zipfile.ZipFile(first) as archive:
        names = archive.namelist()
    assert "manifest.json" in names
    assert not any(name.startswith("generated/") for name in names)
    assert not any(name.startswith("reports/") for name in names)
    assert not any(name.startswith("dist/") for name in names)
    assert not any(name.startswith("runtime/") for name in names)
