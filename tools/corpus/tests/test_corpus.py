"""Validate canonical corpus schemas, semantics, generation, reports, and packaging boundaries."""

from __future__ import annotations

import base64
import json
import shutil
import zipfile
from pathlib import Path

import pytest

import openbridge_corpus.corpuslib as corpuslib
from openbridge_corpus import __version__
from openbridge_corpus.corpuslib import (
    CorpusError,
    build_report,
    discover_cases,
    generate_variants,
    lint_corpus,
    pack_corpus,
    terminal_kinds,
    write_report,
)


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CORPUS_ROOT = REPOSITORY_ROOT / "testdata"


def _copy_corpus(destination: Path) -> Path:
    """Copy the canonical corpus while excluding all derived-output directories."""
    target = destination / "testdata"
    shutil.copytree(
        CORPUS_ROOT,
        target,
        ignore=shutil.ignore_patterns("generated", "reports", "dist", "runtime"),
    )
    return target


def test_repository_corpus_lints() -> None:
    """Verify that the repository canonical corpus passes all current lint rules."""
    assert lint_corpus(CORPUS_ROOT) == []


def test_package_and_corpus_versions_match() -> None:
    """Keep the Python package metadata aligned with the canonical corpus release."""
    assert __version__ == (CORPUS_ROOT / "VERSION").read_text(encoding="utf-8").strip()


def test_lint_reports_catalog_mismatch_and_suspected_secret(tmp_path: Path) -> None:
    """Verify that catalog mismatches and suspected credentials are reported by lint."""
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
    """Verify that duplicate JSON keys and undeclared case files are rejected."""
    root = _copy_corpus(tmp_path)
    case_directory = next((root / "cases").rglob("*.text.non_stream"))
    request = case_directory / "client-request.json"
    request.write_text('{"model":"one","model":"two"}\n', encoding="utf-8")
    (case_directory / "untracked-oracle.json").write_text("{}\n", encoding="utf-8")
    errors = lint_corpus(root)
    assert any("duplicate object key" in error for error in errors)
    assert all("model" not in error for error in errors if "duplicate object key" in error)
    assert any("undeclared case file untracked-oracle.json" in error for error in errors)


@pytest.mark.parametrize("constant", ["NaN", "Infinity", "-Infinity"])
def test_strict_json_rejects_non_finite_numbers(
    tmp_path: Path, constant: str
) -> None:
    """Reject non-standard numeric constants in canonical and runtime JSON."""
    path = tmp_path / "non-finite.json"
    path.write_text(f'{{"value":{constant}}}\n', encoding="utf-8")
    with pytest.raises(CorpusError, match="non-finite JSON number"):
        corpuslib.load_json(path)
    with pytest.raises(ValueError):
        corpuslib.dump_json({"value": float("nan")})


def test_json_input_limits_are_enforced(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Bound file size, tree depth/node count, and individual string size."""
    path = tmp_path / "bounded.json"

    monkeypatch.setattr(corpuslib, "MAX_CORPUS_FILE_BYTES", 32)
    path.write_text(json.dumps({"value": "x" * 64}), encoding="utf-8")
    with pytest.raises(CorpusError, match="file exceeds"):
        corpuslib.load_json(path)

    monkeypatch.setattr(corpuslib, "MAX_CORPUS_FILE_BYTES", 1024)
    monkeypatch.setattr(corpuslib, "MAX_JSON_STRING_BYTES", 4)
    path.write_text(json.dumps({"value": "12345"}), encoding="utf-8")
    with pytest.raises(CorpusError, match="string exceeds"):
        corpuslib.load_json(path)

    monkeypatch.setattr(corpuslib, "MAX_JSON_STRING_BYTES", 1024)
    monkeypatch.setattr(corpuslib, "MAX_JSON_NODES", 2)
    path.write_text(json.dumps({"one": 1, "two": 2}), encoding="utf-8")
    with pytest.raises(CorpusError, match="node limit"):
        corpuslib.load_json(path)

    monkeypatch.setattr(corpuslib, "MAX_JSON_NODES", 100)
    monkeypatch.setattr(corpuslib, "MAX_JSON_DEPTH", 1)
    path.write_text(json.dumps({"outer": {"inner": 1}}), encoding="utf-8")
    with pytest.raises(CorpusError, match="depth limit"):
        corpuslib.load_json(path)


def test_sse_json_and_event_limits_are_enforced() -> None:
    """Apply strict JSON complexity and stream-count bounds inside SSE data fields."""
    deeply_nested = ("[" * 5000 + "0" + "]" * 5000).encode("ascii")
    with pytest.raises(CorpusError, match="strict policy"):
        corpuslib._parse_sse_events(b"data: " + deeply_nested + b"\n\n")

    with pytest.raises(CorpusError, match="strict policy"):
        corpuslib._parse_sse_events(b"data: {\"value\":NaN}\n\n")

    too_many_events = b"data: {}\n\n" * (corpuslib.MAX_SSE_EVENTS + 1)
    with pytest.raises(CorpusError, match="event limit"):
        corpuslib._parse_sse_events(too_many_events)

    too_many_blocks = b": keepalive\n\n" * corpuslib.MAX_SSE_BLOCKS
    with pytest.raises(CorpusError, match="block limit"):
        corpuslib._parse_sse_events(too_many_blocks)


def test_lint_rejects_artifact_escape_and_inconsistent_stream_contract(
    tmp_path: Path,
) -> None:
    """Verify that out-of-bounds artifacts and stream/non-stream semantic conflicts are rejected."""
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
    """Verify that a damaged case manifest becomes a readable corpus error."""
    root = _copy_corpus(tmp_path)
    case_path = next((root / "cases").rglob("case.json"))
    case_path.write_text("{", encoding="utf-8")
    errors = lint_corpus(root)
    assert any("cannot read JSON" in error for error in errors)


def test_terminal_parser_distinguishes_chat_and_responses() -> None:
    """Verify that Chat `[DONE]` and the Responses completed terminal are identified separately."""
    chat = b'data: {"choices":[{"delta":{"content":"ok"}}]}\n\ndata: [DONE]\n\n'
    responses = (
        b'event: response.completed\n'
        b'data: {"type":"response.completed","response":{"id":"resp_1"}}\n\n'
    )
    assert terminal_kinds(chat) == ["chat_done"]
    assert terminal_kinds(responses) == ["response_completed"]


def test_generation_is_deterministic_and_reconstructs_sources(tmp_path: Path) -> None:
    """Verify that one seed produces stable variants that can reconstruct canonical wire data."""
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
    """Verify that the coverage report includes required behavior and every generated fragment type."""
    report = build_report(CORPUS_ROOT)
    assert report["case_count"] == 51
    assert report["semantic_case_count"] == 14
    assert report["missing_required_features"] == []
    assert report["missing_required_generation_kinds"] == []
    assert report["missing_required_semantic_features"] == []


def test_native_tool_lifecycle_has_symmetric_chat_and_responses_oracles() -> None:
    """Verify each Native protocol owns call, result, and parallel streaming tool cases."""
    case_ids = {case.case_id for case in discover_cases(CORPUS_ROOT)}
    assert {
        "chat_native.function_tool.non_stream",
        "chat_native.parallel_tools.stream",
        "chat_native.tool_result.non_stream",
        "responses_native.function_tool.non_stream",
        "responses_native.parallel_tools.stream",
        "responses_native.tool_result.non_stream",
    } <= case_ids


def test_lint_rejects_inconsistent_transport_failure_phase(tmp_path: Path) -> None:
    """Verify that a before-output transport failure cannot also claim downstream output was observed."""
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


def test_lint_rejects_terminal_end_for_completed_non_stream_case(
    tmp_path: Path,
) -> None:
    """Verify that a completed non-streaming case uses response rather than terminal as its end marker."""
    root = _copy_corpus(tmp_path)
    case_path = next(
        (root / "cases").rglob("responses_native.text.non_stream/case.json")
    )
    case = json.loads(case_path.read_text(encoding="utf-8"))
    case["transport"]["client_end"] = "terminal"
    case["transport"]["upstream_end"] = "terminal"
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    errors = lint_corpus(root)

    assert any(
        "completed non-stream case requires response client_end" in error
        for error in errors
    )
    assert any(
        "completed non-stream case requires response upstream_end" in error
        for error in errors
    )


def test_lint_rejects_unmarked_sse_type_conflicts_and_post_terminal_events(
    tmp_path: Path,
) -> None:
    """Verify that unmarked SSE type conflicts and post-terminal events are rejected by lint."""
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
    """Verify that generation, report, and package outputs cannot overwrite canonical corpus files."""
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
    """Verify stable packages while excluding generated/reports/dist/runtime derived directories."""
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
