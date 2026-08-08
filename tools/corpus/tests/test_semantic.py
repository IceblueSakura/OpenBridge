"""Validate protocol-neutral function-tool semantic cases and trace verdicts."""

from __future__ import annotations

import copy
import json
import shutil
from pathlib import Path

import pytest

from openbridge_corpus.cli import main
from openbridge_corpus.corpuslib import (
    CorpusError,
    discover_semantic_cases,
    lint_corpus,
    load_json,
)
from openbridge_corpus.semantic import verify_semantic_trace


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CORPUS_ROOT = REPOSITORY_ROOT / "testdata"


def _semantic_case(case_id: str):
    """Return one canonical semantic case by stable ID."""
    return next(
        case
        for case in discover_semantic_cases(CORPUS_ROOT)
        if case.case_id == case_id
    )


def _reference_trace(case_id: str) -> dict:
    """Load the canonical reference trace declared by one semantic case."""
    case = _semantic_case(case_id)
    return load_json(case.directory / case.data["artifacts"]["reference_trace"])


def _copy_corpus(destination: Path) -> Path:
    """Copy canonical inputs without derived outputs for destructive lint tests."""
    target = destination / "testdata"
    shutil.copytree(
        CORPUS_ROOT,
        target,
        ignore=shutil.ignore_patterns("generated", "reports", "dist", "runtime"),
    )
    return target


def test_repository_semantic_cases_are_complete_and_protocol_neutral() -> None:
    """Verify the first semantic release covers each required function-tool decision."""
    cases = discover_semantic_cases(CORPUS_ROOT)
    assert len(cases) == 9
    assert {case.case_id for case in cases} == {
        "function.ambiguous_selection",
        "function.forced_tool",
        "function.missing_argument_clarification",
        "function.no_tool_needed",
        "function.parallel_independent",
        "function.result_grounding",
        "function.single_tool_arguments",
        "function.tool_choice_none",
        "function.tool_choice_required",
    }
    required_targets = {
        "chat_native",
        "responses_native",
        "chat_to_responses",
        "responses_to_chat",
    }
    assert all(set(case.data["applies_to"]) == required_targets for case in cases)


def test_every_reference_trace_satisfies_its_semantic_oracle() -> None:
    """Verify every checked-in reference trace is a positive deterministic example."""
    for case in discover_semantic_cases(CORPUS_ROOT):
        trace = load_json(case.directory / case.data["artifacts"]["reference_trace"])
        assert verify_semantic_trace(CORPUS_ROOT, case.case_id, trace) == []


def test_semantic_verifier_rejects_wrong_missing_and_additional_calls() -> None:
    """Verify call-set mismatches produce stable diagnostics without argument bodies."""
    case_id = "function.single_tool_arguments"
    matching = _reference_trace(case_id)

    wrong = copy.deepcopy(matching)
    wrong["events"][0]["name"] = "SENSITIVE_TOOL_SENTINEL"
    wrong_errors = verify_semantic_trace(CORPUS_ROOT, case_id, wrong)
    assert any("tool_calls[0].name" in error for error in wrong_errors)
    assert all("SENSITIVE_TOOL_SENTINEL" not in error for error in wrong_errors)

    missing = copy.deepcopy(matching)
    missing["events"] = []
    assert any(
        "required tool call" in error
        for error in verify_semantic_trace(CORPUS_ROOT, case_id, missing)
    )

    additional = copy.deepcopy(matching)
    additional["events"].append(
        {
            "arguments": {"location": "Osaka", "unit": "celsius"},
            "call_id": "call_extra",
            "name": "get_weather",
            "turn": 0,
            "type": "assistant_tool_call",
        }
    )
    assert any(
        "unexpected tool call" in error
        for error in verify_semantic_trace(CORPUS_ROOT, case_id, additional)
    )


def test_semantic_verifier_checks_argument_schema_and_expected_values() -> None:
    """Verify argument validity and exact expected values are separate deterministic checks."""
    case_id = "function.single_tool_arguments"
    invalid = _reference_trace(case_id)
    invalid["events"][0]["arguments"]["unit"] = "kelvin"

    errors = verify_semantic_trace(CORPUS_ROOT, case_id, invalid)

    assert any("tool_calls[0].arguments" in error for error in errors)
    assert any("required_calls[0].arguments" in error for error in errors)
    assert all("kelvin" not in error for error in errors)

    malformed = _reference_trace(case_id)
    malformed["events"][0]["arguments"] = "SENSITIVE_SCHEMA_SENTINEL"
    with pytest.raises(CorpusError) as captured:
        verify_semantic_trace(CORPUS_ROOT, case_id, malformed)
    assert "SENSITIVE_SCHEMA_SENTINEL" not in str(captured.value)


def test_semantic_verifier_accepts_unordered_parallel_calls() -> None:
    """Verify independent parallel calls match as a set instead of by emission order."""
    case_id = "function.parallel_independent"
    trace = _reference_trace(case_id)
    call_names = [
        event["name"]
        for event in trace["events"]
        if event["type"] == "assistant_tool_call"
    ]
    oracle_names = [
        call["name"]
        for call in _semantic_case(case_id).data["oracle"]["calls"]["required"]
    ]

    assert call_names == list(reversed(oracle_names))
    assert verify_semantic_trace(CORPUS_ROOT, case_id, trace) == []


def test_semantic_verifier_enforces_no_tool_and_final_response_facts() -> None:
    """Verify forbidden calls and grounded final-answer facts are independently enforced."""
    no_tool_id = "function.no_tool_needed"
    unexpected_call = _reference_trace(no_tool_id)
    unexpected_call["events"].insert(
        0,
        {
            "arguments": {"location": "Shanghai", "unit": "celsius"},
            "call_id": "call_unexpected",
            "name": "get_weather",
            "turn": 0,
            "type": "assistant_tool_call",
        },
    )
    assert any(
        "unexpected tool call" in error
        for error in verify_semantic_trace(CORPUS_ROOT, no_tool_id, unexpected_call)
    )

    grounding_id = "function.result_grounding"
    missing_result = _reference_trace(grounding_id)
    missing_result["events"] = [
        event for event in missing_result["events"] if event["type"] != "tool_result"
    ]
    assert any(
        "required_results[0] is missing" in error
        for error in verify_semantic_trace(CORPUS_ROOT, grounding_id, missing_result)
    )

    wrong_result = _reference_trace(grounding_id)
    wrong_result["events"][1]["output"]["quantity"] = 999
    result_errors = verify_semantic_trace(CORPUS_ROOT, grounding_id, wrong_result)
    assert any("required_results[0].output" in error for error in result_errors)
    assert all("999" not in error for error in result_errors)

    nonfinal_message = _reference_trace(grounding_id)
    message = nonfinal_message["events"].pop()
    message["turn"] = 1
    nonfinal_message["events"][1]["turn"] = 2
    nonfinal_message["events"].insert(1, message)
    assert any(
        "is not the final trace event" in error
        for error in verify_semantic_trace(CORPUS_ROOT, grounding_id, nonfinal_message)
    )

    ungrounded = _reference_trace(grounding_id)
    ungrounded["events"][-1]["text"] = "SENSITIVE_FINAL_SENTINEL"
    errors = verify_semantic_trace(CORPUS_ROOT, grounding_id, ungrounded)
    assert any("final_response.contains_all" in error for error in errors)
    assert all("SENSITIVE_FINAL_SENTINEL" not in error for error in errors)


def test_semantic_lint_rejects_catalog_and_strict_schema_drift(
    tmp_path: Path,
) -> None:
    """Verify semantic discovery and strict function schemas remain canonical lint contracts."""
    root = _copy_corpus(tmp_path)

    # Remove one semantic catalog declaration.
    catalog_path = root / "catalog.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
    catalog["semantic_case_ids"] = catalog["semantic_case_ids"][1:]
    catalog_path.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    # Break the strict root object guarantee in one function definition.
    case_path = next(
        (root / "semantic-cases").rglob("function.single_tool_arguments/case.json")
    )
    case = json.loads(case_path.read_text(encoding="utf-8"))
    case["task"]["tools"][0]["parameters"]["additionalProperties"] = True
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    errors = lint_corpus(root)

    assert any("catalog/semantic-case mismatch" in error for error in errors)
    assert any("strict tool schema" in error for error in errors)


def test_semantic_cli_reports_safe_pass_and_failure_verdicts(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Verify the CLI exposes verdicts without echoing trace content."""
    case_id = "function.result_grounding"
    matching_path = tmp_path / "matching.json"
    matching_path.write_text(
        json.dumps(_reference_trace(case_id), ensure_ascii=False),
        encoding="utf-8",
    )
    assert (
        main(
            [
                "--root",
                str(CORPUS_ROOT),
                "verify-semantic-trace",
                "--case",
                case_id,
                "--trace",
                str(matching_path),
            ]
        )
        == 0
    )
    captured = capsys.readouterr()
    assert "semantic trace passed" in captured.out
    assert captured.err == ""

    failing = _reference_trace(case_id)
    failing["events"][-1]["text"] = "SENSITIVE_CLI_SENTINEL"
    failing_path = tmp_path / "failing.json"
    failing_path.write_text(json.dumps(failing), encoding="utf-8")
    assert (
        main(
            [
                "--root",
                str(CORPUS_ROOT),
                "verify-semantic-trace",
                "--case",
                case_id,
                "--trace",
                str(failing_path),
            ]
        )
        == 1
    )
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "semantic trace failed" in captured.err
    assert "SENSITIVE_CLI_SENTINEL" not in captured.err
